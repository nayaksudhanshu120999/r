use std::env;
use std::ffi::{c_void, OsStr};
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT,
    WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_SPACE,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetForegroundWindow,
    GetMessageW, GetSystemMetrics, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
    IsIconic, IsWindow, PostMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    SetWindowLongW, ShowWindow, TrackPopupMenu, TranslateMessage, UnregisterClassW, CW_USEDEFAULT,
    GWL_STYLE, HICON, HMENU, MF_SEPARATOR, MF_STRING, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    SW_SHOW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, WM_APP, WM_DESTROY,
    WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_MAXIMIZEBOX,
    WS_OVERLAPPEDWINDOW, WS_THICKFRAME,
};

#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        lp_mutex_attributes: *const c_void,
        b_initial_owner: i32,
        lp_name: *const u16,
    ) -> HANDLE;
}

#[derive(Debug, Clone, Copy)]
pub enum SystemEvent {
    ShowPalette,
    TogglePalette,
    OpenSettings,
    ReloadConfig,
    QuitApp,
}

const HOTKEY_ID: i32 = 1;
const TRAY_ICON_ID: u32 = 1001;

const WM_TRAYICON: u32 = WM_APP + 1;
const WM_REBIND_HOTKEY: u32 = WM_APP + 2;
const WM_IPC_SHOW_PALETTE: u32 = WM_APP + 3;
const WM_IPC_RELOAD_CONFIG: u32 = WM_APP + 4;
const WM_IPC_SHUTDOWN: u32 = WM_APP + 5;

const IDM_OPEN_PALETTE: usize = 2001;
const IDM_OPEN_SETTINGS: usize = 2002;
const IDM_RELOAD_CONFIG: usize = 2003;
const IDM_QUIT_APP: usize = 2004;

const HIDDEN_CLASS_NAME: &str = "CmdPalRsTrayHostWindowClass";
const HIDDEN_WINDOW_TITLE: &str = "CmdPalRsTrayHostWindow";
const SINGLE_INSTANCE_MUTEX: &str = "Local\\CmdPalRsSingleInstanceMutex";

static EVENT_TX: OnceLock<Mutex<Sender<SystemEvent>>> = OnceLock::new();
static HOST_HWND: AtomicIsize = AtomicIsize::new(0);
static PALETTE_HWND: AtomicIsize = AtomicIsize::new(0);
static INSTANCE_MUTEX: AtomicIsize = AtomicIsize::new(0);

pub fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn send_event(ev: SystemEvent) {
    if let Some(lock) = EVENT_TX.get() {
        if let Ok(tx) = lock.lock() {
            let _ = tx.send(ev);
        }
    }
}

pub fn parse_hotkey_spec(raw: &str) -> (u32, u32, String) {
    let mut mods = MOD_NOREPEAT;
    let mut vk: u32 = VK_SPACE as u32;
    let mut key_label = "Space".to_string();

    for part in raw.split('+') {
        let token = part.trim();
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "alt" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "win" | "super" | "meta" => mods |= MOD_WIN,
            "space" | "spacebar" => {
                vk = VK_SPACE as u32;
                key_label = "Space".to_string();
            }
            _ if token.len() == 1 => {
                let ch = token.chars().next().unwrap().to_ascii_uppercase();
                if ch.is_ascii_alphanumeric() {
                    vk = ch as u32;
                    key_label = ch.to_string();
                }
            }
            _ => {}
        }
    }

    if (mods & (MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_WIN)) == 0 {
        mods |= MOD_CONTROL;
    }

    let mut parts = Vec::with_capacity(5);
    if (mods & MOD_CONTROL) != 0 {
        parts.push("Ctrl".to_string());
    }
    if (mods & MOD_ALT) != 0 {
        parts.push("Alt".to_string());
    }
    if (mods & MOD_SHIFT) != 0 {
        parts.push("Shift".to_string());
    }
    if (mods & MOD_WIN) != 0 {
        parts.push("Win".to_string());
    }
    parts.push(key_label);

    (mods, vk, parts.join("+"))
}

pub fn normalize_hotkey_str(raw: &str) -> String {
    let (_, _, canonical) = parse_hotkey_spec(raw);
    canonical
}

pub fn apply_runtime_system_config(run_on_startup: bool, hotkey: &str) {
    sync_windows_startup_registry(run_on_startup);
    let (mods, vk, _) = parse_hotkey_spec(hotkey);

    let hwnd_val = HOST_HWND.load(Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe {
            let _ = PostMessageW(
                hwnd_val as HWND,
                WM_REBIND_HOTKEY,
                mods as WPARAM,
                vk as LPARAM,
            );
        }
    }
}

pub fn ensure_single_instance_or_activate_existing() -> bool {
    let mutex_w = to_wide_null(SINGLE_INSTANCE_MUTEX);
    let class_w = to_wide_null(HIDDEN_CLASS_NAME);
    let title_w = to_wide_null(HIDDEN_WINDOW_TITLE);

    unsafe {
        let hmutex = CreateMutexW(null(), 1, mutex_w.as_ptr());
        if !hmutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(hmutex);
            let existing_hwnd = FindWindowW(class_w.as_ptr(), title_w.as_ptr());
            if !existing_hwnd.is_null() {
                let _ = PostMessageW(existing_hwnd, WM_IPC_SHOW_PALETTE, 0, 0);
            }
            return false;
        }
        INSTANCE_MUTEX.store(hmutex as isize, Ordering::SeqCst);
    }
    true
}

pub fn shutdown_system_integration() {
    let hwnd_val = HOST_HWND.swap(0, Ordering::SeqCst);
    if hwnd_val != 0 {
        unsafe {
            let _ = PostMessageW(hwnd_val as HWND, WM_IPC_SHUTDOWN, 0, 0);
        }
    }
    let hmutex = INSTANCE_MUTEX.swap(0, Ordering::SeqCst);
    if hmutex != 0 {
        unsafe {
            let _ = CloseHandle(hmutex as HANDLE);
        }
    }
}

pub fn init_system_integration(run_on_startup: bool, hotkey: &str) -> Receiver<SystemEvent> {
    let (tx, rx) = mpsc::channel();
    let _ = EVENT_TX.set(Mutex::new(tx));

    sync_windows_startup_registry(run_on_startup);
    let (initial_mods, initial_vk, _) = parse_hotkey_spec(hotkey);

    thread::spawn(move || unsafe {
        let hinstance = GetModuleHandleW(null());
        let class_w = to_wide_null(HIDDEN_CLASS_NAME);
        let title_w = to_wide_null(HIDDEN_WINDOW_TITLE);

        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(tray_host_wnd_proc);
        wc.hInstance = hinstance;
        wc.lpszClassName = class_w.as_ptr();
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_w.as_ptr(),
            title_w.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        );

        if hwnd.is_null() {
            return;
        }

        HOST_HWND.store(hwnd as isize, Ordering::SeqCst);

        let _ = RegisterHotKey(hwnd, HOTKEY_ID, initial_mods, initial_vk);

        let (and_mask, bgra) = crate::icon::create_win32_tray_icon_buffers(32);
        let hicon: HICON = CreateIcon(
            hinstance,
            32,
            32,
            1,
            32,
            and_mask.as_ptr(),
            bgra.as_ptr(),
        );

        let mut nid: NOTIFYICONDATAW = zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        nid.hIcon = hicon;

        let tip = to_wide_null("Command Palette (cmdpal-rs)");
        let copy_len = tip.len().min(nid.szTip.len());
        nid.szTip[..copy_len].copy_from_slice(&tip[..copy_len]);

        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        if !hicon.is_null() {
            let _ = DestroyIcon(hicon);
        }
        let _ = DestroyWindow(hwnd);
        let _ = UnregisterClassW(class_w.as_ptr(), hinstance);
    });

    rx
}

unsafe extern "system" fn tray_host_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if wparam as i32 == HOTKEY_ID {
                send_event(SystemEvent::TogglePalette);
            }
            0
        }
        WM_REBIND_HOTKEY => {
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID);
            let _ = RegisterHotKey(hwnd, HOTKEY_ID, wparam as u32, lparam as u32);
            0
        }
        WM_IPC_SHOW_PALETTE => {
            send_event(SystemEvent::ShowPalette);
            0
        }
        WM_IPC_RELOAD_CONFIG => {
            send_event(SystemEvent::ReloadConfig);
            0
        }
        WM_IPC_SHUTDOWN => {
            PostQuitMessage(0);
            0
        }
        WM_TRAYICON => {
            let mouse_msg = (lparam & 0xFFFF) as u32;
            match mouse_msg {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    send_event(SystemEvent::TogglePalette);
                }
                WM_RBUTTONUP => {
                    show_tray_context_menu(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_tray_context_menu(hwnd: HWND) {
    let hmenu: HMENU = CreatePopupMenu();
    if hmenu.is_null() {
        return;
    }

    let open_pal = to_wide_null("Open Command Palette");
    let open_set = to_wide_null("Settings...");
    let reload_cfg = to_wide_null("Reload Config & Rescan");
    let quit_app = to_wide_null("Exit Command Palette");

    let _ = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_PALETTE, open_pal.as_ptr());
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_SETTINGS, open_set.as_ptr());
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_RELOAD_CONFIG, reload_cfg.as_ptr());
    let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, null());
    let _ = AppendMenuW(hmenu, MF_STRING, IDM_QUIT_APP, quit_app.as_ptr());

    let mut pt: POINT = zeroed();
    let _ = GetCursorPos(&mut pt);
    let _ = SetForegroundWindow(hwnd);

    let cmd = TrackPopupMenu(
        hmenu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_BOTTOMALIGN | TPM_LEFTALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        null(),
    );

    let _ = DestroyMenu(hmenu);

    match cmd as usize {
        IDM_OPEN_PALETTE => send_event(SystemEvent::ShowPalette),
        IDM_OPEN_SETTINGS => send_event(SystemEvent::OpenSettings),
        IDM_RELOAD_CONFIG => send_event(SystemEvent::ReloadConfig),
        IDM_QUIT_APP => send_event(SystemEvent::QuitApp),
        _ => {}
    }
}

pub fn sync_windows_startup_registry(enable: bool) {
    let subkey = to_wide_null("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let val_name = to_wide_null("CmdPalRs");

    unsafe {
        let mut hkey: HKEY = null_mut();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey);
        if status != 0 || hkey.is_null() {
            return;
        }

        if enable {
            if let Ok(exe_path) = env::current_exe() {
                let quoted = format!("\"{}\"", exe_path.to_string_lossy());
                let data_w = to_wide_null(&quoted);
                let byte_len = (data_w.len() * 2) as u32;
                let _ = RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data_w.as_ptr() as *const u8,
                    byte_len,
                );
            }
        } else {
            let _ = RegDeleteValueW(hkey, val_name.as_ptr());
        }

        let _ = RegCloseKey(hkey);
    }
}

pub fn get_centered_palette_coords(win_w: i32, win_h: i32) -> (i32, i32) {
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = ((screen_w - win_w) / 2).max(0);
        let y = ((screen_h - win_h) / 4).max(40);
        (x, y)
    }
}

pub fn lock_window_resize_by_title(title: &str) {
    let title_w = to_wide_null(title);
    unsafe {
        let hwnd = FindWindowW(null(), title_w.as_ptr());
        if !hwnd.is_null() {
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let new_style = style & !(WS_THICKFRAME as i32) & !(WS_MAXIMIZEBOX as i32);
            let _ = SetWindowLongW(hwnd, GWL_STYLE, new_style);
        }
    }
}

unsafe fn resolve_own_palette_hwnd(title: &str) -> HWND {
    let cached = PALETTE_HWND.load(Ordering::Relaxed) as HWND;
    if !cached.is_null() && IsWindow(cached) != 0 {
        return cached;
    }

    let title_w = to_wide_null(title);
    let hwnd = FindWindowW(null(), title_w.as_ptr());
    if !hwnd.is_null() {
        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() {
            PALETTE_HWND.store(hwnd as isize, Ordering::Relaxed);
            return hwnd;
        }
    }
    null_mut()
}

pub fn force_focus_window_by_title(title: &str) {
    unsafe {
        let hwnd = if title == "Command Palette" {
            resolve_own_palette_hwnd(title)
        } else {
            let title_w = to_wide_null(title);
            FindWindowW(null(), title_w.as_ptr())
        };
        if !hwnd.is_null() {
            if IsIconic(hwnd) != 0 {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

pub fn check_outside_click_or_focus_loss(title: &str) -> bool {
    unsafe {
        let hwnd = resolve_own_palette_hwnd(title);
        if hwnd.is_null() {
            return false;
        }

        let fg = GetForegroundWindow();
        if !fg.is_null() && fg != hwnd {
            return true;
        }

        let l_down = (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0;
        let r_down = (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0;
        let m_down = (GetAsyncKeyState(VK_MBUTTON as i32) as u16 & 0x8000) != 0;

        if l_down || r_down || m_down {
            let mut pt: POINT = zeroed();
            let mut rect: RECT = zeroed();
            if GetCursorPos(&mut pt) != 0 && GetWindowRect(hwnd, &mut rect) != 0 {
                let inside = pt.x >= rect.left
                    && pt.x <= rect.right
                    && pt.y >= rect.top
                    && pt.y <= rect.bottom;
                if !inside {
                    return true;
                }
            }
        }

        false
    }
}

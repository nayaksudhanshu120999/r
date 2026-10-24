use std::sync::mpsc::{self, Receiver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEvent {
    ShowPalette,
    TogglePalette,
    OpenSettings,
    ReloadConfig,
    QuitApp,
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::SystemEvent;
    use crate::icon::create_win32_tray_icon_buffers;
    use std::mem::zeroed;
    use std::ptr::null;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::OnceLock;
    use std::thread;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, RECT,
        WPARAM,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, CreateMutexW, GetCurrentThreadId,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
        VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_SPACE,
    };
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
        DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos,
        GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowLongW, GetWindowRect,
        GetWindowThreadProcessId, IsWindow, PostMessageW, PostQuitMessage, RegisterClassW,
        SetForegroundWindow, SetWindowLongW, SetWindowPos, ShowWindow, TrackPopupMenu,
        TranslateMessage, GWL_STYLE, MF_SEPARATOR, MF_STRING, MSG, SM_CXSCREEN, SM_CYSCREEN,
        SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, SW_SHOW,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CLOSE,
        WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_THICKFRAME,
    };

    const TRAY_CLASS_NAME: &str = "CmdPalRsTrayHiddenWndClass";
    const WM_TRAYICON: u32 = WM_APP + 1;
    const WM_IPC_SHOW_PALETTE: u32 = WM_APP + 2;
    const HOTKEY_ID_CTRL_SPACE: i32 = 1;

    const MENU_OPEN_PALETTE: u32 = 1001;
    const MENU_OPEN_SETTINGS: u32 = 1002;
    const MENU_RELOAD_CONFIG: u32 = 1003;
    const MENU_QUIT_APP: u32 = 1004;

    static EVENT_SENDER: OnceLock<Sender<SystemEvent>> = OnceLock::new();
    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static CACHED_PALETTE_HWND: AtomicIsize = AtomicIsize::new(0);

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn emit_event(event: SystemEvent) {
        if let Some(tx) = EVENT_SENDER.get() {
            let _ = tx.send(event);
        }
        let _ = slint::invoke_from_event_loop(|| {});
    }

    /// Ensures only one instance of cmdpal-rs runs at a time.
    /// If another instance is already running, signals it to show the palette and returns `false`.
    pub fn ensure_single_instance() -> bool {
        unsafe {
            let class_wide = to_wide(TRAY_CLASS_NAME);
            let existing_hwnd = FindWindowW(class_wide.as_ptr(), null());

            let mutex_name = to_wide("Local\\CmdPalRsSingleInstanceMutex");
            let handle = CreateMutexW(null(), 1, mutex_name.as_ptr());

            if handle != 0 && GetLastError() == ERROR_ALREADY_EXISTS {
                if existing_hwnd != 0 {
                    PostMessageW(existing_hwnd, WM_IPC_SHOW_PALETTE, 0, 0);
                }
                CloseHandle(handle);
                return false;
            }

            // If an older build (without the mutex) left a tray host window open, tell it to close
            if existing_hwnd != 0 {
                PostMessageW(existing_hwnd, WM_DESTROY, 0, 0);
            }

            true
        }
    }

    pub fn cleanup_tray_window() {
        let hwnd = TRAY_HWND.load(Ordering::SeqCst);
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
        }
    }

    pub fn spawn_tray_and_hotkey_thread(tx: Sender<SystemEvent>) {
        let _ = EVENT_SENDER.set(tx);

        thread::spawn(move || unsafe {
            let hinstance = GetModuleHandleW(null());
            let class_name = to_wide(TRAY_CLASS_NAME);

            let mut wc: WNDCLASSW = zeroed();
            wc.lpfnWndProc = Some(tray_wnd_proc);
            wc.hInstance = hinstance;
            wc.lpszClassName = class_name.as_ptr();
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                to_wide("CmdPalRsTrayHost").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                hinstance,
                null(),
            );

            if hwnd == 0 {
                return;
            }
            TRAY_HWND.store(hwnd, Ordering::SeqCst);

            let _ = RegisterHotKey(
                hwnd,
                HOTKEY_ID_CTRL_SPACE,
                (MOD_CONTROL | MOD_NOREPEAT) as u32,
                VK_SPACE as u32,
            );

            let (and_mask, bgra) = create_win32_tray_icon_buffers(32);
            let hicon = CreateIcon(
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
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = hicon;

            let tip = to_wide("Command Palette (Ctrl+Space)");
            for (i, &ch) in tip.iter().take(127).enumerate() {
                nid.szTip[i] = ch;
            }

            Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_CTRL_SPACE);
            if hicon != 0 {
                DestroyIcon(hicon);
            }
        });
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_HOTKEY => {
                if wparam as i32 == HOTKEY_ID_CTRL_SPACE {
                    emit_event(SystemEvent::TogglePalette);
                }
                0
            }
            WM_IPC_SHOW_PALETTE => {
                emit_event(SystemEvent::ShowPalette);
                0
            }
            WM_TRAYICON => {
                let mouse_msg = (lparam as u32) & 0xFFFF;
                if mouse_msg == WM_LBUTTONUP {
                    emit_event(SystemEvent::TogglePalette);
                } else if mouse_msg == WM_RBUTTONUP {
                    show_tray_context_menu(hwnd);
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
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
        let hmenu = CreatePopupMenu();
        if hmenu == 0 {
            return;
        }

        let label_open = to_wide("Open Command Palette\tCtrl+Space");
        let label_settings = to_wide("Settings...");
        let label_reload = to_wide("Reload Apps & Config");
        let label_quit = to_wide("Quit Command Palette");

        AppendMenuW(hmenu, MF_STRING, MENU_OPEN_PALETTE as usize, label_open.as_ptr());
        AppendMenuW(
            hmenu,
            MF_STRING,
            MENU_OPEN_SETTINGS as usize,
            label_settings.as_ptr(),
        );
        AppendMenuW(
            hmenu,
            MF_STRING,
            MENU_RELOAD_CONFIG as usize,
            label_reload.as_ptr(),
        );
        AppendMenuW(hmenu, MF_SEPARATOR, 0, null());
        AppendMenuW(hmenu, MF_STRING, MENU_QUIT_APP as usize, label_quit.as_ptr());

        let mut pt: POINT = zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);

        let selected = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            null(),
        );
        PostMessageW(hwnd, 0, 0, 0);
        DestroyMenu(hmenu);

        match selected as u32 {
            MENU_OPEN_PALETTE => emit_event(SystemEvent::TogglePalette),
            MENU_OPEN_SETTINGS => emit_event(SystemEvent::OpenSettings),
            MENU_RELOAD_CONFIG => emit_event(SystemEvent::ReloadConfig),
            MENU_QUIT_APP => emit_event(SystemEvent::QuitApp),
            _ => {}
        }
    }

    pub fn find_window_by_title(title: &str) -> HWND {
        unsafe {
            let wide = to_wide(title);
            FindWindowW(null(), wide.as_ptr())
        }
    }

    fn get_palette_hwnd(palette_title: &str) -> HWND {
        let cached = CACHED_PALETTE_HWND.load(Ordering::Relaxed);
        unsafe {
            if cached != 0 && IsWindow(cached) != 0 {
                return cached;
            }
        }
        let found = find_window_by_title(palette_title);
        if found != 0 {
            CACHED_PALETTE_HWND.store(found, Ordering::Relaxed);
        }
        found
    }

    pub fn enable_maximize_and_resize(title: &str) {
        unsafe {
            let hwnd = find_window_by_title(title);
            if hwnd == 0 {
                return;
            }
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let new_style = style | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_THICKFRAME;
            SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    pub fn force_activate_window(title: &str) {
        unsafe {
            let hwnd = find_window_by_title(title);
            if hwnd == 0 {
                return;
            }
            let fg_hwnd = GetForegroundWindow();
            let current_thread = GetCurrentThreadId();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, std::ptr::null_mut());

            if fg_thread != 0 && fg_thread != current_thread {
                AttachThreadInput(fg_thread, current_thread, 1);
                ShowWindow(hwnd, SW_RESTORE);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
                AttachThreadInput(fg_thread, current_thread, 0);
            } else {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
            }
        }
    }

    pub fn get_primary_screen_size() -> (f32, f32) {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN).max(1280) as f32;
            let h = GetSystemMetrics(SM_CYSCREEN).max(720) as f32;
            (w, h)
        }
    }

    pub fn should_auto_hide_palette(palette_title: &str, check_focus: bool) -> bool {
        unsafe {
            let hwnd = get_palette_hwnd(palette_title);
            if hwnd == 0 {
                return false;
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

            if check_focus {
                let fg = GetForegroundWindow();
                if fg != 0 && fg != hwnd {
                    return true;
                }
            }

            false
        }
    }
}

pub fn ensure_single_instance_or_activate_existing() -> bool {
    #[cfg(target_os = "windows")]
    {
        win32::ensure_single_instance()
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

pub fn shutdown_system_integration() {
    #[cfg(target_os = "windows")]
    win32::cleanup_tray_window();
}

pub fn init_system_integration() -> Receiver<SystemEvent> {
    let (tx, rx) = mpsc::channel();
    #[cfg(target_os = "windows")]
    win32::spawn_tray_and_hotkey_thread(tx);
    #[cfg(not(target_os = "windows"))]
    let _ = tx;
    rx
}

pub fn activate_window_by_title(title: &str) {
    #[cfg(target_os = "windows")]
    win32::force_activate_window(title);
    #[cfg(not(target_os = "windows"))]
    let _ = title;
}

pub fn enable_window_maximize(title: &str) {
    #[cfg(target_os = "windows")]
    win32::enable_maximize_and_resize(title);
    #[cfg(not(target_os = "windows"))]
    let _ = title;
}

pub fn primary_screen_size() -> (f32, f32) {
    #[cfg(target_os = "windows")]
    {
        win32::get_primary_screen_size()
    }
    #[cfg(not(target_os = "windows"))]
    {
        (1920.0, 1080.0)
    }
}

pub fn check_outside_click_or_focus_loss(palette_title: &str, check_focus: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        win32::should_auto_hide_palette(palette_title, check_focus)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (palette_title, check_focus);
        false
    }
}

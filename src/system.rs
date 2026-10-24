use std::sync::mpsc::{self, Receiver};
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEvent {
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
    use std::sync::mpsc::Sender;
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{ HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
        VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_SPACE,
    };
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
        DestroyMenu, DispatchMessageW, FindWindowW, GetCursorPos, GetForegroundWindow,
        GetMessageW, GetSystemMetrics, GetWindowRect, GetWindowThreadProcessId, PostMessageW,
        PostQuitMessage, RegisterClassW, SetForegroundWindow, ShowWindow, TrackPopupMenu,
        TranslateMessage, MF_SEPARATOR, MF_STRING, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
        SW_SHOW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
        WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    };

    const WM_TRAYICON: u32 = WM_APP + 1;
    const HOTKEY_ID_CTRL_SPACE: i32 = 1;

    const MENU_OPEN_PALETTE: u32 = 1001;
    const MENU_OPEN_SETTINGS: u32 = 1002;
    const MENU_RELOAD_CONFIG: u32 = 1003;
    const MENU_QUIT_APP: u32 = 1004;

    static EVENT_SENDER: OnceLock<Sender<SystemEvent>> = OnceLock::new();

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn emit_event(event: SystemEvent) {
        if let Some(tx) = EVENT_SENDER.get() {
            let _ = tx.send(event);
        }
        slint::invoke_from_event_loop(|| {});
    }

    pub fn spawn_tray_and_hotkey_thread(tx: Sender<SystemEvent>) {
        let _ = EVENT_SENDER.set(tx);

        thread::spawn(move || unsafe {
            let hinstance = GetModuleHandleW(null());
            let class_name = to_wide("CmdPalRsTrayHiddenWndClass");

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

            // Register global Ctrl+Space shortcut
            let _ = RegisterHotKey(
                hwnd,
                HOTKEY_ID_CTRL_SPACE,
                (MOD_CONTROL | MOD_NOREPEAT) as u32,
                VK_SPACE as u32,
            );

            // Create 32x32 Tray HICON
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

            // Register Taskbar Notification Area (Tray) Icon
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
            WM_TRAYICON => {
                let mouse_msg = (lparam as u32) & 0xFFFF;
                if mouse_msg == WM_LBUTTONUP {
                    emit_event(SystemEvent::TogglePalette);
                } else if mouse_msg == WM_RBUTTONUP {
                    show_tray_context_menu(hwnd);
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
            let hwnd = find_window_by_title(palette_title);
            if hwnd == 0 {
                return false;
            }

            // 1. Check if any mouse button was clicked outside the palette bounds
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

            // 2. Check if foreground focus moved to another window
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

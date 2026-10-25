use crate::models::{CandidateItem, ExecutionOutcome, ExecutionTarget};
use crate::notes::execute_note_action;
use crate::system::to_wide_null;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::ptr::{copy_nonoverlapping, null, null_mut};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const CF_UNICODETEXT: u32 = 13;

pub fn open_path_in_default_app(path: &Path) -> io::Result<()> {
    spawn_file_or_app_path(path, false)
}

pub fn execute_candidate(
    item: &CandidateItem,
    run_as_admin: bool,
    custom_cmd_path: &str,
) -> io::Result<ExecutionOutcome> {
    match &item.target {
        ExecutionTarget::SystemProcess { program, args } => {
            spawn_system_process(program, args, run_as_admin)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::CustomCommand(cmd_line) => {
            spawn_custom_command(cmd_line, run_as_admin, custom_cmd_path)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::SettingsUri(uri_or_path) => {
            spawn_settings_target(uri_or_path)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::WebUrl(url) => {
            spawn_shell_url(url)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::AppPath(path) => {
            spawn_file_or_app_path(path, run_as_admin)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::FilePath(path) => {
            if !path.exists() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("txt") || ext.eq_ignore_ascii_case("md") {
                        if let Some(parent) = path.parent() {
                            if !parent.as_os_str().is_empty() && !parent.exists() {
                                let _ = fs::create_dir_all(parent);
                            }
                        }
                        let _ = OpenOptions::new().create(true).write(true).open(path);
                    }
                }
            }
            spawn_file_or_app_path(path, run_as_admin)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::CopyToClipboard(text) => {
            copy_text_to_clipboard(text)?;
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::MutateConfig(mutation) => {
            Ok(ExecutionOutcome::ApplyConfigMutation(mutation.clone()))
        }
        ExecutionTarget::ReloadIndex => Ok(ExecutionOutcome::ReloadAll),
        ExecutionTarget::OpenPaletteSettingsTab(tab) => {
            Ok(ExecutionOutcome::OpenSettingsTab(*tab))
        }
        ExecutionTarget::ActivateBang { bang, label } => Ok(ExecutionOutcome::ActivateBang {
            bang: bang.clone(),
            label: label.clone(),
        }),
        ExecutionTarget::NoteAction(action) => {
            let keep_open = execute_note_action(action)?;
            if keep_open {
                Ok(ExecutionOutcome::KeepOpenAndRefresh { clear_query: true })
            } else {
                Ok(ExecutionOutcome::HidePalette)
            }
        }
        ExecutionTarget::NoOp => Ok(ExecutionOutcome::KeepOpenAndRefresh { clear_query: false }),
    }
}

/// Zero-process native Win32 UTF-16 clipboard copy (eliminates `clip.exe` spawn & handle leaks).
fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    let wide = to_wide_null(text);
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return Err(io::Error::last_os_error());
        }
        let _ = EmptyClipboard();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if hmem.is_null() {
            let _ = CloseClipboard();
            return Err(io::Error::last_os_error());
        }

        let locked = GlobalLock(hmem) as *mut u16;
        if locked.is_null() {
            let _ = CloseClipboard();
            return Err(io::Error::last_os_error());
        }

        copy_nonoverlapping(wide.as_ptr(), locked, wide.len());
        let _ = GlobalUnlock(hmem);

        if SetClipboardData(CF_UNICODETEXT, hmem).is_null() {
            let _ = CloseClipboard();
            return Err(io::Error::last_os_error());
        }

        let _ = CloseClipboard();
    }
    Ok(())
}

fn spawn_custom_command(
    cmd_line: &str,
    run_as_admin: bool,
    custom_cmd_path: &str,
) -> io::Result<()> {
    let trimmed = cmd_line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let shell = if custom_cmd_path.trim().is_empty() {
        "cmd.exe"
    } else {
        custom_cmd_path.trim()
    };

    if Path::new(trimmed).exists() {
        return spawn_file_or_app_path(Path::new(trimmed), run_as_admin);
    }

    if run_as_admin {
        let ps_command = format!(
            "Start-Process -FilePath '{}' -ArgumentList '/C {}' -Verb RunAs",
            escape_ps_single_quoted(shell),
            escape_ps_single_quoted(trimmed)
        );
        drop(
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &ps_command,
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?,
        );
    } else {
        drop(Command::new(shell).args(["/C", trimmed]).spawn()?);
    }

    Ok(())
}

fn spawn_system_process(program: &str, args: &[String], run_as_admin: bool) -> io::Result<()> {
    let trimmed_prog = program.trim();
    if trimmed_prog.is_empty() {
        return Ok(());
    }

    if run_as_admin {
        let escaped_prog = escape_ps_single_quoted(trimmed_prog);
        let ps_command = if args.is_empty() {
            format!("Start-Process -FilePath '{}' -Verb RunAs", escaped_prog)
        } else {
            let joined_args: Vec<String> = args
                .iter()
                .map(|a| format!("'{}'", escape_ps_single_quoted(a)))
                .collect();
            format!(
                "Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs",
                escaped_prog,
                joined_args.join(", ")
            )
        };

        drop(
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &ps_command,
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?,
        );
    } else {
        let mut cmd = Command::new(trimmed_prog);
        cmd.args(args);
        if should_hide_console(trimmed_prog) {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        drop(cmd.spawn()?);
    }

    Ok(())
}

fn spawn_settings_target(uri_or_path: &str) -> io::Result<()> {
    let trimmed = uri_or_path.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if trimmed.contains(':') && !Path::new(trimmed).exists() {
        spawn_shell_url(trimmed)
    } else {
        spawn_file_or_app_path(Path::new(trimmed), false)
    }
}

fn spawn_shell_url(url_or_uri: &str) -> io::Result<()> {
    let url_w = to_wide_null(url_or_uri);
    let verb_w = to_wide_null("open");
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            verb_w.as_ptr(),
            url_w.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if (result as isize) <= 32 {
        drop(
            Command::new("cmd.exe")
                .args(["/C", "start", "", url_or_uri])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?,
        );
    }
    Ok(())
}

fn spawn_file_or_app_path(path: &Path, run_as_admin: bool) -> io::Result<()> {
    let path_str = path.to_string_lossy();
    let verb = if run_as_admin { "runas" } else { "open" };

    let verb_w = to_wide_null(verb);
    let file_w = to_wide_null(&path_str);

    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            verb_w.as_ptr(),
            file_w.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };

    if (result as isize) <= 32 {
        let ps_command = if run_as_admin {
            format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                escape_ps_single_quoted(&path_str)
            )
        } else {
            format!(
                "Start-Process -FilePath '{}'",
                escape_ps_single_quoted(&path_str)
            )
        };

        drop(
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &ps_command,
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?,
        );
    }

    Ok(())
}

fn should_hide_console(program: &str) -> bool {
    let lower = program.to_ascii_lowercase();
    !lower.ends_with("cmd.exe")
        && !lower.ends_with("powershell.exe")
        && !lower.ends_with("pwsh.exe")
        && !lower.ends_with("wt.exe")
}

fn escape_ps_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

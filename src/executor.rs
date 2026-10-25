use crate::config::AppConfig;
use crate::extensions::execute_extension_action;
use crate::models::{CandidateItem, ExecutionOutcome, ExecutionTarget};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::ptr::{null, null_mut};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const CONFIG_FILE_PATH: &str = "config.toml";

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
            // If opening a .txt or .md file that hasn't been created yet, create it first
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
        ExecutionTarget::ToggleStartup(enable) => {
            let mut cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            cfg.run_on_startup = *enable;
            cfg.save(CONFIG_FILE_PATH);
            crate::system::request_config_reload();
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::SetHotkey(new_hotkey) => {
            let mut cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            cfg.hotkey = new_hotkey.clone();
            cfg.save(CONFIG_FILE_PATH);
            crate::system::request_config_reload();
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::SetDefaultSearchEngine(engine_id) => {
            let mut cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            cfg.default_search_engine = engine_id.clone();
            cfg.save(CONFIG_FILE_PATH);
            crate::system::request_config_reload();
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::SwitchTheme(theme_mode) => {
            let mut cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            cfg.theme = theme_mode.clone();
            cfg.save(CONFIG_FILE_PATH);
            crate::system::request_config_reload();
            Ok(ExecutionOutcome::HidePalette)
        }
        ExecutionTarget::ReloadIndex => {
            crate::system::request_config_reload();
            Ok(ExecutionOutcome::ReloadAll)
        }
        ExecutionTarget::OpenPaletteSettings => Ok(ExecutionOutcome::OpenSettingsTab(0)),
        ExecutionTarget::OpenPaletteSettingsTab(tab) => {
            Ok(ExecutionOutcome::OpenSettingsTab(*tab))
        }
        ExecutionTarget::ActivateBang { bang, label } => Ok(ExecutionOutcome::ActivateBang {
            bang: bang.clone(),
            label: label.clone(),
        }),
        ExecutionTarget::ExtensionAction(action) => {
            let keep_open = execute_extension_action(action)?;
            if keep_open {
                Ok(ExecutionOutcome::KeepOpenAndRefresh { clear_query: true })
            } else {
                Ok(ExecutionOutcome::HidePalette)
            }
        }
        ExecutionTarget::NoOp => Ok(ExecutionOutcome::KeepOpenAndRefresh { clear_query: false }),
    }
}

fn copy_text_to_clipboard(text: &str) -> io::Result<()> {
    let mut child = Command::new("clip.exe")
        .stdin(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let _ = child.wait();
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
            .spawn()?;
    } else {
        Command::new(shell).args(["/C", trimmed]).spawn()?;
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
            .spawn()?;
    } else {
        let mut cmd = Command::new(trimmed_prog);
        cmd.args(args);
        if should_hide_console(trimmed_prog) {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.spawn()?;
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
    Command::new("cmd.exe")
        .args(["/C", "start", "", url_or_uri])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
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
            .spawn()?;
    }

    Ok(())
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
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

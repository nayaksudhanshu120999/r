use crate::models::{CandidateItem, ExecutionTarget};
use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn execute_candidate(item: &CandidateItem, run_as_admin: bool) -> io::Result<()> {
    match &item.target {
        ExecutionTarget::SystemProcess { program, args } => {
            spawn_system_process(program, args, run_as_admin)
        }
        ExecutionTarget::SettingsUri(uri_or_path) => spawn_settings_target(uri_or_path),
        ExecutionTarget::WebUrl(url) => spawn_shell_url(url),
        ExecutionTarget::AppPath(path) => spawn_app_path(path, run_as_admin),
        ExecutionTarget::OpenPaletteSettings => Ok(()),
    }
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
            let joined_args = args
                .iter()
                .map(|a| format!("'{}'", escape_ps_single_quoted(a)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Start-Process -FilePath '{}' -ArgumentList @({}) -Verb RunAs",
                escaped_prog, joined_args
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
        // Interactive console shells (cmd, powershell, wt, taskmgr, control) should show their window
        let lower = trimmed_prog.to_ascii_lowercase();
        let is_interactive = lower.contains("cmd")
            || lower.contains("powershell")
            || lower.contains("pwsh")
            || lower.contains("wt")
            || lower.contains("taskmgr")
            || lower.contains("control");

        let mut cmd = Command::new(trimmed_prog);
        cmd.args(args);
        if !is_interactive {
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

    if trimmed.starts_with("ms-settings:") {
        Command::new("explorer.exe")
            .arg(trimmed)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    } else if Path::new(trimmed).exists() || trimmed.ends_with(".cpl") || trimmed.ends_with(".msc") {
        Command::new("cmd.exe")
            .args(["/C", "start", "", trimmed])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    } else {
        Command::new("explorer.exe")
            .arg(trimmed)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }
    Ok(())
}

fn spawn_shell_url(url: &str) -> io::Result<()> {
    Command::new("explorer.exe")
        .arg(url)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

fn spawn_app_path(path: &Path, run_as_admin: bool) -> io::Result<()> {
    let path_str = path.to_string_lossy();

    if run_as_admin {
        let ps_command = format!(
            "Start-Process -FilePath '{}' -Verb RunAs",
            escape_ps_single_quoted(&path_str)
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
        Command::new("cmd.exe")
            .args(["/C", "start", "", &path_str])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }

    Ok(())
}

fn escape_ps_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

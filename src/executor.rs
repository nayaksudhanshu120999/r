use crate::models::{CandidateItem, ExecutionTarget};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn execute_candidate(item: &CandidateItem, run_as_admin: bool) -> Result<(), String> {
    if run_as_admin {
        return execute_elevated(&item.target);
    }
    execute_standard(&item.target)
}

fn execute_standard(target: &ExecutionTarget) -> Result<(), String> {
    match target {
        ExecutionTarget::SystemProcess { program, args } => {
            let mut cmd = Command::new(program);
            cmd.args(*args);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.spawn().map_err(|e| e.to_string())?;
            Ok(())
        }
        ExecutionTarget::SettingsUri(uri) => open_shell_target(uri),
        ExecutionTarget::WebUrl(url) => open_shell_target(url),
        ExecutionTarget::AppPath(path) => {
            let path_str = path.to_string_lossy();
            open_shell_target(&path_str)
        }
    }
}

fn open_shell_target(target_uri_or_path: &str) -> Result<(), String> {
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(target_uri_or_path);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn execute_elevated(target: &ExecutionTarget) -> Result<(), String> {
    let ps_script = match target {
        ExecutionTarget::SystemProcess { program, args } => {
            if args.is_empty() {
                format!(
                    "Start-Process -FilePath '{}' -Verb RunAs",
                    escape_ps_single_quotes(program)
                )
            } else {
                let joined_args = args.join(" ");
                format!(
                    "Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs",
                    escape_ps_single_quotes(program),
                    escape_ps_single_quotes(&joined_args)
                )
            }
        }
        ExecutionTarget::AppPath(path) => {
            let path_str = path.to_string_lossy();
            format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                escape_ps_single_quotes(&path_str)
            )
        }
        ExecutionTarget::SettingsUri(uri) => {
            format!(
                "Start-Process -FilePath 'explorer.exe' -ArgumentList '{}' -Verb RunAs",
                escape_ps_single_quotes(uri)
            )
        }
        ExecutionTarget::WebUrl(url) => {
            return open_shell_target(url);
        }
    };

    let mut cmd = Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &ps_script,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn escape_ps_single_quotes(input: &str) -> String {
    input.replace('\'', "''")
}

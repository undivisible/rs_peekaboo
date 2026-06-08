use crate::PeekabooError;
use crate::Result;
use crate::models::ShellOutput;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub stdout: String,
}

pub fn run(program: &str, args: &[&str], input: Option<&str>) -> Result<ProcessOutput> {
    let output = run_status(program, args, input, None)?;
    if !output.success {
        return Err(PeekabooError::CommandFailed {
            program: program.to_string(),
            status: output.status,
            stderr: output.stderr,
        });
    }
    Ok(ProcessOutput {
        stdout: output.stdout,
    })
}

pub fn run_status(
    program: &str,
    args: &[&str],
    input: Option<&str>,
    cwd: Option<&Path>,
) -> Result<ShellOutput> {
    let mut command = Command::new(program);
    command.args(args.iter().map(OsStr::new));
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn()?;
    if let Some(input) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok(ShellOutput {
        stdout,
        stderr,
        status,
        success: output.status.success(),
    })
}

pub fn probe(program: &str, args: &[&str]) -> bool {
    run(program, args, None).is_ok()
}

pub fn shell_program() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "/bin/sh"
    }
}

pub fn shell_args(command: &str) -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["/C".to_string(), command.to_string()]
    } else {
        vec!["-c".to_string(), command.to_string()]
    }
}

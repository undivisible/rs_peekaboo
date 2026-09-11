use crate::PeekabooError;
use crate::Result;
use crate::models::ShellOutput;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub stdout: String,
}

pub fn run(program: &str, args: &[&str], input: Option<&str>) -> Result<ProcessOutput> {
    run_with_timeout(program, args, input, None, None)
}

/// Run a process with an optional wall-clock timeout.
///
/// On timeout the child is killed and `PeekabooError::CommandFailed` is returned
/// so callers (e.g. Hybrid click AXPress) can fall back instead of hanging forever.
pub fn run_with_timeout(
    program: &str,
    args: &[&str],
    input: Option<&str>,
    cwd: Option<&Path>,
    timeout: Option<Duration>,
) -> Result<ProcessOutput> {
    let output = run_status_with_timeout(program, args, input, cwd, timeout)?;
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
    run_status_with_timeout(program, args, input, cwd, None)
}

pub fn run_status_with_timeout(
    program: &str,
    args: &[&str],
    input: Option<&str>,
    cwd: Option<&Path>,
    timeout: Option<Duration>,
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

    let Some(timeout) = timeout else {
        return shell_output_from_wait(child.wait_with_output()?);
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stdout.take() {
                    pipe.read_to_string(&mut stdout)?;
                }
                if let Some(mut pipe) = child.stderr.take() {
                    pipe.read_to_string(&mut stderr)?;
                }
                return Ok(ShellOutput {
                    stdout,
                    stderr,
                    status: status.code().unwrap_or(-1),
                    success: status.success(),
                });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let secs = timeout.as_secs_f64();
                return Err(PeekabooError::CommandFailed {
                    program: program.to_string(),
                    status: -1,
                    stderr: format!("timed out after {secs:.1}s"),
                });
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn shell_output_from_wait(output: std::process::Output) -> Result<ShellOutput> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_with_timeout_should_kill_hanging_process() {
        let err = run_with_timeout(
            "sleep",
            &["30"],
            None,
            None,
            Some(Duration::from_millis(200)),
        )
        .expect_err("sleep should time out");
        let msg = err.to_string();
        assert!(
            msg.contains("timed out") || msg.contains("sleep"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn run_with_timeout_should_allow_fast_process() {
        let out = run_with_timeout("printf", &["ok"], None, None, Some(Duration::from_secs(2)))
            .expect("printf should succeed");
        assert_eq!(out.stdout, "ok");
    }
}

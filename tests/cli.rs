use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn tools_should_print_catalog() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.arg("tools")
        .assert()
        .success()
        .stdout(predicate::str::contains("click"));
}

#[test]
fn help_should_include_core_commands() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("hotkey"));
}

#[test]
fn shell_should_return_stdout_and_status() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "shell", "echo shell-ok"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"stdout\": \"shell-ok\\n\""))
        .stdout(predicate::str::contains("\"status\": 0"))
        .stdout(predicate::str::contains("\"success\": true"));
}

#[test]
fn shell_should_report_nonzero_status_without_cli_failure() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "shell", "exit 7"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": 7"))
        .stdout(predicate::str::contains("\"success\": false"));
}

#[test]
fn completions_should_print_shell_script() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_rs-peekaboo"));
}

#[test]
fn json_should_emit_structured_error_for_invalid_image_mode() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "image", "--mode", "bogus"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains(
            "\"error\": \"invalid image mode: bogus\"",
        ));
}

#[test]
fn json_should_emit_structured_error_for_invalid_direction() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "scroll", "--direction", "sideways"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains(
            "\"error\": \"invalid direction: sideways\"",
        ));
}

#[test]
fn json_should_reject_invalid_snapshot_id_on_clean() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "clean", "--snapshot", "../etc/passwd"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains(
            "\"error\": \"invalid snapshot id: ../etc/passwd\"",
        ));
}

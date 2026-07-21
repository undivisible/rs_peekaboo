use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn tools_should_print_catalog() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.arg("tools")
        .assert()
        .success()
        .stdout(predicate::str::contains("click"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("mcp"));
}

#[test]
fn doctor_should_report_platform_health() {
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":"))
        .stdout(predicate::str::contains("capabilities"))
        .stdout(predicate::str::contains("permissions"));
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
    // Global --mode before subcommand sets ComputerUseMode; subcommand --mode sets ImageMode.
    // Without global mode, image --mode bogus hits ImageMode::parse_or_err.
    let mut cmd = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd.args(["--json", "image", "--mode", "bogus"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains(
            "\"error\": \"invalid image mode: bogus\"",
        ));
    // Global --mode before subcommand sets ComputerUseMode
    let mut cmd2 = Command::cargo_bin("rs-peekaboo").unwrap();
    cmd2.args(["--json", "--mode", "bogus", "image"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"ok\": false"))
        .stdout(predicate::str::contains(
            "\"error\": \"invalid mode: bogus, expected hybrid|native|vision|legacy|coords\"",
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

#[test]
fn release_workflow_should_package_notices_and_require_publish() {
    let workflow = include_str!("../.github/workflows/release.yml");
    assert!(workflow.contains("cargo install cargo-bundle-licenses --version 4.2.0 --locked"));
    assert!(workflow.contains("cargo bundle-licenses --format yaml --output THIRDPARTY.yml"));
    assert!(workflow.contains("cp LICENSE THIRDPARTY.yml"));
    assert!(workflow.contains("Copy-Item \"LICENSE\", \"THIRDPARTY.yml\""));
    assert!(workflow.contains("cargo info \"rs_peekaboo@${VERSION}\""));
    assert!(!workflow.contains("continue-on-error"));
}

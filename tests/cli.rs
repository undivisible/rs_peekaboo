use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn tools_should_print_catalog() {
    let mut cmd = Command::cargo_bin("rs_peekaboo").unwrap();
    cmd.arg("tools")
        .assert()
        .success()
        .stdout(predicate::str::contains("click"));
}

#[test]
fn help_should_include_core_commands() {
    let mut cmd = Command::cargo_bin("rs_peekaboo").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("hotkey"));
}

#[test]
fn completions_should_print_shell_script() {
    let mut cmd = Command::cargo_bin("rs_peekaboo").unwrap();
    cmd.args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_rs-peekaboo"));
}

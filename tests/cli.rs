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
fn release_workflow_should_package_notices_and_stage_assets() {
    let workflow = include_str!("../.github/workflows/release.yml");
    assert!(workflow.contains("cargo install cargo-bundle-licenses --version 4.2.0 --locked"));
    assert!(workflow.contains("cargo bundle-licenses --format yaml --output THIRDPARTY.yml"));
    assert!(workflow.contains("cp LICENSE THIRDPARTY.yml"));
    assert!(workflow.contains("Copy-Item \"LICENSE\", \"THIRDPARTY.yml\""));
    assert!(workflow.contains("  release:\n    needs: [validate]"));
    assert!(workflow.contains("  github_release:\n    needs: [release]"));
    assert!(!workflow.contains("cargo publish"));
    assert!(workflow.contains("if-no-files-found: error"));
    assert!(!workflow.contains("continue-on-error"));
}

#[cfg(target_os = "linux")]
#[test]
fn release_gate_rejects_wrong_tags_and_incomplete_or_corrupt_assets() {
    use std::process::Command as Process;

    let workflow = include_str!("../.github/workflows/release.yml");
    let script = |name: &str| {
        workflow
            .split(&format!("- name: {name}\n"))
            .nth(1)
            .unwrap()
            .split("        run: |\n")
            .nth(1)
            .unwrap()
            .lines()
            .take_while(|line| line.starts_with("          "))
            .map(|line| &line[10..])
            .collect::<Vec<_>>()
            .join("\n")
    };
    let validate = script("Verify tag matches package version");
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    for (candidate, success) in [(tag.as_str(), true), ("v0.0.0-invalid", false)] {
        let result = Process::new("bash")
            .args(["-e", "-o", "pipefail", "-c", &validate])
            .env("GITHUB_REF_NAME", candidate)
            .status()
            .unwrap();
        assert_eq!(result.success(), success, "tag {candidate}");
    }

    let dir = tempfile::tempdir().unwrap();
    let dist = dir.path().join("dist");
    std::fs::create_dir(&dist).unwrap();
    let targets = [
        ("aarch64-apple-darwin", "tar.gz"),
        ("x86_64-apple-darwin", "tar.gz"),
        ("x86_64-unknown-linux-gnu", "tar.gz"),
        ("x86_64-pc-windows-msvc", "zip"),
    ];
    let verify = script("Verify all release assets");
    let run = || {
        Process::new("bash")
            .args(["-e", "-o", "pipefail", "-c", &verify])
            .current_dir(dir.path())
            .env("GITHUB_REF_NAME", &tag)
            .output()
            .unwrap()
            .status
            .success()
    };
    assert!(!run(), "empty artifact directory must fail");
    for (target, suffix) in targets {
        let name = format!("rs-peekaboo-{tag}-{target}.{suffix}");
        std::fs::write(dist.join(&name), b"release artifact").unwrap();
        let checksum = Process::new("sha256sum")
            .arg(&name)
            .current_dir(&dist)
            .output()
            .unwrap();
        assert!(checksum.status.success());
        std::fs::write(dist.join(format!("{name}.sha256")), checksum.stdout).unwrap();
    }
    assert!(run(), "all four valid artifact/checksum pairs must pass");
    std::fs::write(
        dist.join(format!("rs-peekaboo-{tag}-x86_64-pc-windows-msvc.zip")),
        b"corrupt",
    )
    .unwrap();
    assert!(!run(), "corrupt artifact must fail");
}

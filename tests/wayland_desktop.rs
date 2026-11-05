#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn screens(
    runtime: &std::path::Path,
    session: &str,
    path: &std::path::Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rs-peekaboo"))
        .args(["--json", "list", "screens"])
        .env("XDG_SESSION_TYPE", session)
        .env("XDG_RUNTIME_DIR", runtime)
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env("DISPLAY", ":99")
        .env("PATH", path)
        .output()
        .unwrap()
}

#[test]
fn unreachable_wayland_is_an_error_not_an_x11_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let xrandr = dir.path().join("xrandr");
    std::fs::write(
        &xrandr,
        "#!/bin/sh\nprintf 'FAKE connected 999x888+0+0\\n'\n",
    )
    .unwrap();
    std::fs::set_permissions(&xrandr, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = screens(dir.path(), "wayland", dir.path());
    assert!(!output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["ok"], false);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("Wayland display enumeration failed")
    );

    let output = screens(dir.path(), "x11", dir.path());
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["screens"][0]["name"], "FAKE");
}

struct Compositor(Child);

impl Drop for Compositor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires Weston; CI explicitly runs this test on Linux"]
fn headless_weston_reports_logical_and_physical_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut compositor = Compositor(
        Command::new("weston")
            .args([
                "--backend=headless-backend.so",
                "--socket=wayland-test",
                "--width=1280",
                "--height=720",
                "--scale=2",
                "--idle-time=0",
            ])
            .env("XDG_RUNTIME_DIR", dir.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Weston is required for this test"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.path().join("wayland-test").exists() {
        assert!(
            compositor.0.try_wait().unwrap().is_none(),
            "Weston exited before creating its socket"
        );
        assert!(
            Instant::now() < deadline,
            "Weston did not create its socket"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = screens(dir.path(), "wayland", dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let screens = result["data"]["screens"].as_array().unwrap();
    assert_eq!(screens.len(), 1);
    let screen = &screens[0];
    assert_eq!(
        (screen["x"].as_i64(), screen["y"].as_i64()),
        (Some(0), Some(0))
    );
    assert_eq!(
        (screen["width"].as_i64(), screen["height"].as_i64()),
        (Some(1280), Some(720))
    );
    assert_eq!(screen["buffer_scale"], 2);
    assert_eq!(
        (
            screen["mode"]["width"].as_i64(),
            screen["mode"]["height"].as_i64()
        ),
        (Some(2560), Some(1440))
    );
}

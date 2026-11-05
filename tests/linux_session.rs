#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn child() {
    let Ok(action) = std::env::var("TEST_ACTION") else {
        return;
    };
    use rs_peekaboo::platform::linux;
    match action.as_str() {
        "screen" => linux::capture_image(
            rs_peekaboo::ImageMode::Screen,
            "/tmp/test image.png".as_ref(),
            false,
            None,
        )
        .unwrap(),
        "region" => linux::capture_image(
            rs_peekaboo::ImageMode::Screen,
            "/tmp/test image.png".as_ref(),
            false,
            Some(&rs_peekaboo::Bounds {
                x: -10,
                y: 20,
                width: 300,
                height: 400,
            }),
        )
        .unwrap(),
        "read" => {
            linux::clipboard_read().unwrap();
        }
        "write" => {
            linux::clipboard_write("hello").unwrap();
        }
        "write_fail" => {
            assert!(linux::clipboard_write("hello").is_err());
        }
        _ => unreachable!(),
    }
}

fn run(session: &str, action: &str, fail: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("log");
    for tool in ["grim", "scrot", "import", "wl-copy", "wl-paste", "xclip"] {
        if fail == "missing_scrot" && tool == "scrot" {
            continue;
        }
        let path = dir.path().join(tool);
        std::fs::write(&path, format!("#!/bin/sh\nif [ '{tool}' = grim ] && [ \"$1\" = --version ]; then exit 1; fi\nif [ \"$1\" = --version ] || [ \"$1\" = -version ] || [ \"$1\" = -h ]; then exit 0; fi\nif [ '{tool}' = wl-copy ] || {{ [ '{tool}' = xclip ] && {{ [ \"$TEST_ACTION\" = write ] || [ \"$TEST_ACTION\" = write_fail ]; }}; }}; then /bin/cat > /dev/null || exit 1; fi\nprintf '%s' '{tool}' >> \"$TEST_LOG\"\nprintf ' <%s>' \"$@\" >> \"$TEST_LOG\"\nprintf '\\n' >> \"$TEST_LOG\"\n[ '{tool}' != \"$FAIL_TOOL\" ]\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child"])
        .env("PATH", dir.path())
        .env("XDG_SESSION_TYPE", session)
        .env("DISPLAY", ":0")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("TEST_ACTION", action)
        .env("TEST_LOG", &log)
        .env("FAIL_TOOL", fail)
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::read_to_string(log).unwrap()
}

#[test]
fn x11_selects_x11_tools_when_both_families_exist() {
    assert_eq!(run("x11", "screen", ""), "scrot </tmp/test image.png>\n");
}

#[test]
fn x11_import_fallback_passes_separate_arguments() {
    assert_eq!(
        run("x11", "region", "missing_scrot"),
        "import <-window> <root> <-crop> <300x400-10+20> </tmp/test image.png>\n"
    );
}

#[test]
fn x11_clipboard_selects_xclip() {
    assert_eq!(
        run("x11", "read", ""),
        "xclip <-selection> <clipboard> <-o>\n"
    );
    assert_eq!(run("x11", "write", ""), "xclip <-selection> <clipboard>\n");
}

#[test]
fn wayland_grim_screen() {
    assert_eq!(run("wayland", "screen", ""), "grim </tmp/test image.png>\n");
}

#[test]
fn wayland_grim_region() {
    assert_eq!(
        run("wayland", "region", ""),
        "grim <-g> <-10,20 300x400> </tmp/test image.png>\n"
    );
}

#[test]
fn wayland_clipboard() {
    assert_eq!(run("wayland", "read", ""), "wl-paste <--no-newline>\n");
    assert_eq!(run("wayland", "write", ""), "wl-copy <>\n");
}

#[test]
fn missing_session_type_uses_wayland_display() {
    assert_eq!(run("", "screen", ""), "grim </tmp/test image.png>\n");
}

#[test]
fn failed_clipboard_write_is_not_retried() {
    assert_eq!(run("wayland", "write_fail", "wl-copy"), "wl-copy <>\n");
}

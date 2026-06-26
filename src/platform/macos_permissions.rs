use crate::Result;
use crate::platform::process;
use serde_json::{Value, json};

const SCREEN_RECORDING_PROBE_PATH: &str = "/tmp/rs_peekaboo_permission_probe.png";

pub fn permissions() -> Value {
    json!({
        "platform": "macos",
        "accessibility": check_accessibility(),
        "screen_recording": check_screen_recording(),
        "clipboard": process::probe("pbpaste", &[]),
        "recommended_mode": "hybrid",
    })
}

pub fn grant_permissions() -> Result<Value> {
    process::run(
        "open",
        &["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"],
        None,
    )?;
    let _ = process::run(
        "open",
        &["x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"],
        None,
    );
    Ok(json!({
        "opened": ["system_settings_accessibility", "system_settings_screen_recording"],
        "note": "Grant Accessibility and Screen Recording for rs-peekaboo in the opened settings panes."
    }))
}

pub fn check_accessibility() -> bool {
    probe_osascript("tell application \"System Events\" to get name of first process")
}

pub fn check_screen_recording() -> bool {
    let ok = process::probe("screencapture", &["-x", SCREEN_RECORDING_PROBE_PATH]);
    let _ = std::fs::remove_file(SCREEN_RECORDING_PROBE_PATH);
    ok
}

fn probe_osascript(script: &str) -> bool {
    process::run("osascript", &["-e", script], None).is_ok()
}

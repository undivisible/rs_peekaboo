//! macOS backend dispatcher.
//! Routes calls to the selected backend based on `ComputerUseMode`.
//! Default mode is Hybrid (AX first, fallback to CG/legacy).

use crate::Result;
use crate::models::{Bounds, ComputerUseMode, Direction, ImageMode, Point, UiElement, UiNode};
use crate::platform::{macos_ax, macos_cg, macos_legacy, macos_permissions};
use serde_json::Value;
use std::path::Path;

// ── mode dispatch helpers ──────────────────────────────────────────────

static CURRENT_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn get_mode() -> ComputerUseMode {
    match CURRENT_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => ComputerUseMode::Hybrid,
        1 => ComputerUseMode::Native,
        2 => ComputerUseMode::Vision,
        3 => ComputerUseMode::Legacy,
        4 => ComputerUseMode::Coords,
        _ => ComputerUseMode::Hybrid,
    }
}

pub fn set_mode(mode: ComputerUseMode) {
    let val = match mode {
        ComputerUseMode::Hybrid => 0,
        ComputerUseMode::Native => 1,
        ComputerUseMode::Vision => 2,
        ComputerUseMode::Legacy => 3,
        ComputerUseMode::Coords => 4,
    };
    CURRENT_MODE.store(val, std::sync::atomic::Ordering::Relaxed);
}

// ── capture ────────────────────────────────────────────────────────────

pub fn capture_image(
    mode: ImageMode,
    path: &Path,
    retina: bool,
    region: Option<&Bounds>,
) -> Result<()> {
    macos_cg::capture_image(mode, path, retina, region)
}

// ── UI elements ────────────────────────────────────────────────────────

pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiElement>> {
    match get_mode() {
        ComputerUseMode::Native | ComputerUseMode::Hybrid => {
            let nodes = macos_ax::ui_elements(app_filter)?;
            Ok(nodes.into_iter().map(UiElement::from).collect())
        }
        ComputerUseMode::Legacy | ComputerUseMode::Coords => macos_legacy::ui_elements(app_filter),
        ComputerUseMode::Vision => Ok(Vec::new()),
    }
}

pub fn element_at_point(point: Point) -> Option<UiNode> {
    macos_ax::element_at_point(point)
}

// ── screens ────────────────────────────────────────────────────────────

pub fn list_screens() -> Result<Value> {
    macos_legacy::list_screens()
}

// ── permissions ────────────────────────────────────────────────────────

pub fn permissions() -> Value {
    macos_permissions::permissions()
}

pub fn grant_permissions() -> Result<Value> {
    macos_permissions::grant_permissions()
}

// ── input ──────────────────────────────────────────────────────────────

pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    macos_legacy::click(point, button, count)
}

/// Prefer AX/background action for element targets; fall back to coords.
pub fn click_element(element: &UiElement, button: &str, count: u32) -> Result<Value> {
    match get_mode() {
        ComputerUseMode::Native | ComputerUseMode::Hybrid => {
            macos_ax::click_element(element, button, count)
        }
        _ => {
            let point = element
                .bounds
                .map(|b| b.center())
                .ok_or_else(|| crate::PeekabooError::TargetNotFound(element.id.clone()))?;
            macos_legacy::click(point, button, count)
        }
    }
}

pub fn doctor(mode: ComputerUseMode, backend: &str) -> Value {
    use serde_json::json;
    let permissions = permissions();
    let accessibility = permissions
        .get("accessibility")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let screen = permissions
        .get("screen_recording")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "platform": "macos",
        "mode": mode,
        "backend": backend,
        "permissions": permissions,
        "capabilities": {
            "background_click": true,
            "ax_tree": true,
            "element_index": true,
            "window_capture": true,
            "mcp": true
        },
        "tools": {
            "screencapture": std::path::Path::new("/usr/sbin/screencapture").exists()
                || crate::platform::process::probe("screencapture", &["-help"]),
            "osascript": crate::platform::process::probe("osascript", &["-e", "return 1"]),
            "pbpaste": crate::platform::process::probe("pbpaste", &[])
        },
        "ok": accessibility && screen
    })
}

pub fn move_cursor(point: Point) -> Result<Value> {
    macos_cg::move_cursor(point)
}

pub fn type_text(
    text: &str,
    clear: bool,
    press_return: bool,
    delay_ms: Option<u64>,
    app: Option<&str>,
) -> Result<Value> {
    macos_legacy::type_text(text, clear, press_return, delay_ms, app)
}

pub fn press(key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
    macos_legacy::press(key, count, delay_ms)
}

pub fn hotkey(keys: &[&str]) -> Result<Value> {
    macos_legacy::hotkey(keys)
}

pub fn paste(text: &str) -> Result<Value> {
    macos_legacy::paste(text)
}

pub fn scroll(direction: Direction, amount: u32) -> Result<Value> {
    macos_legacy::scroll(direction, amount)
}

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    macos_legacy::drag(from, to, duration_ms)
}

// ── actions on elements ────────────────────────────────────────────────

pub fn set_value(element: &UiElement, value: &str) -> Result<Value> {
    macos_legacy::set_value(element, value)
}

pub fn perform_action(element: &UiElement, action: &str) -> Result<Value> {
    macos_legacy::perform_action(element, action)
}

// ── app / window / menu ────────────────────────────────────────────────

pub fn list_apps() -> Result<Value> {
    macos_legacy::list_apps()
}

pub fn app(action: &str, name: Option<&str>) -> Result<Value> {
    macos_legacy::app(action, name)
}

pub fn open(path_or_url: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
    macos_legacy::open(path_or_url, app, no_focus)
}

pub fn window(
    action: &str,
    app: Option<&str>,
    title: Option<&str>,
    bounds: Option<Bounds>,
) -> Result<Value> {
    macos_legacy::window(action, app, title, bounds)
}

pub fn menu(action: &str, app: &str, menu: Option<&str>, item: Option<&str>) -> Result<Value> {
    macos_legacy::menu(action, app, menu, item)
}

// ── clipboard ──────────────────────────────────────────────────────────

pub fn clipboard_read() -> Result<String> {
    macos_legacy::clipboard_read()
}

pub fn clipboard_write(text: &str) -> Result<Value> {
    macos_legacy::clipboard_write(text)
}

// ── re-exports for tests ───────────────────────────────────────────────

pub use macos_legacy::{
    apple_string, element_process_name, element_ui_reference, parse_snapshot_line,
};

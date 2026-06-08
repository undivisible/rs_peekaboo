use crate::PeekabooError;
use crate::Result;
use crate::models::{Bounds, Direction, ImageMode, Point, UiElement};
use crate::platform::process::{self, ProcessOutput};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub fn capture_image(
    mode: ImageMode,
    path: &Path,
    retina: bool,
    region: Option<&Bounds>,
) -> Result<()> {
    let mut args = vec!["-x".to_string()];
    if mode == ImageMode::Window {
        args.push("-w".to_string());
    }
    if let Some(region) = region {
        args.push("-R".to_string());
        args.push(format!(
            "{},{},{},{}",
            region.x, region.y, region.width, region.height
        ));
    }
    if !retina {
        args.push("-r".to_string());
    }
    args.push(path.to_string_lossy().into_owned());
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    process::run("screencapture", &refs, None)?;
    Ok(())
}

pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiElement>> {
    let stdout = osascript(snapshot_script())?.stdout;
    Ok(stdout
        .lines()
        .filter_map(parse_snapshot_line)
        .filter(|element| app_filter.is_none_or(|app| element.app.eq_ignore_ascii_case(app)))
        .collect())
}

pub fn list_screens() -> Result<Value> {
    let output = process::run("system_profiler", &["SPDisplaysDataType", "-json"], None)?;
    Ok(serde_json::from_str(&output.stdout).unwrap_or_else(|_| {
        json!({
            "raw": output.stdout
        })
    }))
}

pub fn permissions() -> Value {
    json!({
        "platform": "macos",
        "screen_recording": process::probe(
            "screencapture",
            &["-x", "/tmp/rs_peekaboo_permission_probe.png"],
        ),
        "accessibility": probe_osascript(
            "tell application \"System Events\" to get name of first process",
        ),
        "clipboard": process::probe("pbpaste", &[]),
    })
}

pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    let click_count = count.max(1);
    for _ in 0..click_count {
        match button {
            "right" => osascript(&format!(
                "tell application \"System Events\" to right click at {{{}, {}}}",
                point.x, point.y
            ))?,
            _ => osascript(&format!(
                "tell application \"System Events\" to click at {{{}, {}}}",
                point.x, point.y
            ))?,
        };
    }
    Ok(json!({ "point": point, "button": button, "count": click_count }))
}

pub fn move_cursor(point: Point) -> Result<Value> {
    move_cursor_to(&point)?;
    Ok(json!({ "point": point }))
}

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    let script = format!(
        r#"tell application "System Events"
mouse down at {{{}, {}}}
delay {}
mouse up at {{{}, {}}}
end tell"#,
        from.x,
        from.y,
        duration_ms as f64 / 1000.0,
        to.x,
        to.y
    );
    osascript(&script)?;
    Ok(json!({ "from": from, "to": to, "duration_ms": duration_ms }))
}

pub fn scroll(direction: Direction, amount: u32) -> Result<Value> {
    let direction_name = match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    };
    osascript(&format!(
        "tell application \"System Events\" to scroll {direction_name} {}",
        amount.max(1)
    ))?;
    Ok(json!({ "direction": direction, "amount": amount.max(1) }))
}

pub fn press(key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
    let key = key_code_name(key);
    if key.is_empty() {
        return Err(PeekabooError::MissingArgument("key"));
    }
    let count = count.max(1);
    for index in 0..count {
        osascript(&format!(
            "tell application \"System Events\" to key code {key}"
        ))?;
        if index + 1 < count
            && let Some(delay_ms) = delay_ms
        {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }
    Ok(json!({ "key": key, "count": count }))
}

pub fn hotkey(keys: &[&str]) -> Result<Value> {
    let Some(key) = keys.last() else {
        return Err(PeekabooError::MissingArgument("keys"));
    };
    let modifiers = keys[..keys.len().saturating_sub(1)]
        .iter()
        .filter_map(|key| match key.to_ascii_lowercase().as_str() {
            "cmd" | "command" => Some("command down"),
            "ctrl" | "control" => Some("control down"),
            "alt" | "option" => Some("option down"),
            "shift" => Some("shift down"),
            _ => None,
        })
        .collect::<Vec<_>>();
    let script = if modifiers.is_empty() {
        format!(
            "tell application \"System Events\" to keystroke {}",
            apple_string(key)
        )
    } else {
        format!(
            "tell application \"System Events\" to keystroke {} using {{{}}}",
            apple_string(key),
            modifiers.join(", ")
        )
    };
    osascript(&script)?;
    Ok(json!({ "keys": keys }))
}

pub fn type_text(
    text: &str,
    clear: bool,
    press_return: bool,
    delay_ms: Option<u64>,
    _app: Option<&str>,
) -> Result<Value> {
    if clear {
        hotkey(&["cmd", "a"])?;
    }
    if let Some(delay_ms) = delay_ms {
        for ch in text.chars() {
            osascript(&format!(
                "tell application \"System Events\" to keystroke {}",
                apple_string(&ch.to_string())
            ))?;
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    } else {
        osascript(&format!(
            "tell application \"System Events\" to keystroke {}",
            apple_string(text)
        ))?;
    }
    if press_return {
        press("return", 1, None)?;
    }
    Ok(json!({ "typed": text.chars().count(), "return": press_return }))
}

pub fn paste(text: &str) -> Result<Value> {
    let previous = clipboard_read().ok();
    clipboard_write(text)?;
    hotkey(&["cmd", "v"])?;
    if let Some(previous) = previous {
        let _ = clipboard_write(&previous);
    }
    Ok(json!({ "pasted": text.len() }))
}

pub fn set_value(element: &UiElement, value: &str) -> Result<Value> {
    let script = format!(
        "tell application \"System Events\" to tell process {} to set value of UI element {} to {}",
        apple_string(&element.app),
        apple_string(&element.label),
        apple_string(value)
    );
    osascript(&script)?;
    Ok(json!({ "target": element.id, "value": value }))
}

pub fn perform_action(element: &UiElement, action: &str) -> Result<Value> {
    let script = format!(
        "tell application \"System Events\" to tell process {} to perform action {} of UI element {}",
        apple_string(&element.app),
        apple_string(action),
        apple_string(&element.label)
    );
    osascript(&script)?;
    Ok(json!({ "target": element.id, "action": action }))
}

pub fn list_apps() -> Result<Value> {
    let output = process::run("ps", &["ax", "-o", "pid=,comm="], None)?;
    let apps = output
        .stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (pid, command) = trimmed.split_once(' ')?;
            Some(json!({
                "pid": pid.trim().parse::<i64>().ok(),
                "command": command.trim()
            }))
        })
        .collect::<Vec<_>>();
    Ok(json!(apps))
}

pub fn app(action: &str, name: Option<&str>) -> Result<Value> {
    match action {
        "list" => list_apps(),
        "launch" | "switch" | "activate" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            process::run("open", &["-a", app], None)?;
            Ok(json!({ "app": app, "action": action }))
        }
        "quit" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            osascript(&format!("tell application {} to quit", apple_string(app)))?;
            Ok(json!({ "app": app, "action": action }))
        }
        "hide" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            osascript(&format!(
                "tell application \"System Events\" to set visible of process {} to false",
                apple_string(app)
            ))?;
            Ok(json!({ "app": app, "action": action }))
        }
        "unhide" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            osascript(&format!(
                "tell application \"System Events\" to set visible of process {} to true",
                apple_string(app)
            ))?;
            Ok(json!({ "app": app, "action": action }))
        }
        _ => Err(PeekabooError::MissingArgument("action")),
    }
}

pub fn open(path_or_url: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
    let mut args = Vec::new();
    if no_focus {
        args.push("-g".to_string());
    }
    if let Some(app) = app {
        args.push("-a".to_string());
        args.push(app.to_string());
    }
    args.push(path_or_url.to_string());
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    process::run("open", &refs, None)?;
    Ok(json!({ "opened": path_or_url, "app": app, "no_focus": no_focus }))
}

pub fn window(
    action: &str,
    app: Option<&str>,
    _title: Option<&str>,
    bounds: Option<Bounds>,
) -> Result<Value> {
    if action == "list" {
        return Ok(json!(ui_elements(None)?));
    }
    let app = app.ok_or(PeekabooError::MissingArgument("app"))?;
    match action {
        "focus" => {
            osascript(&format!(
                "tell application {} to activate",
                apple_string(app)
            ))?;
        }
        "close" => {
            osascript(&format!(
                "tell application {} to close front window",
                apple_string(app)
            ))?;
        }
        "minimize" => {
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set value of attribute \"AXMinimized\" of front window to true",
                apple_string(app)
            ))?;
        }
        "move" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set position of front window to {{{}, {}}}",
                apple_string(app),
                bounds.x,
                bounds.y
            ))?;
        }
        "resize" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set size of front window to {{{}, {}}}",
                apple_string(app),
                bounds.width,
                bounds.height
            ))?;
        }
        "set-bounds" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set position of front window to {{{}, {}}}",
                apple_string(app),
                bounds.x,
                bounds.y
            ))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set size of front window to {{{}, {}}}",
                apple_string(app),
                bounds.width,
                bounds.height
            ))?;
        }
        _ => return Err(PeekabooError::MissingArgument("action")),
    };
    Ok(json!({ "app": app, "action": action }))
}

pub fn menu(action: &str, app: &str, menu: Option<&str>, item: Option<&str>) -> Result<Value> {
    match action {
        "list" | "list-all" => {
            let script = format!(
                "tell application \"System Events\" to tell process {} to get name of menu bar items of menu bar 1",
                apple_string(app)
            );
            Ok(json!({ "app": app, "menus": osascript(&script)?.stdout.trim() }))
        }
        "click" => {
            let menu = menu.ok_or(PeekabooError::MissingArgument("menu"))?;
            let item = item.ok_or(PeekabooError::MissingArgument("item"))?;
            let script = format!(
                "tell application \"System Events\" to tell process {} to click menu item {} of menu {} of menu bar item {} of menu bar 1",
                apple_string(app),
                apple_string(item),
                apple_string(menu),
                apple_string(menu)
            );
            osascript(&script)?;
            Ok(json!({ "app": app, "menu": menu, "item": item }))
        }
        _ => Err(PeekabooError::MissingArgument("action")),
    }
}

pub fn clipboard_read() -> Result<String> {
    Ok(process::run("pbpaste", &[], None)?.stdout)
}

pub fn clipboard_write(text: &str) -> Result<Value> {
    process::run("pbcopy", &[], Some(text))?;
    Ok(json!({ "bytes": text.len() }))
}

pub fn parse_snapshot_line(line: &str) -> Option<UiElement> {
    let parts = line.split('\t').collect::<Vec<_>>();
    match parts.as_slice() {
        ["app", app, frontmost] => Some(UiElement {
            id: format!("app:{app}"),
            role: "application".to_string(),
            label: (*app).to_string(),
            app: (*app).to_string(),
            window: None,
            bounds: None,
            state: json!({ "frontmost": frontmost.eq_ignore_ascii_case("true") }),
        }),
        ["window", app, title, x, y, width, height, minimized] => Some(UiElement {
            id: format!("window:{app}:{title}"),
            role: "window".to_string(),
            label: (*title).to_string(),
            app: (*app).to_string(),
            window: Some((*title).to_string()),
            bounds: Some(Bounds {
                x: x.parse().ok()?,
                y: y.parse().ok()?,
                width: width.parse().ok()?,
                height: height.parse().ok()?,
            }),
            state: json!({ "minimized": minimized.eq_ignore_ascii_case("true") }),
        }),
        _ => None,
    }
}

fn snapshot_script() -> &'static str {
    r#"tell application "System Events"
set out to ""
repeat with p in (application processes whose background only is false)
set appName to name of p
set frontValue to frontmost of p as text
set out to out & "app" & tab & appName & tab & frontValue & "\n"
repeat with w in windows of p
try
set winName to name of w
set posValue to position of w
set sizeValue to size of w
set minimizedValue to false
try
set minimizedValue to value of attribute "AXMinimized" of w
end try
set out to out & "window" & tab & appName & tab & winName & tab & (item 1 of posValue as text) & tab & (item 2 of posValue as text) & tab & (item 1 of sizeValue as text) & tab & (item 2 of sizeValue as text) & tab & (minimizedValue as text) & "\n"
end try
end repeat
end repeat
return out
end tell"#
}

fn key_code_name(key: &str) -> &'static str {
    match key.to_ascii_lowercase().as_str() {
        "return" | "enter" => "36",
        "tab" => "48",
        "space" => "49",
        "escape" | "esc" => "53",
        "delete" | "backspace" => "51",
        "left" => "123",
        "right" => "124",
        "down" => "125",
        "up" => "126",
        _ => "",
    }
}

fn apple_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn osascript(script: &str) -> Result<ProcessOutput> {
    process::run("osascript", &["-e", script], None)
}

fn probe_osascript(script: &str) -> bool {
    osascript(script).is_ok()
}

fn move_cursor_to(point: &Point) -> Result<()> {
    post_mouse(CGEventType::MouseMoved, point, CGMouseButton::Left)
}

fn post_mouse(event_type: CGEventType, point: &Point, button: CGMouseButton) -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| PeekabooError::System("failed to create CoreGraphics event source".into()))?;
    let event = CGEvent::new_mouse_event(
        source,
        event_type,
        CGPoint::new(point.x as f64, point.y as f64),
        button,
    )
    .map_err(|_| PeekabooError::System("failed to create CoreGraphics mouse event".into()))?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}

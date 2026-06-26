use crate::{PeekabooError, Result};
use crate::models::UiElement;
use serde_json::{Value, json};
use crate::models::{Bounds, Direction, Point};
use crate::platform::process::{self, ProcessOutput};
use std::time::Duration;

// ── legacy (osascript) backend ──────────────────────────────────────────

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
        json!({ "raw": output.stdout })
    }))
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

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    let script = format!(
        r#"tell application "System Events"
mouse down at {{{}, {}}}
delay {}
mouse up at {{{}, {}}}
end tell"#,
        from.x, from.y,
        duration_ms as f64 / 1000.0,
        to.x, to.y
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
        osascript(&format!("tell application \"System Events\" to key code {key}"))?;
        if index + 1 < count && let Some(delay_ms) = delay_ms {
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
        format!("tell application \"System Events\" to keystroke {}", apple_string(key))
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
    let clipboard_restored = if let Some(previous) = previous {
        clipboard_write(&previous).is_ok()
    } else {
        true
    };
    Ok(json!({ "pasted": text.len(), "clipboard_restored": clipboard_restored }))
}

pub fn set_value(element: &UiElement, value: &str) -> Result<Value> {
    let script = format!(
        "tell application \"System Events\" to tell process {} to set value of {} to {}",
        apple_string(&element_process_name(element)),
        element_ui_reference(element),
        apple_string(value)
    );
    osascript(&script)?;
    Ok(json!({ "target": element.id, "value": value }))
}

pub fn perform_action(element: &UiElement, action: &str) -> Result<Value> {
    let script = format!(
        "tell application \"System Events\" to tell process {} to perform action {} of {}",
        apple_string(&element_process_name(element)),
        apple_string(action),
        element_ui_reference(element)
    );
    osascript(&script)?;
    Ok(json!({ "target": element.id, "action": action }))
}

pub fn list_apps() -> Result<Value> {
    let output = process::run("ps", &["ax", "-o", "pid=,comm="], None)?;
    let apps = output.stdout.lines()
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
    if no_focus { args.push("-g".to_string()); }
    if let Some(app) = app {
        args.push("-a".to_string());
        args.push(app.to_string());
    }
    args.push(path_or_url.to_string());
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    process::run("open", &refs, None)?;
    Ok(json!({ "opened": path_or_url, "app": app, "no_focus": no_focus }))
}

pub fn window(action: &str, app: Option<&str>, _title: Option<&str>, bounds: Option<Bounds>) -> Result<Value> {
    if action == "list" {
        return Ok(json!(ui_elements(None)?));
    }
    let app = app.ok_or(PeekabooError::MissingArgument("app"))?;
    match action {
        "focus" => {
            osascript(&format!("tell application {} to activate", apple_string(app)))?;
        }
        "close" => {
            osascript(&format!("tell application {} to close front window", apple_string(app)))?;
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
                apple_string(app), bounds.x, bounds.y
            ))?;
        }
        "resize" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set size of front window to {{{}, {}}}",
                apple_string(app), bounds.width, bounds.height
            ))?;
        }
        "set-bounds" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set position of front window to {{{}, {}}}",
                apple_string(app), bounds.x, bounds.y
            ))?;
            osascript(&format!(
                "tell application \"System Events\" to tell process {} to set size of front window to {{{}, {}}}",
                apple_string(app), bounds.width, bounds.height
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
                apple_string(app), apple_string(item), apple_string(menu), apple_string(menu)
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

pub fn apple_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub fn element_process_name(element: &UiElement) -> String {
    if let Some(name) = element.id.strip_prefix("app:") {
        return name.to_string();
    }
    if let Some(rest) = element.id.strip_prefix("window:") {
        if let Some((app, _)) = rest.split_once(':') {
            return app.to_string();
        }
    }
    element.app.clone()
}

pub fn element_ui_reference(element: &UiElement) -> String {
    if let Some(rest) = element.id.strip_prefix("window:") {
        if let Some((_app, title)) = rest.split_once(':') {
            let element_name = element_selector_name(element);
            return format!("UI element {} of window {}", apple_string(&element_name), apple_string(title));
        }
    }
    format!("UI element {}", apple_string(&element_selector_name(element)))
}

fn element_selector_name(element: &UiElement) -> String {
    if !element.label.is_empty() {
        return element.label.clone();
    }
    if let Some(rest) = element.id.strip_prefix("window:") {
        if let Some((_app, title)) = rest.split_once(':') {
            return title.to_string();
        }
    }
    if let Some(name) = element.id.strip_prefix("app:") {
        return name.to_string();
    }
    if !is_generic_element_id(&element.id) {
        return element.id.clone();
    }
    element.label.clone()
}

fn is_generic_element_id(id: &str) -> bool {
    id.starts_with("win-") || id.is_empty()
}

fn osascript(script: &str) -> Result<ProcessOutput> {
    process::run("osascript", &["-e", script], None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Bounds;

    fn sample_window_element() -> UiElement {
        UiElement {
            id: "window:Finder:Desktop".to_string(),
            role: "window".to_string(),
            label: "Desktop".to_string(),
            app: "Finder".to_string(),
            window: Some("Desktop".to_string()),
            bounds: Some(Bounds { x: 0, y: 0, width: 100, height: 100 }),
            state: json!({}),
        }
    }

    #[test]
    fn apple_string_should_escape_quotes_and_backslashes() {
        assert_eq!(apple_string(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(apple_string(r"path\to\file"), r#""path\\to\\file""#);
    }

    #[test]
    fn parse_snapshot_line_should_parse_app_and_window_rows() {
        let app = parse_snapshot_line("app\tSafari\ttrue").expect("app row");
        assert_eq!(app.id, "app:Safari");
        assert_eq!(app.role, "application");
        assert_eq!(app.label, "Safari");

        let window = parse_snapshot_line("window\tSafari\tStart Page\t10\t20\t800\t600\tfalse").expect("window row");
        assert_eq!(window.id, "window:Safari:Start Page");
        assert_eq!(window.label, "Start Page");
        assert_eq!(window.bounds, Some(Bounds { x: 10, y: 20, width: 800, height: 600 }));
    }

    #[test]
    fn element_process_name_should_prefer_stable_id() {
        let element = sample_window_element();
        assert_eq!(element_process_name(&element), "Finder");
        let app = parse_snapshot_line("app\tNotes\tfalse").expect("app row");
        assert_eq!(element_process_name(&app), "Notes");
    }

    #[test]
    fn element_ui_reference_should_scope_windows_by_id() {
        let element = sample_window_element();
        assert_eq!(element_ui_reference(&element), r#"UI element "Desktop" of window "Desktop""#);
        let app = parse_snapshot_line("app\tNotes\tfalse").expect("app row");
        assert_eq!(element_ui_reference(&app), r#"UI element "Notes""#);
    }

    #[test]
    fn element_ui_reference_should_fall_back_to_id_when_label_missing() {
        let element = UiElement {
            id: "window:Mail:Inbox".to_string(),
            role: "window".to_string(),
            label: String::new(),
            app: "Mail".to_string(),
            window: Some("Inbox".to_string()),
            bounds: None,
            state: json!({}),
        };
        assert_eq!(element_ui_reference(&element), r#"UI element "Inbox" of window "Inbox""#);
    }
}

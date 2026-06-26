use crate::PeekabooError;
use crate::Result;
use crate::models::{Bounds, Direction, ImageMode, Point, UiElement};
use crate::platform::process;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub fn capture_image(
    mode: ImageMode,
    path: &Path,
    _retina: bool,
    region: Option<&Bounds>,
) -> Result<()> {
    if mode != ImageMode::Screen {
        return Err(PeekabooError::UnsupportedPlatform("image mode"));
    }
    if process::probe("grim", &["--version"]) {
        let mut args = vec!["-o".to_string(), path.to_string_lossy().into_owned()];
        if let Some(bounds) = region {
            args.push("-g".to_string());
            args.push(format!(
                "{},{},{},{}",
                bounds.x, bounds.y, bounds.width, bounds.height
            ));
        }
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        process::run("grim", &refs, None)?;
        return Ok(());
    }
    if process::probe("scrot", &["--version"]) {
        if let Some(bounds) = region {
            process::run(
                "scrot",
                &[
                    "-a",
                    &format!(
                        "{},{},{},{}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    ),
                    path.to_string_lossy().as_ref(),
                ],
                None,
            )?;
        } else {
            process::run("scrot", &[path.to_string_lossy().as_ref()], None)?;
        }
        return Ok(());
    }
    if process::probe("import", &["-version"]) {
        let target = if let Some(bounds) = region {
            format!(
                "-window root -crop {}x{}+{}+{}",
                bounds.width, bounds.height, bounds.x, bounds.y
            )
        } else {
            "-window root".to_string()
        };
        process::run(
            "import",
            &[target.as_str(), path.to_string_lossy().as_ref()],
            None,
        )?;
        return Ok(());
    }
    Err(PeekabooError::System(
        "no screenshot tool found; install grim, scrot, or imagemagick".into(),
    ))
}

pub fn ui_elements(app: Option<&str>) -> Result<Vec<UiElement>> {
    require_tool("wmctrl", &["-m"])?;
    let output = process::run("wmctrl", &["-l"], None)?;
    let mut elements = Vec::new();
    for (index, line) in output.stdout.lines().enumerate() {
        let Some((window_id, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let _desktop = parts.next();
        let pid = parts.next().and_then(|value| value.parse::<i64>().ok());
        let title = parts.collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            continue;
        }
        if app.is_some_and(|filter| {
            !title
                .to_ascii_lowercase()
                .contains(&filter.to_ascii_lowercase())
        }) {
            continue;
        }
        let bounds = window_bounds(window_id).ok();
        elements.push(UiElement {
            id: format!("linux-{index}"),
            role: "window".to_string(),
            label: title.clone(),
            app: title.clone(),
            window: Some(title),
            bounds,
            state: json!({ "window_id": window_id, "pid": pid }),
        });
    }
    Ok(elements)
}

pub fn list_screens() -> Result<Value> {
    if process::probe("xrandr", &["--version"]) {
        let output = process::run("xrandr", &["--query"], None)?;
        let screens = output
            .stdout
            .lines()
            .filter(|line| line.contains(" connected"))
            .map(|line| {
                let name = line
                    .split_whitespace()
                    .next()
                    .unwrap_or("screen")
                    .to_string();
                json!({ "name": name, "raw": line })
            })
            .collect::<Vec<_>>();
        return Ok(json!({ "screens": screens }));
    }
    Ok(json!({ "screens": [], "note": "install xrandr for display metadata" }))
}

pub fn permissions() -> Value {
    json!({
        "platform": "linux",
        "screen_recording": process::probe("grim", &["--version"])
            || process::probe("scrot", &["--version"])
            || process::probe("import", &["-version"]),
        "accessibility": process::probe("xdotool", &["--version"]),
        "clipboard": process::probe("wl-copy", &["--version"])
            || process::probe("xclip", &["-version"])
    })
}

pub fn grant_permissions() -> Result<Value> {
    Ok(json!({
        "note": "Install xdotool, grim/scrot, and wl-clipboard/xclip; grant desktop access in your compositor session.",
        "platform": "linux"
    }))
}

pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    require_xdotool()?;
    process::run(
        "xdotool",
        &[
            "mousemove",
            "--sync",
            &point.x.to_string(),
            &point.y.to_string(),
        ],
        None,
    )?;
    let button_arg = if button == "right" { "3" } else { "1" };
    for _ in 0..count.max(1) {
        process::run("xdotool", &["click", button_arg], None)?;
    }
    Ok(json!({ "point": point, "button": button, "count": count.max(1) }))
}

pub fn move_cursor(point: Point) -> Result<Value> {
    require_xdotool()?;
    process::run(
        "xdotool",
        &[
            "mousemove",
            "--sync",
            &point.x.to_string(),
            &point.y.to_string(),
        ],
        None,
    )?;
    Ok(json!({ "point": point }))
}

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    require_xdotool()?;
    process::run(
        "xdotool",
        &[
            "mousemove",
            "--sync",
            &from.x.to_string(),
            &from.y.to_string(),
        ],
        None,
    )?;
    process::run("xdotool", &["mousedown", "1"], None)?;
    std::thread::sleep(Duration::from_millis(duration_ms.max(50)));
    process::run(
        "xdotool",
        &["mousemove", "--sync", &to.x.to_string(), &to.y.to_string()],
        None,
    )?;
    process::run("xdotool", &["mouseup", "1"], None)?;
    Ok(json!({ "from": from, "to": to, "duration_ms": duration_ms }))
}

pub fn scroll(direction: Direction, amount: u32) -> Result<Value> {
    require_xdotool()?;
    let button = match direction {
        Direction::Up => "4",
        Direction::Down => "5",
        Direction::Left => "6",
        Direction::Right => "7",
    };
    let repeats = amount.max(1).to_string();
    process::run("xdotool", &["click", "--repeat", &repeats, button], None)?;
    Ok(json!({ "direction": direction, "amount": amount.max(1) }))
}

pub fn press(key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
    require_xdotool()?;
    let key_name = xdotool_key_name(key);
    let count = count.max(1);
    for index in 0..count {
        process::run("xdotool", &["key", "--clearmodifiers", &key_name], None)?;
        if index + 1 < count
            && let Some(delay_ms) = delay_ms
        {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }
    Ok(json!({ "key": key_name, "count": count }))
}

pub fn hotkey(keys: &[&str]) -> Result<Value> {
    require_xdotool()?;
    let chord = keys
        .iter()
        .map(|key| xdotool_modifier(key))
        .collect::<Vec<_>>()
        .join("+");
    process::run("xdotool", &["key", "--clearmodifiers", &chord], None)?;
    Ok(json!({ "keys": keys }))
}

pub fn type_text(
    text: &str,
    clear: bool,
    press_return: bool,
    delay_ms: Option<u64>,
    _app: Option<&str>,
) -> Result<Value> {
    require_xdotool()?;
    if clear {
        process::run("xdotool", &["key", "--clearmodifiers", "ctrl+a"], None)?;
        process::run("xdotool", &["key", "--clearmodifiers", "BackSpace"], None)?;
    }
    if let Some(delay_ms) = delay_ms {
        for ch in text.chars() {
            process::run(
                "xdotool",
                &["type", "--clearmodifiers", "--delay", "0", &ch.to_string()],
                None,
            )?;
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    } else if text.chars().count() <= 1 {
        process::run("xdotool", &["type", "--clearmodifiers", "--", text], None)?;
    } else {
        clipboard_write(text)?;
        hotkey(&["ctrl", "v"])?;
    }
    if press_return {
        press("return", 1, None)?;
    }
    Ok(json!({ "typed": text.chars().count(), "return": press_return }))
}

pub fn paste(text: &str) -> Result<Value> {
    let previous = clipboard_read().ok();
    clipboard_write(text)?;
    hotkey(&["ctrl", "v"])?;
    let clipboard_restored = if let Some(previous) = previous {
        clipboard_write(&previous).is_ok()
    } else {
        true
    };
    Ok(json!({
        "pasted": text.len(),
        "clipboard_restored": clipboard_restored
    }))
}

pub fn set_value(element: &UiElement, value: &str) -> Result<Value> {
    let point = element
        .bounds
        .map(|b| b.center())
        .unwrap_or(Point { x: 0, y: 0 });
    click(point, "left", 1)?;
    type_text(value, true, false, None, None)
}

pub fn perform_action(element: &UiElement, action: &str) -> Result<Value> {
    let point = element
        .bounds
        .map(|b| b.center())
        .unwrap_or(Point { x: 0, y: 0 });
    match action {
        "right_click" | "right-click" => click(point, "right", 1),
        "double_click" | "double-click" | "open" | "press" => click(point, "left", 2),
        _ => click(point, "left", 1),
    }
}

pub fn list_apps() -> Result<Value> {
    let output = process::run("ps", &["-eo", "pid=,comm="], None)?;
    let apps = output
        .stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (pid, command) = trimmed.split_once(char::is_whitespace)?;
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
        "launch" | "open" | "switch" | "activate" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            process::run("xdg-open", &[app], None)?;
            Ok(json!({ "app": app, "action": action }))
        }
        "quit" | "close" => {
            let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
            if process::probe("wmctrl", &["-m"]) {
                process::run("wmctrl", &["-c", app], None)?;
            } else if process::probe("xdotool", &["--version"]) {
                process::run("xdotool", &["search", "--name", app, "windowclose"], None)?;
            } else {
                return Err(PeekabooError::System(
                    "install wmctrl or xdotool to close windows".into(),
                ));
            }
            Ok(json!({ "app": app, "action": action }))
        }
        "hide" => Err(PeekabooError::UnsupportedPlatform("hide")),
        "unhide" => Err(PeekabooError::UnsupportedPlatform("unhide")),
        _ => Err(PeekabooError::MissingArgument("action")),
    }
}

pub fn open(path_or_url: &str, app: Option<&str>, _no_focus: bool) -> Result<Value> {
    if let Some(app) = app {
        process::run(app, &[path_or_url], None)?;
    } else {
        process::run("xdg-open", &[path_or_url], None)?;
    }
    Ok(json!({ "opened": path_or_url, "app": app }))
}

pub fn window(
    action: &str,
    app: Option<&str>,
    title: Option<&str>,
    bounds: Option<Bounds>,
) -> Result<Value> {
    if action == "list" {
        return Ok(json!(ui_elements(app)?));
    }
    require_tool("wmctrl", &["-m"])?;
    let query = title
        .or(app)
        .ok_or(PeekabooError::MissingArgument("app"))?
        .to_string();
    match action {
        "focus" | "activate" | "switch" => {
            process::run("wmctrl", &["-a", &query], None)?;
        }
        "close" => {
            process::run("wmctrl", &["-c", &query], None)?;
        }
        "minimize" => {
            if process::probe("xdotool", &["--version"]) {
                let window_id = find_window_id(&query)?;
                process::run("xdotool", &["windowminimize", &window_id], None)?;
            } else {
                return Err(PeekabooError::System(
                    "install xdotool to minimize windows".into(),
                ));
            }
        }
        "move" | "resize" | "set-bounds" => {
            let bounds = bounds.ok_or(PeekabooError::MissingArgument("bounds"))?;
            let window_id = find_window_id(&query)?;
            let gravity = "0";
            process::run(
                "wmctrl",
                &[
                    "-i",
                    "-r",
                    &window_id,
                    "-e",
                    &format!(
                        "{gravity},{},{},{},{}",
                        bounds.x, bounds.y, bounds.width, bounds.height
                    ),
                ],
                None,
            )?;
        }
        _ => return Err(PeekabooError::MissingArgument("action")),
    }
    Ok(json!({ "app": app, "title": title, "action": action }))
}

pub fn menu(action: &str, app: &str, menu: Option<&str>, item: Option<&str>) -> Result<Value> {
    match action {
        "list" | "list-all" | "inspect" => Ok(json!({
            "app": app,
            "menus": [],
            "note": "Linux menu inspection is limited; use see snapshots for window context."
        })),
        "click" => {
            let menu = menu.ok_or(PeekabooError::MissingArgument("menu"))?;
            let item = item.ok_or(PeekabooError::MissingArgument("item"))?;
            if process::probe("xdotool", &["--version"]) {
                process::run("xdotool", &["key", "--clearmodifiers", "alt"], None)?;
                for entry in menu
                    .split('>')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                {
                    process::run("xdotool", &["type", "--", entry], None)?;
                    process::run("xdotool", &["key", "Return"], None)?;
                }
                process::run("xdotool", &["type", "--", item], None)?;
                process::run("xdotool", &["key", "Return"], None)?;
            } else {
                return Err(PeekabooError::System(
                    "install xdotool to click menus".into(),
                ));
            }
            Ok(json!({ "app": app, "menu": menu, "item": item }))
        }
        _ => Err(PeekabooError::MissingArgument("action")),
    }
}

pub fn clipboard_read() -> Result<String> {
    if process::probe("wl-paste", &["--version"]) {
        return Ok(process::run("wl-paste", &["--no-newline"], None)?.stdout);
    }
    if process::probe("xclip", &["-version"]) {
        return Ok(process::run("xclip", &["-selection", "clipboard", "-o"], None)?.stdout);
    }
    Err(PeekabooError::System(
        "no clipboard tool found; install wl-clipboard or xclip".into(),
    ))
}

pub fn clipboard_write(text: &str) -> Result<Value> {
    if process::probe("wl-copy", &["--version"]) {
        process::run("wl-copy", &[], Some(text))?;
    } else if process::probe("xclip", &["-version"]) {
        process::run("xclip", &["-selection", "clipboard"], Some(text))?;
    } else {
        return Err(PeekabooError::System(
            "no clipboard tool found; install wl-clipboard or xclip".into(),
        ));
    }
    Ok(json!({ "bytes": text.len() }))
}

fn require_xdotool() -> Result<()> {
    require_tool("xdotool", &["--version"])
}

fn require_tool(program: &str, args: &[&str]) -> Result<()> {
    if process::probe(program, args) {
        Ok(())
    } else {
        Err(PeekabooError::System(format!(
            "{program} is required for this action on Linux"
        )))
    }
}

fn find_window_id(query: &str) -> Result<String> {
    let output = process::run("wmctrl", &["-l"], None)?;
    for line in output.stdout.lines() {
        if line
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
        {
            let window_id = line.split_whitespace().next().unwrap_or_default();
            if !window_id.is_empty() {
                return Ok(window_id.to_string());
            }
        }
    }
    Err(PeekabooError::TargetNotFound(query.to_string()))
}

fn window_bounds(window_id: &str) -> Result<Bounds> {
    require_xdotool()?;
    let output = process::run(
        "xdotool",
        &["getwindowgeometry", "--shell", window_id],
        None,
    )?;
    let mut x = 0_i64;
    let mut y = 0_i64;
    let mut width = 0_i64;
    let mut height = 0_i64;
    for line in output.stdout.lines() {
        if let Some(value) = line.strip_prefix("X=") {
            x = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("Y=") {
            y = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("WIDTH=") {
            width = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("HEIGHT=") {
            height = value.parse().unwrap_or(0);
        }
    }
    Ok(Bounds {
        x,
        y,
        width,
        height,
    })
}

fn xdotool_modifier(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "cmd" | "command" | "super" | "meta" | "win" | "windows" => "super".to_string(),
        "ctrl" | "control" => "ctrl".to_string(),
        "alt" | "option" => "alt".to_string(),
        "shift" => "shift".to_string(),
        other => other.to_string(),
    }
}

fn xdotool_key_name(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "return" | "enter" => "Return".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "space".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "backspace" => "BackSpace".to_string(),
        "left" | "arrowleft" => "Left".to_string(),
        "right" | "arrowright" => "Right".to_string(),
        "up" | "arrowup" => "Up".to_string(),
        "down" | "arrowdown" => "Down".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "Page_Up".to_string(),
        "pagedown" => "Page_Down".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdotool_key_name_should_normalize_common_keys() {
        assert_eq!(xdotool_key_name("enter"), "Return");
        assert_eq!(xdotool_key_name("escape"), "Escape");
    }

    #[test]
    fn xdotool_modifier_should_map_super_keys() {
        assert_eq!(xdotool_modifier("cmd"), "super");
        assert_eq!(xdotool_modifier("ctrl"), "ctrl");
    }
}

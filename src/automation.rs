use crate::cache;
use crate::error::{PeekabooError, Result};
use crate::models::*;
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::Builder;

#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
#[cfg(target_os = "macos")]
use core_graphics::geometry::CGPoint;

#[derive(Clone, Debug, Default)]
pub struct Peekaboo;

impl Peekaboo {
    pub fn new() -> Self {
        Self
    }

    pub fn image(
        &self,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<ImageCapture> {
        require_macos("image")?;
        let path = match path {
            Some(path) => expand_home(path),
            None => Builder::new()
                .prefix("rs_peekaboo_")
                .suffix(".png")
                .tempfile()?
                .into_temp_path()
                .keep()
                .map_err(|err| err.error)?,
        };
        let mut args = vec!["-x".to_string()];
        if mode == ImageMode::Window {
            args.push("-w".to_string());
        }
        if !retina {
            args.push("-r".to_string());
        }
        args.push(path.to_string_lossy().into_owned());
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        run("screencapture", &refs, None)?;
        let bytes = std::fs::metadata(&path)?.len();
        Ok(ImageCapture {
            path,
            mode,
            bytes,
            mime_type: "image/png".to_string(),
        })
    }

    pub fn see(
        &self,
        app: Option<&str>,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<Snapshot> {
        let _ = self.image(mode, path, retina)?;
        let snapshot_id = cache::new_snapshot_id();
        let elements = self.ui_elements(app)?;
        let snapshot = Snapshot {
            snapshot_id,
            elements,
        };
        cache::save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn ui_elements(&self, app_filter: Option<&str>) -> Result<Vec<UiElement>> {
        require_macos("see")?;
        let stdout = osascript(snapshot_script())?.stdout;
        let elements = stdout
            .lines()
            .filter_map(parse_snapshot_line)
            .filter(|element| app_filter.is_none_or(|app| element.app.eq_ignore_ascii_case(app)))
            .collect::<Vec<_>>();
        Ok(elements)
    }

    pub fn list_apps(&self) -> Result<Value> {
        let output = run("ps", &["ax", "-o", "pid=,comm="], None)?;
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

    pub fn list_windows(&self) -> Result<Value> {
        Ok(json!(self.ui_elements(None)?))
    }

    pub fn list_screens(&self) -> Result<Value> {
        require_macos("list screens")?;
        let output = run("system_profiler", &["SPDisplaysDataType", "-json"], None)?;
        let parsed = serde_json::from_str::<Value>(&output.stdout).unwrap_or_else(|_| {
            json!({
                "raw": output.stdout
            })
        });
        Ok(parsed)
    }

    pub fn permissions(&self) -> Value {
        json!({
            "platform": std::env::consts::OS,
            "screen_recording": probe("screencapture", &["-x", "/tmp/rs_peekaboo_permission_probe.png"]),
            "accessibility": probe_osascript("tell application \"System Events\" to get name of first process"),
            "clipboard": probe("pbpaste", &[])
        })
    }

    pub fn click(&self, target: Target, button: &str, count: u32) -> Result<Value> {
        require_macos("click")?;
        let point = self.resolve_target(target)?;
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

    pub fn move_cursor(&self, target: Target) -> Result<Value> {
        require_macos("move")?;
        let point = self.resolve_target(target)?;
        move_cursor_to(&point)?;
        Ok(json!({ "point": point }))
    }

    pub fn type_text(
        &self,
        text: &str,
        clear: bool,
        press_return: bool,
        delay_ms: Option<u64>,
    ) -> Result<Value> {
        require_macos("type")?;
        if clear {
            self.hotkey(&["cmd", "a"])?;
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
            self.press("return", 1, None)?;
        }
        Ok(json!({ "typed": text.chars().count(), "return": press_return }))
    }

    pub fn press(&self, key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
        require_macos("press")?;
        let key = key_code_name(key);
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

    pub fn hotkey(&self, keys: &[&str]) -> Result<Value> {
        require_macos("hotkey")?;
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

    pub fn paste(&self, text: &str) -> Result<Value> {
        require_macos("paste")?;
        let previous = self.clipboard_read().ok();
        self.clipboard_write(text)?;
        self.hotkey(&["cmd", "v"])?;
        if let Some(previous) = previous {
            let _ = self.clipboard_write(&previous);
        }
        Ok(json!({ "pasted": text.len() }))
    }

    pub fn scroll(&self, direction: Direction, amount: u32) -> Result<Value> {
        require_macos("scroll")?;
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

    pub fn drag(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        require_macos("drag")?;
        let from = self.resolve_target(from)?;
        let to = self.resolve_target(to)?;
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

    pub fn swipe(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        self.drag(from, to, duration_ms)
    }

    pub fn set_value(&self, target: Target, value: &str) -> Result<Value> {
        require_macos("set-value")?;
        let element = self.resolve_element(target)?;
        let script = format!(
            "tell application \"System Events\" to tell process {} to set value of UI element {} to {}",
            apple_string(&element.app),
            apple_string(&element.label),
            apple_string(value)
        );
        osascript(&script)?;
        Ok(json!({ "target": element.id, "value": value }))
    }

    pub fn perform_action(&self, target: Target, action: &str) -> Result<Value> {
        require_macos("perform-action")?;
        let element = self.resolve_element(target)?;
        let script = format!(
            "tell application \"System Events\" to tell process {} to perform action {} of UI element {}",
            apple_string(&element.app),
            apple_string(action),
            apple_string(&element.label)
        );
        osascript(&script)?;
        Ok(json!({ "target": element.id, "action": action }))
    }

    pub fn app(&self, action: &str, name: Option<&str>) -> Result<Value> {
        require_macos("app")?;
        match action {
            "list" => self.list_apps(),
            "launch" | "switch" | "activate" => {
                let app = name.ok_or(PeekabooError::MissingArgument("app"))?;
                run("open", &["-a", app], None)?;
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

    pub fn open(&self, path_or_url: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
        require_macos("open")?;
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
        run("open", &refs, None)?;
        Ok(json!({ "opened": path_or_url, "app": app, "no_focus": no_focus }))
    }

    pub fn window(&self, action: &str, app: Option<&str>, bounds: Option<Bounds>) -> Result<Value> {
        require_macos("window")?;
        if action == "list" {
            return self.list_windows();
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

    pub fn menu(
        &self,
        action: &str,
        app: &str,
        menu: Option<&str>,
        item: Option<&str>,
    ) -> Result<Value> {
        require_macos("menu")?;
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

    pub fn clipboard_read(&self) -> Result<String> {
        require_macos("clipboard")?;
        Ok(run("pbpaste", &[], None)?.stdout)
    }

    pub fn clipboard_write(&self, text: &str) -> Result<Value> {
        require_macos("clipboard")?;
        run("pbcopy", &[], Some(text))?;
        Ok(json!({ "bytes": text.len() }))
    }

    pub fn run_file(&self, path: &Path) -> Result<Vec<Value>> {
        let data = std::fs::read(path)?;
        let file = serde_json::from_slice::<RunFile>(&data)?;
        let mut results = Vec::with_capacity(file.steps.len());
        for step in file.steps {
            results.push(self.run_step(&step.command, step.args)?);
        }
        Ok(results)
    }

    fn run_step(&self, command: &str, args: Value) -> Result<Value> {
        match command {
            "sleep" => {
                let duration_ms = args.get("duration_ms").and_then(Value::as_u64).unwrap_or(0);
                std::thread::sleep(Duration::from_millis(duration_ms));
                Ok(json!({ "slept_ms": duration_ms }))
            }
            "hotkey" => {
                let keys = args
                    .get("keys")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("keys"))?;
                let parts = split_keys(keys);
                self.hotkey(&parts)
            }
            "type" => {
                let text = args
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("text"))?;
                self.type_text(text, false, false, None)
            }
            "click" => {
                let coords = args
                    .get("coords")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("coords"))?;
                self.click(Target::Point(parse_point(coords)?), "left", 1)
            }
            _ => Err(PeekabooError::MissingArgument("command")),
        }
    }

    fn resolve_target(&self, target: Target) -> Result<Point> {
        match target {
            Target::Point(point) => Ok(point),
            Target::Element(element) => {
                element.bounds.map(|bounds| bounds.center()).ok_or_else(|| {
                    PeekabooError::TargetNotFound(format!("{} has no bounds", element.id))
                })
            }
            Target::Query { query, snapshot } => {
                let element = self.resolve_element(Target::Query { query, snapshot })?;
                element.bounds.map(|bounds| bounds.center()).ok_or_else(|| {
                    PeekabooError::TargetNotFound(format!("{} has no bounds", element.id))
                })
            }
        }
    }

    fn resolve_element(&self, target: Target) -> Result<UiElement> {
        match target {
            Target::Element(element) => Ok(element),
            Target::Point(_) => Err(PeekabooError::MissingArgument("element target")),
            Target::Query { query, snapshot } => {
                let snapshot = if let Some(snapshot) = snapshot {
                    cache::load_snapshot(&snapshot)?
                } else {
                    Snapshot {
                        snapshot_id: "live".to_string(),
                        elements: self.ui_elements(None)?,
                    }
                };
                snapshot
                    .elements
                    .into_iter()
                    .find(|element| {
                        element.id == query
                            || element.label.eq_ignore_ascii_case(&query)
                            || element
                                .label
                                .to_ascii_lowercase()
                                .contains(&query.to_ascii_lowercase())
                    })
                    .ok_or(PeekabooError::TargetNotFound(query))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Target {
    Point(Point),
    Query {
        query: String,
        snapshot: Option<String>,
    },
    Element(UiElement),
}

pub fn parse_point(value: &str) -> Result<Point> {
    let Some((x, y)) = value.split_once(',') else {
        return Err(PeekabooError::InvalidCoordinates(value.to_string()));
    };
    Ok(Point {
        x: x.trim()
            .parse::<i64>()
            .map_err(|_| PeekabooError::InvalidCoordinates(value.to_string()))?,
        y: y.trim()
            .parse::<i64>()
            .map_err(|_| PeekabooError::InvalidCoordinates(value.to_string()))?,
    })
}

pub fn split_keys(value: &str) -> Vec<&str> {
    value
        .split([',', '+'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_snapshot_line(line: &str) -> Option<UiElement> {
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
        _ => "36",
    }
}

fn apple_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn osascript(script: &str) -> Result<ProcessOutput> {
    run("osascript", &["-e", script], None)
}

fn run(program: &str, args: &[&str], input: Option<&str>) -> Result<ProcessOutput> {
    let mut command = Command::new(program);
    command.args(args.iter().map(OsStr::new));
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn()?;
    if let Some(input) = input
        && let Some(mut stdin) = child.stdin.take()
    {
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    let status = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(PeekabooError::CommandFailed {
            program: program.to_string(),
            status,
            stderr,
        });
    }
    Ok(ProcessOutput { stdout })
}

#[derive(Clone, Debug)]
struct ProcessOutput {
    stdout: String,
}

fn probe(program: &str, args: &[&str]) -> bool {
    run(program, args, None).is_ok()
}

fn probe_osascript(script: &str) -> bool {
    osascript(script).is_ok()
}

#[cfg(target_os = "macos")]
fn move_cursor_to(point: &Point) -> Result<()> {
    post_mouse(CGEventType::MouseMoved, point, CGMouseButton::Left)
}

#[cfg(not(target_os = "macos"))]
fn move_cursor_to(_point: &Point) -> Result<()> {
    Err(PeekabooError::UnsupportedPlatform("move"))
}

#[cfg(target_os = "macos")]
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

fn require_macos(command: &'static str) -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(PeekabooError::UnsupportedPlatform(command))
    }
}

fn expand_home(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_point_should_accept_comma_pair() {
        let point = parse_point("10, 20").unwrap();
        assert_eq!(point, Point { x: 10, y: 20 });
    }

    #[test]
    fn split_keys_should_accept_commas_and_pluses() {
        assert_eq!(split_keys("cmd,shift+t"), vec!["cmd", "shift", "t"]);
    }
}

use crate::cache;
use crate::error::{PeekabooError, Result};
use crate::models::*;
use crate::platform::{backend, process};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::Builder;

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
        self.capture_image(mode, path, retina, None)
    }

    pub fn image_region(
        &self,
        bounds: Bounds,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<ImageCapture> {
        self.capture_image(ImageMode::Screen, path, retina, Some(bounds))
    }

    fn capture_image(
        &self,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
        region: Option<Bounds>,
    ) -> Result<ImageCapture> {
        let (path, ephemeral) = match path {
            Some(path) => (expand_home(path)?, false),
            None => {
                // Temp file is kept on disk; caller is responsible for cleanup.
                let path = Builder::new()
                    .prefix("rs_peekaboo_")
                    .suffix(".png")
                    .tempfile()?
                    .into_temp_path()
                    .keep()
                    .map_err(|err| err.error)?;
                (path, true)
            }
        };
        backend::capture_image(mode, &path, retina, region.as_ref())?;
        let bytes = std::fs::metadata(&path)?.len();
        Ok(ImageCapture {
            path,
            mode,
            bytes,
            mime_type: "image/png".to_string(),
            ephemeral,
        })
    }

    pub fn see(
        &self,
        app: Option<&str>,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<Snapshot> {
        if path.is_some() {
            let _ = self.image(mode, path, retina)?;
        }
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
        backend::ui_elements(app_filter)
    }

    pub fn list_apps(&self) -> Result<Value> {
        backend::list_apps()
    }

    pub fn list_windows(&self) -> Result<Value> {
        Ok(json!(self.ui_elements(None)?))
    }

    pub fn list_screens(&self) -> Result<Value> {
        backend::list_screens()
    }

    pub fn shell(&self, command: &str, cwd: Option<&Path>) -> Result<ShellOutput> {
        let args = process::shell_args(command);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        process::run_status(process::shell_program(), &refs, None, cwd)
    }

    pub fn permissions(&self) -> Value {
        backend::permissions()
    }

    pub fn grant_permissions(&self) -> Result<Value> {
        backend::grant_permissions()
    }

    pub fn click(&self, target: Target, button: &str, count: u32) -> Result<Value> {
        let point = self.resolve_target(target)?;
        backend::click(point, button, count)
    }

    pub fn move_cursor(&self, target: Target) -> Result<Value> {
        let point = self.resolve_target(target)?;
        backend::move_cursor(point)
    }

    pub fn type_text(
        &self,
        text: &str,
        clear: bool,
        press_return: bool,
        delay_ms: Option<u64>,
        app: Option<&str>,
    ) -> Result<Value> {
        backend::type_text(text, clear, press_return, delay_ms, app)
    }

    pub fn press(&self, key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
        backend::press(key, count, delay_ms)
    }

    pub fn hotkey(&self, keys: &[&str]) -> Result<Value> {
        backend::hotkey(keys)
    }

    pub fn paste(&self, text: &str) -> Result<Value> {
        backend::paste(text)
    }

    pub fn scroll(&self, direction: Direction, amount: u32) -> Result<Value> {
        backend::scroll(direction, amount)
    }

    pub fn drag(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        let from = self.resolve_target(from)?;
        let to = self.resolve_target(to)?;
        backend::drag(from, to, duration_ms)
    }

    pub fn swipe(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        self.drag(from, to, duration_ms)
    }

    pub fn set_value(&self, target: Target, value: &str) -> Result<Value> {
        #[cfg(target_os = "macos")]
        {
            let element = self.resolve_element(target)?;
            backend::set_value(&element, value)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let point = self.resolve_target(target)?;
            backend::set_value(point, value)
        }
    }

    pub fn perform_action(&self, target: Target, action: &str) -> Result<Value> {
        #[cfg(target_os = "macos")]
        {
            let element = self.resolve_element(target)?;
            backend::perform_action(&element, action)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let point = self.resolve_target(target)?;
            backend::perform_action(point, action)
        }
    }

    pub fn app(&self, action: &str, name: Option<&str>) -> Result<Value> {
        backend::app(action, name)
    }

    pub fn open(&self, path_or_url: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
        backend::open(path_or_url, app, no_focus)
    }

    pub fn window(
        &self,
        action: &str,
        app: Option<&str>,
        title: Option<&str>,
        bounds: Option<Bounds>,
    ) -> Result<Value> {
        backend::window(action, app, title, bounds)
    }

    pub fn menu(
        &self,
        action: &str,
        app: &str,
        menu: Option<&str>,
        item: Option<&str>,
    ) -> Result<Value> {
        backend::menu(action, app, menu, item)
    }

    pub fn clipboard_read(&self) -> Result<String> {
        backend::clipboard_read()
    }

    pub fn clipboard_write(&self, text: &str) -> Result<Value> {
        backend::clipboard_write(text)
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
                let keys = required_str(&args, "keys")?;
                let parts = split_keys(keys);
                self.hotkey(&parts)
            }
            "type" => {
                let text = required_str(&args, "text")?;
                self.type_text(
                    text,
                    args.get("clear").and_then(Value::as_bool).unwrap_or(false),
                    args.get("return")
                        .or_else(|| args.get("press_return"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    args.get("delay_ms").and_then(Value::as_u64),
                    args.get("app").and_then(Value::as_str),
                )
            }
            "press" => {
                let key = required_str(&args, "key")?;
                self.press(
                    key,
                    args.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
                    args.get("delay_ms").and_then(Value::as_u64),
                )
            }
            "click" => self.click(
                run_target(&args)?,
                args.get("button").and_then(Value::as_str).unwrap_or("left"),
                args.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
            ),
            "move" => self.move_cursor(run_target(&args)?),
            "paste" => self.paste(required_str(&args, "text")?),
            "scroll" => self.scroll(
                Direction::parse_or_err(
                    args.get("direction")
                        .and_then(Value::as_str)
                        .unwrap_or("down"),
                )?,
                args.get("amount").and_then(Value::as_u64).unwrap_or(3) as u32,
            ),
            "drag" | "swipe" => {
                let from = required_str(&args, "from")?;
                let to = required_str(&args, "to")?;
                let duration_ms = args.get("duration_ms").and_then(Value::as_u64).unwrap_or(250);
                self.drag(
                    Target::Point(parse_point(from)?),
                    Target::Point(parse_point(to)?),
                    duration_ms,
                )
            }
            "see" => Ok(serde_json::to_value(self.see(
                args.get("app").and_then(Value::as_str),
                ImageMode::parse_or_err(
                    args.get("mode").and_then(Value::as_str).unwrap_or("screen"),
                )?,
                optional_output_path(&args)?,
                args.get("retina").and_then(Value::as_bool).unwrap_or(false),
            )?)?),
            "image" => Ok(serde_json::to_value(self.image(
                ImageMode::parse_or_err(
                    args.get("mode").and_then(Value::as_str).unwrap_or("screen"),
                )?,
                optional_output_path(&args)?,
                args.get("retina").and_then(Value::as_bool).unwrap_or(false),
            )?)?),
            "set-value" => self.set_value(
                Target::Query {
                    query: required_str(&args, "on")?.to_string(),
                    snapshot: args
                        .get("snapshot")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                required_str(&args, "value")?,
            ),
            "perform-action" => self.perform_action(
                Target::Query {
                    query: required_str(&args, "on")?.to_string(),
                    snapshot: args
                        .get("snapshot")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                required_str(&args, "action")?,
            ),
            "window" => run_window(self, &args),
            "app" => run_app(self, &args),
            "open" => self.open(
                required_str(&args, "target")?,
                args.get("app").and_then(Value::as_str),
                args.get("no_focus").and_then(Value::as_bool).unwrap_or(false),
            ),
            "menu" => run_menu(self, &args),
            "clipboard" => run_clipboard(self, &args),
            "permissions" => Ok(match args.get("action").and_then(Value::as_str) {
                Some("grant") => self.grant_permissions()?,
                _ => self.permissions(),
            }),
            "clean" => Ok(json!({
                "removed": cache::clean_snapshots(
                    args.get("all_snapshots").and_then(Value::as_bool).unwrap_or(false),
                    args.get("snapshot").and_then(Value::as_str),
                )?,
            })),
            "list" => Ok(match required_str(&args, "kind")? {
                "apps" => self.list_apps()?,
                "windows" => self.list_windows()?,
                "screens" => self.list_screens()?,
                kind => return Err(PeekabooError::UnsupportedRunCommand(format!("list: {kind}"))),
            }),
            "shell" => {
                let command = required_str(&args, "command")?;
                let cwd = args.get("cwd").and_then(Value::as_str).map(Path::new);
                Ok(serde_json::to_value(self.shell(command, cwd)?)?)
            }
            other => Err(PeekabooError::UnsupportedRunCommand(other.to_string())),
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
                resolve_query(&snapshot.elements, &query)
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

fn required_str<'a>(args: &'a Value, key: &'static str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or(PeekabooError::MissingArgument(key))
}

fn run_target(args: &Value) -> Result<Target> {
    if let Some(coords) = args.get("coords").and_then(Value::as_str) {
        return Ok(Target::Point(parse_point(coords)?));
    }
    let query = args
        .get("on")
        .or_else(|| args.get("target"))
        .and_then(Value::as_str)
        .ok_or(PeekabooError::MissingArgument("target"))?;
    Ok(Target::Query {
        query: query.to_string(),
        snapshot: args
            .get("snapshot")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn resolve_query(elements: &[UiElement], query: &str) -> Result<UiElement> {
    if let Some(element) = elements.iter().find(|element| element.id == query) {
        return Ok(element.clone());
    }

    let exact_label = elements
        .iter()
        .filter(|element| element.label.eq_ignore_ascii_case(query))
        .collect::<Vec<_>>();
    if exact_label.len() == 1 {
        return Ok(exact_label[0].clone());
    }
    if exact_label.len() > 1 {
        return Err(PeekabooError::AmbiguousTarget(format!(
            "{query} matched {} elements by label",
            exact_label.len()
        )));
    }

    let query_lower = query.to_ascii_lowercase();
    let partial = elements
        .iter()
        .filter(|element| element.label.to_ascii_lowercase().contains(&query_lower))
        .collect::<Vec<_>>();
    match partial.len() {
        0 => Err(PeekabooError::TargetNotFound(query.to_string())),
        1 => Ok(partial[0].clone()),
        count => Err(PeekabooError::AmbiguousTarget(format!(
            "{query} matched {count} elements"
        ))),
    }
}

pub fn validate_output_path(path: &Path) -> Result<PathBuf> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(PeekabooError::System(
            "path traversal ('..') is not allowed".into(),
        ));
    }

    let expanded = expand_home_raw(path)?;
    let canonical = canonicalize_output_path(&expanded)?;

    let text = path.to_string_lossy();
    if (text == "~" || text.starts_with("~/"))
        && let Some(home) = dirs::home_dir()
    {
        let home_canonical = home
            .canonicalize()
            .unwrap_or_else(|_| home.clone());
        if !canonical.starts_with(&home_canonical) {
            return Err(PeekabooError::System(
                "path escapes home directory".into(),
            ));
        }
    }

    Ok(canonical)
}

fn expand_home(path: PathBuf) -> Result<PathBuf> {
    validate_output_path(&path)
}

fn expand_home_raw(path: &Path) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" {
        dirs::home_dir().ok_or_else(|| PeekabooError::System("home directory not found".into()))
    } else if let Some(rest) = text.strip_prefix("~/") {
        let home =
            dirs::home_dir().ok_or_else(|| PeekabooError::System("home directory not found".into()))?;
        Ok(home.join(rest))
    } else {
        Ok(path.to_path_buf())
    }
}

fn canonicalize_output_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path.canonicalize().map_err(PeekabooError::from);
    }

    let Some(parent) = path.parent() else {
        return Ok(path.to_path_buf());
    };

    if parent.as_os_str().is_empty() {
        return Ok(std::env::current_dir()?.join(path));
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| PeekabooError::System("invalid output path".into()))?;
    let canonical_parent = canonicalize_output_path(parent)?;
    Ok(canonical_parent.join(file_name))
}

fn optional_output_path(args: &Value) -> Result<Option<PathBuf>> {
    match args.get("path").and_then(Value::as_str) {
        Some(path) => Ok(Some(expand_home(PathBuf::from(path))?)),
        None => Ok(None),
    }
}

fn run_bounds(args: &Value) -> Bounds {
    Bounds {
        x: args.get("x").and_then(Value::as_i64).unwrap_or(0),
        y: args.get("y").and_then(Value::as_i64).unwrap_or(0),
        width: args.get("width").and_then(Value::as_i64).unwrap_or(0),
        height: args.get("height").and_then(Value::as_i64).unwrap_or(0),
    }
}

fn run_window(peekaboo: &Peekaboo, args: &Value) -> Result<Value> {
    let action = required_str(args, "action")?;
    let app = args.get("app").and_then(Value::as_str);
    match action {
        "list" => peekaboo.window("list", None, None, None),
        "focus" => peekaboo.window("focus", app, None, None),
        "close" => peekaboo.window("close", app, None, None),
        "minimize" => peekaboo.window("minimize", app, None, None),
        "move" | "resize" | "set-bounds" => {
            let app = app.ok_or(PeekabooError::MissingArgument("app"))?;
            peekaboo.window(action, Some(app), None, Some(run_bounds(args)))
        }
        other => Err(PeekabooError::UnsupportedRunCommand(other.to_string())),
    }
}

fn run_app(peekaboo: &Peekaboo, args: &Value) -> Result<Value> {
    let action = required_str(args, "action")?;
    let app = args.get("app").and_then(Value::as_str);
    match action {
        "list" => peekaboo.app("list", None),
        "launch" | "quit" | "hide" | "unhide" | "switch" => {
            let app = app.ok_or(PeekabooError::MissingArgument("app"))?;
            peekaboo.app(action, Some(app))
        }
        other => Err(PeekabooError::UnsupportedRunCommand(other.to_string())),
    }
}

fn run_menu(peekaboo: &Peekaboo, args: &Value) -> Result<Value> {
    let action = required_str(args, "action")?;
    let app = required_str(args, "app")?;
    match action {
        "list" => peekaboo.menu("list", app, None, None),
        "list-all" => peekaboo.menu("list-all", app, None, None),
        "click" => peekaboo.menu(
            "click",
            app,
            Some(required_str(args, "menu")?),
            Some(required_str(args, "item")?),
        ),
        other => Err(PeekabooError::UnsupportedRunCommand(other.to_string())),
    }
}

fn run_clipboard(peekaboo: &Peekaboo, args: &Value) -> Result<Value> {
    match args.get("action").and_then(Value::as_str).unwrap_or("read") {
        "write" => peekaboo.clipboard_write(required_str(args, "text")?),
        "read" => Ok(json!({ "text": peekaboo.clipboard_read()? })),
        other => Err(PeekabooError::UnsupportedRunCommand(other.to_string())),
    }
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

    #[test]
    fn required_str_should_require_argument() {
        assert!(required_str(&json!({}), "text").is_err());
        assert_eq!(required_str(&json!({ "text": "hi" }), "text").unwrap(), "hi");
    }

    #[test]
    fn run_target_should_accept_coords() {
        let target = run_target(&json!({ "coords": "10,20" })).expect("coords target");
        match target {
            Target::Point(point) => assert_eq!(point, Point { x: 10, y: 20 }),
            _ => panic!("expected point target"),
        }
    }

    #[test]
    fn validate_output_path_should_reject_parent_dir_segments() {
        assert!(validate_output_path(Path::new("~/../../tmp/x.png")).is_err());
        assert!(validate_output_path(Path::new("../../tmp/x.png")).is_err());
    }

    #[test]
    fn expand_home_should_reject_traversal_via_tilde_prefix() {
        assert!(expand_home(PathBuf::from("~/../../tmp/x.png")).is_err());
    }

    #[test]
    fn resolve_query_should_fail_on_ambiguous_partial_match() {
        let elements = vec![
            UiElement {
                id: "a".to_string(),
                role: "button".to_string(),
                label: "Save draft".to_string(),
                app: "App".to_string(),
                window: None,
                bounds: None,
                state: serde_json::json!({}),
            },
            UiElement {
                id: "b".to_string(),
                role: "button".to_string(),
                label: "Save file".to_string(),
                app: "App".to_string(),
                window: None,
                bounds: None,
                state: serde_json::json!({}),
            },
        ];

        assert!(resolve_query(&elements, "Save").is_err());
        assert_eq!(
            resolve_query(&elements, "Save draft")
                .expect("exact label")
                .id,
            "a"
        );
    }
}

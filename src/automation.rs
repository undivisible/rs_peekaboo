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
        backend::capture_image(mode, &path, retina, region.as_ref())?;
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
                self.type_text(text, false, false, None, None)
            }
            "click" => {
                let coords = args
                    .get("coords")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("coords"))?;
                self.click(Target::Point(parse_point(coords)?), "left", 1)
            }
            "shell" => {
                let command = args
                    .get("command")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("command"))?;
                let cwd = args.get("cwd").and_then(Value::as_str).map(Path::new);
                Ok(serde_json::to_value(self.shell(command, cwd)?)?)
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

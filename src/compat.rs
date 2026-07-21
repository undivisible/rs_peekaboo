use crate::{
    Bounds, ComputerUseMode, Direction, ImageCapture, ImageMode, Point, Result, ShellOutput,
    automation::{Peekaboo as CurrentPeekaboo, PeekabooConfig, Target as CurrentTarget},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiElement {
    pub id: String,
    pub role: String,
    pub label: String,
    pub app: String,
    pub window: Option<String>,
    pub bounds: Option<Bounds>,
    pub state: Value,
}

impl From<crate::UiElement> for UiElement {
    fn from(element: crate::UiElement) -> Self {
        Self {
            id: element.id,
            role: element.role,
            label: element.label,
            app: element.app,
            window: element.window,
            bounds: element.bounds,
            state: element.state,
        }
    }
}

impl From<UiElement> for crate::UiElement {
    fn from(element: UiElement) -> Self {
        Self {
            id: element.id,
            role: element.role,
            label: element.label,
            app: element.app,
            window: element.window,
            bounds: element.bounds,
            state: element.state,
            index: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub elements: Vec<UiElement>,
}

impl From<crate::Snapshot> for Snapshot {
    fn from(snapshot: crate::Snapshot) -> Self {
        Self {
            snapshot_id: snapshot.snapshot_id,
            elements: snapshot.elements.into_iter().map(Into::into).collect(),
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

impl From<Target> for CurrentTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Point(point) => Self::Point(point),
            Target::Query { query, snapshot } => Self::Query { query, snapshot },
            Target::Element(element) => {
                Self::Element(crate::UiNode::from(crate::UiElement::from(element)))
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Peekaboo;

impl Peekaboo {
    pub fn new() -> Self {
        Self
    }

    fn current(&self) -> CurrentPeekaboo {
        CurrentPeekaboo::with_config(PeekabooConfig {
            mode: ComputerUseMode::Legacy,
            background: false,
        })
    }

    fn resolve_target(&self, target: Target) -> Result<CurrentTarget> {
        match target {
            Target::Point(point) => Ok(CurrentTarget::Point(point)),
            Target::Element(element) => Ok(CurrentTarget::Element(crate::UiNode::from(
                crate::UiElement::from(element),
            ))),
            Target::Query { query, snapshot } => {
                let elements = match snapshot {
                    Some(snapshot) => crate::cache::load_snapshot(&snapshot)?.elements,
                    None => self.current().ui_elements(None)?,
                };
                resolve_query(&elements, &query)
                    .map(|element| CurrentTarget::Element(crate::UiNode::from(element)))
            }
        }
    }

    pub fn image(
        &self,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<ImageCapture> {
        self.current().image(mode, path, retina)
    }

    pub fn image_region(
        &self,
        bounds: Bounds,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<ImageCapture> {
        self.current().image_region(bounds, path, retina)
    }

    pub fn see(
        &self,
        app: Option<&str>,
        mode: ImageMode,
        path: Option<PathBuf>,
        retina: bool,
    ) -> Result<Snapshot> {
        self.current().see(app, mode, path, retina).map(Into::into)
    }

    pub fn ui_elements(&self, app_filter: Option<&str>) -> Result<Vec<UiElement>> {
        self.current()
            .ui_elements(app_filter)
            .map(|elements| elements.into_iter().map(Into::into).collect())
    }

    pub fn list_apps(&self) -> Result<Value> {
        self.current().list_apps()
    }

    pub fn list_windows(&self) -> Result<Value> {
        Ok(serde_json::to_value(self.ui_elements(None)?)?)
    }

    pub fn list_screens(&self) -> Result<Value> {
        self.current().list_screens()
    }

    pub fn shell(&self, command: &str, cwd: Option<&Path>) -> Result<ShellOutput> {
        self.current().shell(command, cwd)
    }

    pub fn permissions(&self) -> Value {
        self.current().permissions()
    }

    pub fn grant_permissions(&self) -> Result<Value> {
        self.current().grant_permissions()
    }

    pub fn click(&self, target: Target, button: &str, count: u32) -> Result<Value> {
        self.current()
            .click(self.resolve_target(target)?, button, count)
    }

    pub fn move_cursor(&self, target: Target) -> Result<Value> {
        self.current().move_cursor(self.resolve_target(target)?)
    }

    pub fn type_text(
        &self,
        text: &str,
        clear: bool,
        press_return: bool,
        delay_ms: Option<u64>,
        app: Option<&str>,
    ) -> Result<Value> {
        self.current()
            .type_text(text, clear, press_return, delay_ms, app)
    }

    pub fn press(&self, key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
        self.current().press(key, count, delay_ms)
    }

    pub fn hotkey(&self, keys: &[&str]) -> Result<Value> {
        self.current().hotkey(keys)
    }

    pub fn paste(&self, text: &str) -> Result<Value> {
        self.current().paste(text)
    }

    pub fn scroll(&self, direction: Direction, amount: u32) -> Result<Value> {
        self.current().scroll(direction, amount)
    }

    pub fn drag(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        self.current().drag(
            self.resolve_target(from)?,
            self.resolve_target(to)?,
            duration_ms,
        )
    }

    pub fn swipe(&self, from: Target, to: Target, duration_ms: u64) -> Result<Value> {
        self.current().swipe(
            self.resolve_target(from)?,
            self.resolve_target(to)?,
            duration_ms,
        )
    }

    pub fn set_value(&self, target: Target, value: &str) -> Result<Value> {
        #[cfg(not(target_os = "macos"))]
        {
            if let Target::Point(point) = &target {
                self.current()
                    .click(CurrentTarget::Point(point.clone()), "left", 1)?;
                return self.current().type_text(value, true, false, None, None);
            }
        }
        self.current()
            .set_value(self.resolve_target(target)?, value)
    }

    pub fn perform_action(&self, target: Target, action: &str) -> Result<Value> {
        #[cfg(not(target_os = "macos"))]
        {
            if let Target::Point(point) = &target {
                let (button, count) = legacy_action_click(action);
                return self
                    .current()
                    .click(CurrentTarget::Point(point.clone()), button, count);
            }
        }
        self.current()
            .perform_action(self.resolve_target(target)?, action)
    }

    pub fn app(&self, action: &str, name: Option<&str>) -> Result<Value> {
        self.current().app(action, name)
    }

    pub fn open(&self, path_or_url: &str, app: Option<&str>, no_focus: bool) -> Result<Value> {
        self.current().open(path_or_url, app, no_focus)
    }

    pub fn window(
        &self,
        action: &str,
        app: Option<&str>,
        title: Option<&str>,
        bounds: Option<Bounds>,
    ) -> Result<Value> {
        self.current().window(action, app, title, bounds)
    }

    pub fn menu(
        &self,
        action: &str,
        app: &str,
        menu: Option<&str>,
        item: Option<&str>,
    ) -> Result<Value> {
        self.current().menu(action, app, menu, item)
    }

    pub fn clipboard_read(&self) -> Result<String> {
        self.current().clipboard_read()
    }

    pub fn clipboard_write(&self, text: &str) -> Result<Value> {
        self.current().clipboard_write(text)
    }

    pub fn run_file(&self, path: &Path) -> Result<Vec<Value>> {
        self.current().run_file(path)
    }
}

pub fn parse_point(value: &str) -> Result<Point> {
    crate::automation::parse_point(value)
}

pub fn split_keys(value: &str) -> Vec<&str> {
    crate::automation::split_keys(value)
}

pub fn validate_output_path(path: &Path) -> Result<PathBuf> {
    crate::automation::validate_output_path(path)
}

fn resolve_query(elements: &[crate::UiElement], query: &str) -> Result<crate::UiElement> {
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
        return Err(crate::PeekabooError::AmbiguousTarget(format!(
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
        0 => Err(crate::PeekabooError::TargetNotFound(query.to_string())),
        1 => Ok(partial[0].clone()),
        count => Err(crate::PeekabooError::AmbiguousTarget(format!(
            "{query} matched {count} elements"
        ))),
    }
}

#[cfg(not(target_os = "macos"))]
fn legacy_action_click(action: &str) -> (&'static str, u32) {
    match action {
        "right_click" | "right-click" => ("right", 1),
        "double_click" | "double-click" | "open" | "press" => ("left", 2),
        _ => ("left", 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type SeeFn = fn(&Peekaboo, Option<&str>, ImageMode, Option<PathBuf>, bool) -> Result<Snapshot>;
    type TypeTextFn = fn(&Peekaboo, &str, bool, bool, Option<u64>, Option<&str>) -> Result<Value>;

    #[test]
    fn old_constructors_and_signatures_compile() {
        let _: Peekaboo = Peekaboo;
        let _: fn() -> Peekaboo = Peekaboo::new;
        let _: fn(&Peekaboo, ImageMode, Option<PathBuf>, bool) -> Result<ImageCapture> =
            Peekaboo::image;
        let _: SeeFn = Peekaboo::see;
        let _: fn(&Peekaboo, Option<&str>) -> Result<Vec<UiElement>> = Peekaboo::ui_elements;
        let _: fn(&Peekaboo, Target, &str, u32) -> Result<Value> = Peekaboo::click;
        let _: TypeTextFn = Peekaboo::type_text;
        let _: fn(&Peekaboo, Target, &str) -> Result<Value> = Peekaboo::perform_action;
    }

    #[test]
    fn compatibility_facade_uses_legacy_mode() {
        assert_eq!(Peekaboo.current().resolve_backend(), "legacy");
    }

    #[test]
    fn complete_old_surface_compiles() {
        let compile = |peekaboo: &Peekaboo, target: Target| {
            let _ = peekaboo.image(ImageMode::Screen, None, false);
            let _ = peekaboo.image_region(
                Bounds {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                None,
                false,
            );
            let _ = peekaboo.see(None, ImageMode::Screen, None, false);
            let _ = peekaboo.ui_elements(None);
            let _ = peekaboo.list_apps();
            let _ = peekaboo.list_windows();
            let _ = peekaboo.list_screens();
            let _ = peekaboo.shell("true", None);
            let _ = peekaboo.permissions();
            let _ = peekaboo.grant_permissions();
            let _ = peekaboo.click(target.clone(), "left", 1);
            let _ = peekaboo.move_cursor(target.clone());
            let _ = peekaboo.type_text("text", false, false, None, None);
            let _ = peekaboo.press("return", 1, None);
            let _ = peekaboo.hotkey(&["cmd", "a"]);
            let _ = peekaboo.paste("text");
            let _ = peekaboo.scroll(Direction::Down, 1);
            let _ = peekaboo.drag(target.clone(), target.clone(), 1);
            let _ = peekaboo.swipe(target.clone(), target.clone(), 1);
            let _ = peekaboo.set_value(target.clone(), "value");
            let _ = peekaboo.perform_action(target, "press");
            let _ = peekaboo.app("launch", Some("App"));
            let _ = peekaboo.open("file", None, false);
            let _ = peekaboo.window("focus", Some("App"), None, None);
            let _ = peekaboo.menu("list", "App", None, None);
            let _ = peekaboo.clipboard_read();
            let _ = peekaboo.clipboard_write("text");
            let _ = peekaboo.run_file(Path::new("steps.json"));
        };
        let _ = compile;
    }

    #[test]
    fn old_targets_route_to_current_targets() {
        let point = Point { x: 1, y: 2 };
        assert!(matches!(
            CurrentTarget::from(Target::Point(point.clone())),
            CurrentTarget::Point(value) if value == point
        ));

        let element = UiElement {
            id: "button".into(),
            role: "button".into(),
            label: "Save".into(),
            app: "Editor".into(),
            window: Some("Document".into()),
            bounds: Some(Bounds {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
            }),
            state: serde_json::json!({ "enabled": true }),
        };
        match CurrentTarget::from(Target::Element(element)) {
            CurrentTarget::Element(node) => {
                assert_eq!(node.id, "button");
                assert_eq!(node.label.as_deref(), Some("Save"));
                assert_eq!(node.backend, "legacy");
            }
            _ => panic!("element target did not route to current element target"),
        }
    }

    #[test]
    fn current_elements_route_without_new_fields() {
        let current = crate::UiElement {
            id: "field".into(),
            role: "textbox".into(),
            label: "Name".into(),
            app: "Editor".into(),
            window: None,
            bounds: None,
            state: Value::Null,
            index: Some(7),
        };
        let old = UiElement::from(current);
        assert_eq!(old.id, "field");
        assert_eq!(old.label, "Name");
        let json = serde_json::to_value(old).expect("serialize compatibility element");
        assert!(json.get("index").is_none());
    }

    #[test]
    fn old_list_windows_schema_omits_current_only_fields() {
        let current = crate::UiElement {
            id: "window".into(),
            role: "window".into(),
            label: "Document".into(),
            app: "Editor".into(),
            window: Some("Document".into()),
            bounds: None,
            state: Value::Null,
            index: Some(3),
        };
        let json = serde_json::to_value(vec![UiElement::from(current)]).unwrap();
        assert!(json[0].get("index").is_none());
        assert_eq!(json[0]["label"], "Document");
    }

    #[test]
    fn old_query_resolution_preserves_id_label_partial_and_ambiguity() {
        let elements = [
            crate::UiElement {
                id: "save-primary".into(),
                role: "button".into(),
                label: "Save Document".into(),
                app: "Editor".into(),
                window: None,
                bounds: None,
                state: Value::Null,
                index: Some(0),
            },
            crate::UiElement {
                id: "save-copy".into(),
                role: "button".into(),
                label: "Save Copy".into(),
                app: "Editor".into(),
                window: None,
                bounds: None,
                state: Value::Null,
                index: Some(1),
            },
            crate::UiElement {
                id: "cancel".into(),
                role: "button".into(),
                label: "Cancel".into(),
                app: "Editor".into(),
                window: None,
                bounds: None,
                state: Value::Null,
                index: Some(2),
            },
        ];

        assert_eq!(
            resolve_query(&elements, "save-copy").unwrap().index,
            Some(1)
        );
        assert_eq!(resolve_query(&elements, "cancel").unwrap().index, Some(2));
        assert_eq!(resolve_query(&elements, "document").unwrap().index, Some(0));
        assert!(matches!(
            resolve_query(&elements, "save"),
            Err(crate::PeekabooError::AmbiguousTarget(_))
        ));
    }

    #[test]
    fn old_public_helpers_route_to_current_implementations() {
        assert_eq!(parse_point("4,5").unwrap(), Point { x: 4, y: 5 });
        assert_eq!(split_keys("cmd+shift,p"), ["cmd", "shift", "p"]);
        assert!(validate_output_path(Path::new("../escape")).is_err());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn old_point_action_mapping_is_preserved() {
        assert_eq!(legacy_action_click("right_click"), ("right", 1));
        assert_eq!(legacy_action_click("double-click"), ("left", 2));
        assert_eq!(legacy_action_click("press"), ("left", 2));
        assert_eq!(legacy_action_click("focus"), ("left", 1));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn old_macos_point_element_actions_keep_the_platform_boundary() {
        let peekaboo = Peekaboo;
        assert!(matches!(
            peekaboo.set_value(Target::Point(Point { x: 1, y: 2 }), "value"),
            Err(crate::PeekabooError::MissingArgument("element target"))
        ));
        assert!(matches!(
            peekaboo.perform_action(Target::Point(Point { x: 1, y: 2 }), "press"),
            Err(crate::PeekabooError::MissingArgument("element target"))
        ));
    }
}

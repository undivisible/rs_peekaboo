use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageMode {
    Screen,
    Window,
    Menu,
}

impl ImageMode {
    pub fn parse(value: &str) -> Self {
        Self::parse_or_err(value).unwrap_or(Self::Screen)
    }

    pub fn parse_or_err(value: &str) -> crate::Result<Self> {
        match value {
            "screen" => Ok(Self::Screen),
            "window" => Ok(Self::Window),
            "menu" | "menubar" => Ok(Self::Menu),
            other => Err(crate::PeekabooError::InvalidImageMode(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn parse(value: &str) -> Self {
        Self::parse_or_err(value).unwrap_or(Self::Down)
    }

    pub fn parse_or_err(value: &str) -> crate::Result<Self> {
        match value {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            other => Err(crate::PeekabooError::InvalidDirection(other.to_string())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

impl Bounds {
    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.width / 2,
            y: self.y + self.height / 2,
        }
    }

    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }

    pub fn intersection_area(&self, other: &Bounds) -> i64 {
        let x = (self.x).max(other.x);
        let y = (self.y).max(other.y);
        let w = (self.x + self.width).min(other.x + other.width) - x;
        let h = (self.y + self.height).min(other.y + other.height) - y;
        if w <= 0 || h <= 0 { 0 } else { w * h }
    }

    pub fn overlaps(&self, other: &Bounds) -> bool {
        self.intersection_area(other) > 0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiElement {
    pub id: String,
    pub role: String,
    pub label: String,
    pub app: String,
    pub window: Option<String>,
    pub bounds: Option<Bounds>,
    pub state: Value,
    /// Stable 0-based index within a snapshot for agent targeting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub elements: Vec<UiElement>,
}

/// Health / capability report for agent preflight.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthReport {
    pub platform: String,
    pub mode: ComputerUseMode,
    pub backend: String,
    pub permissions: Value,
    pub capabilities: Value,
    pub tools: Value,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageCapture {
    pub path: PathBuf,
    pub mode: ImageMode,
    pub bytes: u64,
    pub mime_type: String,
    /// True when the capture was written to a temp file that the caller should delete.
    #[serde(default)]
    pub ephemeral: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
    pub success: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CommandResult {
    pub fn ok(data: impl Serialize) -> crate::Result<Self> {
        Ok(Self {
            ok: true,
            data: Some(serde_json::to_value(data)?),
            error: None,
        })
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunFile {
    pub steps: Vec<RunStep>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunStep {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

/// Automation mode selection. Default: Hybrid on macOS, Legacy elsewhere.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseMode {
    /// Native AX tree + screenshot metadata (macOS default).
    #[default]
    Hybrid,
    /// Pure native Accessibility API (macOS only).
    Native,
    /// Screenshot-first with external detection import hooks.
    Vision,
    /// AppleScript/System Events (existing behaviour).
    Legacy,
    /// Pure screenshot + coordinates + mouse/keyboard events.
    Coords,
}

impl ComputerUseMode {
    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "hybrid" => Ok(Self::Hybrid),
            "native" => Ok(Self::Native),
            "vision" => Ok(Self::Vision),
            "legacy" => Ok(Self::Legacy),
            "coords" => Ok(Self::Coords),
            other => Err(crate::PeekabooError::InvalidMode(other.to_string())),
        }
    }
}

/// Metadata about which backend served a request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendMetadata {
    pub backend: String,
    pub mode: ComputerUseMode,
    pub fallbacks_used: Vec<String>,
}

/// Rich UI node with full accessibility properties.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiNode {
    pub id: String,
    pub backend: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Bounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_in_parent: Option<i32>,
    /// Stable 0-based index within a snapshot for agent targeting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<UiNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count: Option<i32>,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub source: Vec<String>,
    #[serde(default)]
    pub state: Value,
}

impl From<&UiNode> for UiElement {
    fn from(node: &UiNode) -> Self {
        Self {
            id: node.id.clone(),
            role: node.role.clone(),
            label: node
                .title
                .clone()
                .or_else(|| node.label.clone())
                .unwrap_or_default(),
            app: node.app.clone(),
            window: node.window.clone(),
            bounds: node.bounds,
            state: node.state.clone(),
            index: node.index,
        }
    }
}

impl From<UiNode> for UiElement {
    fn from(node: UiNode) -> Self {
        Self::from(&node)
    }
}

impl From<UiElement> for UiNode {
    fn from(el: UiElement) -> Self {
        Self {
            id: el.id,
            backend: "legacy".into(),
            role: el.role,
            subrole: None,
            title: if el.label.is_empty() {
                None
            } else {
                Some(el.label.clone())
            },
            label: Some(el.label),
            description: None,
            value: None,
            identifier: None,
            app: el.app,
            pid: None,
            window: el.window,
            bounds: el.bounds,
            enabled: None,
            focused: None,
            selected: None,
            depth: None,
            index_in_parent: None,
            index: el.index,
            parent_id: None,
            children: None,
            children_count: None,
            actions: Vec::new(),
            attributes: HashMap::new(),
            confidence: None,
            source: vec!["legacy".into()],
            state: el.state,
        }
    }
}

/// Assign stable 0-based indices to a flat element list.
pub fn assign_element_indices(elements: &mut [UiElement]) {
    for (i, element) in elements.iter_mut().enumerate() {
        element.index = Some(i as u32);
    }
}

/// Assign stable 0-based indices to a flat node list.
pub fn assign_node_indices(nodes: &mut [UiNode]) {
    for (i, node) in nodes.iter_mut().enumerate() {
        node.index = Some(i as u32);
    }
}

/// Imported detection data from an external vision model.
#[derive(Clone, Debug, Deserialize)]
pub struct VisionDetections {
    pub image: Option<String>,
    pub elements: Vec<VisionElement>,
}

/// Single element from a vision model detection pass.
#[derive(Clone, Debug, Deserialize)]
pub struct VisionElement {
    pub role: Option<String>,
    pub label: Option<String>,
    pub bounds: Option<Bounds>,
    pub confidence: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeekabooError;

    #[test]
    fn image_mode_parse_or_err_should_accept_known_modes() {
        assert_eq!(
            ImageMode::parse_or_err("screen").unwrap(),
            ImageMode::Screen
        );
        assert_eq!(
            ImageMode::parse_or_err("window").unwrap(),
            ImageMode::Window
        );
        assert_eq!(ImageMode::parse_or_err("menu").unwrap(), ImageMode::Menu);
        assert_eq!(ImageMode::parse_or_err("menubar").unwrap(), ImageMode::Menu);
    }

    #[test]
    fn image_mode_parse_or_err_should_reject_unknown_modes() {
        let err = ImageMode::parse_or_err("bogus").unwrap_err();
        assert!(matches!(err, PeekabooError::InvalidImageMode(value) if value == "bogus"));
    }

    #[test]
    fn direction_parse_or_err_should_accept_known_directions() {
        assert_eq!(Direction::parse_or_err("up").unwrap(), Direction::Up);
        assert_eq!(Direction::parse_or_err("down").unwrap(), Direction::Down);
        assert_eq!(Direction::parse_or_err("left").unwrap(), Direction::Left);
        assert_eq!(Direction::parse_or_err("right").unwrap(), Direction::Right);
    }

    #[test]
    fn direction_parse_or_err_should_reject_unknown_directions() {
        let err = Direction::parse_or_err("sideways").unwrap_err();
        assert!(matches!(
            err,
            PeekabooError::InvalidDirection(value) if value == "sideways"
        ));
    }

    #[test]
    fn command_result_err_should_serialize_json_error() {
        let result = CommandResult::err("invalid image mode: bogus");
        let json = serde_json::to_value(result).expect("serialize error result");
        assert_eq!(json["ok"], false);
        assert_eq!(json["error"], "invalid image mode: bogus");
        assert!(json.get("data").is_none());
    }

    #[test]
    fn computer_use_mode_parse_should_accept_valid_modes() {
        assert_eq!(
            ComputerUseMode::parse("hybrid").unwrap(),
            ComputerUseMode::Hybrid
        );
        assert_eq!(
            ComputerUseMode::parse("native").unwrap(),
            ComputerUseMode::Native
        );
        assert_eq!(
            ComputerUseMode::parse("vision").unwrap(),
            ComputerUseMode::Vision
        );
        assert_eq!(
            ComputerUseMode::parse("legacy").unwrap(),
            ComputerUseMode::Legacy
        );
        assert_eq!(
            ComputerUseMode::parse("coords").unwrap(),
            ComputerUseMode::Coords
        );
    }

    #[test]
    fn computer_use_mode_default_is_hybrid() {
        assert_eq!(ComputerUseMode::default(), ComputerUseMode::Hybrid);
    }

    #[test]
    fn bounds_contains_should_check_point_inside() {
        let b = Bounds {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        assert!(b.contains(&Point { x: 50, y: 40 }));
        assert!(!b.contains(&Point { x: 5, y: 40 }));
        assert!(!b.contains(&Point { x: 50, y: 100 }));
    }

    #[test]
    fn bounds_overlaps_should_detect_intersection() {
        let a = Bounds {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let b = Bounds {
            x: 50,
            y: 50,
            width: 100,
            height: 100,
        };
        let c = Bounds {
            x: 200,
            y: 200,
            width: 50,
            height: 50,
        };
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn bounds_intersection_area_should_be_zero_when_disjoint() {
        let a = Bounds {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let b = Bounds {
            x: 100,
            y: 100,
            width: 10,
            height: 10,
        };
        assert_eq!(a.intersection_area(&b), 0);
    }

    #[test]
    fn ui_node_from_ui_element_maps_fields() {
        let el = UiElement {
            id: "test:id".into(),
            role: "button".into(),
            label: "Click me".into(),
            app: "Test".into(),
            window: Some("Window".into()),
            bounds: Some(Bounds {
                x: 0,
                y: 0,
                width: 10,
                height: 10,
            }),
            state: serde_json::json!({}),
            index: Some(3),
        };
        let node = UiNode::from(el);
        assert_eq!(node.backend, "legacy");
        assert_eq!(node.source, vec!["legacy"]);
        assert_eq!(node.title, Some("Click me".into()));
        assert_eq!(node.role, "button");
        assert_eq!(node.index, Some(3));
    }

    #[test]
    fn assign_element_indices_sets_stable_order() {
        let mut elements = vec![
            UiElement {
                id: "a".into(),
                role: "button".into(),
                label: "A".into(),
                app: "App".into(),
                window: None,
                bounds: None,
                state: serde_json::json!({}),
                index: None,
            },
            UiElement {
                id: "b".into(),
                role: "button".into(),
                label: "B".into(),
                app: "App".into(),
                window: None,
                bounds: None,
                state: serde_json::json!({}),
                index: None,
            },
        ];
        assign_element_indices(&mut elements);
        assert_eq!(elements[0].index, Some(0));
        assert_eq!(elements[1].index, Some(1));
    }
}

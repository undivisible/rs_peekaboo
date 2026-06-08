use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub elements: Vec<UiElement>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeekabooError;

    #[test]
    fn image_mode_parse_or_err_should_accept_known_modes() {
        assert_eq!(ImageMode::parse_or_err("screen").unwrap(), ImageMode::Screen);
        assert_eq!(ImageMode::parse_or_err("window").unwrap(), ImageMode::Window);
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
}

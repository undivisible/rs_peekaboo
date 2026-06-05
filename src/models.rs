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
        match value {
            "window" => Self::Window,
            "menu" | "menubar" => Self::Menu,
            _ => Self::Screen,
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
        match value {
            "up" => Self::Up,
            "left" => Self::Left,
            "right" => Self::Right,
            _ => Self::Down,
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
    pub data: Value,
}

impl CommandResult {
    pub fn ok(data: impl Serialize) -> crate::Result<Self> {
        Ok(Self {
            ok: true,
            data: serde_json::to_value(data)?,
        })
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

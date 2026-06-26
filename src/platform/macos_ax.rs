use crate::Result;
use crate::models::{Point, UiNode};
use serde_json::Value;

// Re-export CoreGraphics functions for use by other modules
pub use super::macos_cg::*;

// ── Placeholders for native AX backend ─────────────────────────────────

/// Traverse the full AX tree for the given app filter.
pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiNode>> {
    // Fall back to legacy osascript for now
    let els = super::macos_legacy::ui_elements(app_filter)?;
    Ok(els.into_iter().map(UiNode::from).collect())
}

/// Click at a point using AX actions when possible, CG otherwise.
pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    super::macos_legacy::click(point, button, count)
}

pub fn set_value(element: &UiNode, value: &str) -> Result<Value> {
    let el = crate::UiElement::from(element);
    super::macos_legacy::set_value(&el, value)
}

pub fn perform_action(element: &UiNode, action: &str) -> Result<Value> {
    let el = crate::UiElement::from(element);
    super::macos_legacy::perform_action(&el, action)
}

/// Get element at a specific screen point.
pub fn element_at_point(point: Point) -> Option<UiNode> {
    let _ = point;
    None // ponytail: not yet implemented, needs AXFFI
}

/// Return the focused element.
pub fn focused_element() -> Option<UiNode> {
    None // ponytail: needs AXFFI
}

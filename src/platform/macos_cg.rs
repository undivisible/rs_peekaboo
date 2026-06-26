use crate::models::{Bounds, ImageMode, Point};
use crate::platform::process;
use crate::{PeekabooError, Result};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use serde_json::{Value, json};
use std::path::Path;

pub fn capture_image(
    mode: ImageMode,
    path: &Path,
    retina: bool,
    region: Option<&Bounds>,
) -> Result<()> {
    let mut args = vec!["-x".to_string()];
    if mode == ImageMode::Window {
        args.push("-w".to_string());
    }
    if let Some(region) = region {
        args.push("-R".to_string());
        args.push(format!(
            "{},{},{},{}",
            region.x, region.y, region.width, region.height
        ));
    }
    if !retina {
        args.push("-r".to_string());
    }
    args.push(path.to_string_lossy().into_owned());
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    process::run("screencapture", &refs, None)?;
    Ok(())
}

pub fn move_cursor(point: Point) -> Result<Value> {
    move_cursor_to(&point)?;
    Ok(json!({ "point": point }))
}

fn move_cursor_to(point: &Point) -> Result<()> {
    post_mouse(CGEventType::MouseMoved, point, CGMouseButton::Left)
}

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

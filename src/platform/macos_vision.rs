//! Vision mode: screenshot-first with external detection import.
//! No model provider dependency — accepts detections from JSON.

use crate::models::{UiNode, VisionDetections};
use crate::Result;
use std::path::Path;

pub fn capture_screenshot(path: &Path) -> Result<()> {
    // delegates to macos_cg::capture_image with ImageMode::Screen
    super::macos_cg::capture_image(crate::ImageMode::Screen, path, false, None)
}

pub fn import_detections(json_path: &Path) -> Result<VisionDetections> {
    let data = std::fs::read(json_path)?;
    Ok(serde_json::from_slice(&data)?)
}

pub fn normalize_detections(detections: &VisionDetections) -> Vec<UiNode> {
    detections
        .elements
        .iter()
        .enumerate()
        .map(|(i, el)| {
            let role = el.role.clone().unwrap_or_else(|| "unknown".into());
            UiNode {
                id: format!("vision:{i}"),
                backend: "vision".into(),
                role,
                subrole: None,
                title: el.label.clone(),
                label: el.label.clone(),
                description: None,
                value: None,
                identifier: None,
                app: String::new(),
                pid: None,
                window: None,
                bounds: el.bounds,
                enabled: None,
                focused: None,
                selected: None,
                depth: None,
                index_in_parent: Some(i as i32),
                parent_id: None,
                children: None,
                children_count: None,
                actions: Vec::new(),
                attributes: std::collections::HashMap::new(),
                confidence: el.confidence,
                source: vec!["vision".into()],
                state: serde_json::json!({}),
            }
        })
        .collect()
}

/// Merge AX nodes with vision detections.
/// Prefer AX role/actions when available, prefer vision label if AX title is empty.
pub fn merge_ax_vision(ax_nodes: &[UiNode], vision_nodes: &[UiNode]) -> Vec<UiNode> {
    let mut merged: Vec<UiNode> = ax_nodes.to_vec();
    for vn in vision_nodes {
        let overlap = merged.iter().position(|ax| {
            ax.bounds
                .zip(vn.bounds)
                .map_or(false, |(a, v)| a.overlaps(&v))
        });
        match overlap {
            Some(idx) => {
                let mut existing = merged[idx].clone();
                if existing.title.as_ref().map_or(true, String::is_empty) {
                    existing.title = vn.title.clone();
                }
                if !existing.source.contains(&"vision".into()) {
                    existing.source.push("vision".into());
                }
                if existing.confidence.is_none() {
                    existing.confidence = vn.confidence;
                }
                merged[idx] = existing;
            }
            None => merged.push(vn.clone()),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, VisionElement};

    #[test]
    fn normalize_detections_creates_uimodes() {
        let dets = VisionDetections {
            image: None,
            elements: vec![VisionElement {
                role: Some("button".into()),
                label: Some("OK".into()),
                bounds: Some(Bounds { x: 0, y: 0, width: 10, height: 10 }),
                confidence: Some(0.95),
            }],
        };
        let nodes = normalize_detections(&dets);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].role, "button");
        assert_eq!(nodes[0].label, Some("OK".into()));
        assert_eq!(nodes[0].confidence, Some(0.95));
    }

    #[test]
    fn merge_ax_vision_prefers_ax_role() {
        let ax = vec![UiNode {
            id: "ax:1".into(),
            backend: "native".into(),
            role: "AXButton".into(),
            subrole: None,
            title: None,
            label: None,
            description: None,
            value: None,
            identifier: None,
            app: "Test".into(),
            pid: None,
            window: None,
            bounds: Some(Bounds { x: 0, y: 0, width: 100, height: 40 }),
            enabled: None,
            focused: None,
            selected: None,
            depth: None,
            index_in_parent: None,
            parent_id: None,
            children: None,
            children_count: None,
            actions: vec!["AXPress".into()],
            attributes: std::collections::HashMap::new(),
            confidence: None,
            source: vec!["ax".into()],
            state: serde_json::json!({}),
        }];
        let vision = vec![UiNode {
            id: "vision:1".into(),
            backend: "vision".into(),
            role: "button".into(),
            subrole: None,
            title: Some("Submit".into()),
            label: Some("Submit".into()),
            description: None,
            value: None,
            identifier: None,
            app: String::new(),
            pid: None,
            window: None,
            bounds: Some(Bounds { x: 5, y: 5, width: 90, height: 30 }),
            enabled: None,
            focused: None,
            selected: None,
            depth: None,
            index_in_parent: None,
            parent_id: None,
            children: None,
            children_count: None,
            actions: Vec::new(),
            attributes: std::collections::HashMap::new(),
            confidence: Some(0.92),
            source: vec!["vision".into()],
            state: serde_json::json!({}),
        }];
        let merged = merge_ax_vision(&ax, &vision);
        assert_eq!(merged.len(), 1);
        // AX role preserved
        assert_eq!(merged[0].role, "AXButton");
        // Vision title filled in since AX title was None
        assert_eq!(merged[0].title, Some("Submit".into()));
        // Both sources
        assert!(merged[0].source.contains(&"vision".into()));
    }
}

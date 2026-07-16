use crate::models::{Bounds, Point, UiNode};
use crate::{PeekabooError, Result};
use std::str::FromStr;

/// A parsed query for matching UI elements by properties.
#[derive(Clone, Debug, Default)]
pub struct Selector {
    pub role: Option<String>,
    pub role_contains: Option<String>,
    pub title: Option<String>,
    pub title_contains: Option<String>,
    pub label: Option<String>,
    pub label_contains: Option<String>,
    pub description: Option<String>,
    pub app: Option<String>,
    pub pid: Option<i32>,
    pub identifier: Option<String>,
    pub focused: Option<bool>,
    pub enabled: Option<bool>,
    /// 0-based index for nth-match filtering.
    pub index: Option<usize>,
    /// Element at specific coordinates.
    pub at: Option<Point>,
    /// Bounds filter (min intersection area ratio 0-1).
    pub bounds_overlap: Option<Bounds>,
}

impl Selector {
    /// Parse a query string into a Selector.
    ///
    /// Supported tokens:
    /// - `role=button` (exact match)
    /// - `role~="butt"` (contains match)
    /// - `title="Submit"` (exact match)
    /// - `title~="Submit"` (contains match)
    /// - `app=Safari`
    /// - `pid=1234`
    /// - `focused=true`
    /// - `enabled=true`
    /// - `index=0` (nth match)
    /// - `at=500,300` (element at coordinates)
    pub fn parse(query: &str) -> Result<Self> {
        let mut sel = Selector::default();
        for token in query.split_whitespace() {
            if token.is_empty() {
                continue;
            }
            if let Some((key, raw_value)) = token.split_once('=') {
                let value = raw_value.trim_matches('"');
                match key {
                    "role" => sel.role = Some(value.to_string()),
                    "role~" => sel.role_contains = Some(value.to_string()),
                    "title" => sel.title = Some(value.to_string()),
                    "title~" => sel.title_contains = Some(value.to_string()),
                    "label" => sel.label = Some(value.to_string()),
                    "label~" => sel.label_contains = Some(value.to_string()),
                    "description" => sel.description = Some(value.to_string()),
                    "app" => sel.app = Some(value.to_string()),
                    "pid" => {
                        sel.pid = Some(i32::from_str(value).map_err(|_| {
                            PeekabooError::InvalidCoordinates(format!("invalid pid: {value}"))
                        })?);
                    }
                    "identifier" | "id" => sel.identifier = Some(value.to_string()),
                    "focused" => {
                        sel.focused = Some(
                            value == "true" || value == "1" || value.eq_ignore_ascii_case("yes"),
                        );
                    }
                    "enabled" => {
                        sel.enabled = Some(
                            value == "true" || value == "1" || value.eq_ignore_ascii_case("yes"),
                        );
                    }
                    "index" | "nth" => {
                        sel.index = Some(usize::from_str(value).map_err(|_| {
                            PeekabooError::InvalidCoordinates(format!("invalid index: {value}"))
                        })?);
                    }
                    "at" => {
                        let (x, y) = value.split_once(',').ok_or_else(|| {
                            PeekabooError::InvalidCoordinates(format!("invalid at: {value}"))
                        })?;
                        sel.at = Some(Point {
                            x: x.trim().parse().map_err(|_| {
                                PeekabooError::InvalidCoordinates(format!("invalid x: {x}"))
                            })?,
                            y: y.trim().parse().map_err(|_| {
                                PeekabooError::InvalidCoordinates(format!("invalid y: {y}"))
                            })?,
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(sel)
    }

    /// Check if a UiNode matches this selector.
    pub fn matches(&self, node: &UiNode) -> bool {
        if let Some(ref role) = self.role {
            if !node.role.eq_ignore_ascii_case(role) {
                return false;
            }
        }
        if let Some(ref contains) = self.role_contains {
            if !node
                .role
                .to_ascii_lowercase()
                .contains(&contains.to_ascii_lowercase())
            {
                return false;
            }
        }
        if let Some(ref title) = self.title {
            match &node.title {
                Some(t) if t.eq_ignore_ascii_case(title) => {}
                _ => return false,
            }
        }
        if let Some(ref contains) = self.title_contains {
            match &node.title {
                Some(t)
                    if t.to_ascii_lowercase()
                        .contains(&contains.to_ascii_lowercase()) => {}
                _ => return false,
            }
        }
        if let Some(ref label) = self.label {
            match &node.label {
                Some(l) if l.eq_ignore_ascii_case(label) => {}
                _ => return false,
            }
        }
        if let Some(ref contains) = self.label_contains {
            match &node.label {
                Some(l)
                    if l.to_ascii_lowercase()
                        .contains(&contains.to_ascii_lowercase()) => {}
                _ => return false,
            }
        }
        if let Some(ref app) = self.app {
            if !node.app.eq_ignore_ascii_case(app) {
                return false;
            }
        }
        if let Some(pid) = self.pid {
            if node.pid != Some(pid) {
                return false;
            }
        }
        if let Some(ref focused) = self.focused {
            if node.focused != Some(*focused) {
                return false;
            }
        }
        if let Some(ref enabled) = self.enabled {
            if node.enabled != Some(*enabled) {
                return false;
            }
        }
        if let Some(ref bounds) = self.bounds_overlap {
            match &node.bounds {
                Some(b) if b.overlaps(bounds) => {}
                _ => return false,
            }
        }
        true
    }

    /// Filter a list of nodes, returning matches.
    pub fn filter<'a>(&self, nodes: &'a [UiNode]) -> Vec<&'a UiNode> {
        let matched: Vec<&UiNode> = nodes.iter().filter(|n| self.matches(n)).collect();
        match self.index {
            Some(idx) if idx < matched.len() => vec![matched[idx]],
            Some(_) => Vec::new(),
            None => matched,
        }
    }

    /// Return the first match, or None.
    pub fn first_match(&self, nodes: &[UiNode]) -> Option<UiNode> {
        match self.index {
            Some(idx) => nodes.iter().filter(|n| self.matches(n)).nth(idx).cloned(),
            None => nodes.iter().find(|n| self.matches(n)).cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Bounds;

    fn sample_node() -> UiNode {
        UiNode {
            id: "test:1".into(),
            backend: "test".into(),
            role: "button".into(),
            subrole: None,
            title: Some("Submit".into()),
            label: Some("Submit".into()),
            description: None,
            value: None,
            identifier: None,
            app: "Safari".into(),
            pid: Some(1234),
            window: Some("Window".into()),
            bounds: Some(Bounds {
                x: 10,
                y: 20,
                width: 100,
                height: 40,
            }),
            enabled: Some(true),
            focused: Some(false),
            selected: None,
            depth: None,
            index_in_parent: None,
            index: None,
            parent_id: None,
            children: None,
            children_count: None,
            actions: vec!["AXPress".into()],
            attributes: std::collections::HashMap::new(),
            confidence: None,
            source: vec!["ax".into()],
            state: serde_json::json!({}),
        }
    }

    #[test]
    fn parse_role_button() {
        let sel = Selector::parse("role=button").unwrap();
        assert_eq!(sel.role, Some("button".into()));
    }

    #[test]
    fn parse_title_contains() {
        let sel = Selector::parse(r#"title~="Continue""#).unwrap();
        assert_eq!(sel.title_contains, Some("Continue".into()));
    }

    #[test]
    fn parse_app_and_role() {
        let sel = Selector::parse("app=Safari role=textfield").unwrap();
        assert_eq!(sel.app, Some("Safari".into()));
        assert_eq!(sel.role, Some("textfield".into()));
    }

    #[test]
    fn parse_focused() {
        let sel = Selector::parse("focused=true").unwrap();
        assert_eq!(sel.focused, Some(true));
    }

    #[test]
    fn parse_pid() {
        let sel = Selector::parse("pid=1234").unwrap();
        assert_eq!(sel.pid, Some(1234));
    }

    #[test]
    fn parse_at() {
        let sel = Selector::parse("at=500,300").unwrap();
        assert_eq!(sel.at, Some(Point { x: 500, y: 300 }));
    }

    #[test]
    fn matches_exact_role() {
        let node = sample_node();
        let sel = Selector {
            role: Some("button".into()),
            ..Default::default()
        };
        assert!(sel.matches(&node));
        let sel2 = Selector {
            role: Some("textfield".into()),
            ..Default::default()
        };
        assert!(!sel2.matches(&node));
    }

    #[test]
    fn matches_title_contains() {
        let node = sample_node();
        let sel = Selector {
            title_contains: Some("ub".into()),
            ..Default::default()
        };
        assert!(sel.matches(&node));
        let sel2 = Selector {
            title_contains: Some("Bmit".into()),
            ..Default::default()
        };
        assert!(sel2.matches(&node));
    }

    #[test]
    fn matches_app() {
        let node = sample_node();
        let sel = Selector {
            app: Some("Safari".into()),
            ..Default::default()
        };
        assert!(sel.matches(&node));
        let sel2 = Selector {
            app: Some("Chrome".into()),
            ..Default::default()
        };
        assert!(!sel2.matches(&node));
    }

    #[test]
    fn matches_pid() {
        let node = sample_node();
        let sel = Selector {
            pid: Some(1234),
            ..Default::default()
        };
        assert!(sel.matches(&node));
        let sel2 = Selector {
            pid: Some(9999),
            ..Default::default()
        };
        assert!(!sel2.matches(&node));
    }

    #[test]
    fn matches_focused() {
        let node = sample_node();
        let sel = Selector {
            focused: Some(false),
            ..Default::default()
        };
        assert!(sel.matches(&node));
        let sel2 = Selector {
            focused: Some(true),
            ..Default::default()
        };
        assert!(!sel2.matches(&node));
    }

    #[test]
    fn filter_respects_index() {
        let nodes = vec![
            UiNode {
                id: "a".into(),
                role: "button".into(),
                ..sample_node()
            },
            UiNode {
                id: "b".into(),
                role: "button".into(),
                ..sample_node()
            },
        ];
        let sel = Selector {
            role: Some("button".into()),
            index: Some(1),
            ..Default::default()
        };
        let result = sel.filter(&nodes);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn first_match_returns_none_when_no_match() {
        let nodes = vec![sample_node()];
        let sel = Selector {
            role: Some("window".into()),
            ..Default::default()
        };
        assert!(sel.first_match(&nodes).is_none());
    }
}

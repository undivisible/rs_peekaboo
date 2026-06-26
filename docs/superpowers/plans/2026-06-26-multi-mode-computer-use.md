# Multi-Mode Computer-Use Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `rs_peekaboo` into a proper multi-mode computer-use backend for agents, with native macOS AX support, compatibility fallbacks, release CI, downstream integration updates, republish, retag, and Homebrew tap update.

**Architecture:** Split the monolithic macOS backend into dedicated modules (AX, Vision, Legacy, CoreGraphics, Permissions). Add a `ComputerUseMode` enum that dispatches to the correct backend. Add selectors for element queries. Keep existing public API compatible, extend with `Peekaboo::with_mode(...)`.

**Tech Stack:** Rust, `clap`, `serde_json`, `core-graphics` (existing), direct `extern "C"` FFI to ApplicationServices/AXUIElement APIs, `core-foundation2` types.

## Global Constraints

- Keep `pub use` API from `lib.rs` compatible — add, don't break
- No telemetry, no model provider dependencies
- Every JSON response includes `"backend"`, `"mode"`, `"fallbacks_used"` fields
- Default mode on macOS = `Hybrid`, else closest equivalent
- Native AX uses direct `extern "C"` FFI, not `objc2` — keeps build simple
- Version bump 0.2.4 → 0.3.0

---

### Task 1: Add Core Types (ComputerUseMode, UiNode, BackendMetadata)

**Files:**
- Modify: `src/models.rs` — add enums, structs

**Interfaces:**
- Produces: `ComputerUseMode`, `UiNode` (rich element), `BackendMetadata`, `DetectionSource`, `VisionElement`

- [ ] **Add ComputerUseMode enum**

```rust
/// Selection of the computer-use automation backends.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerUseMode {
    /// Native AX tree + screenshot metadata. Default on macOS.
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
    pub fn parse(value: &str) -> Result<Self> {
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
```

- [ ] **Add UiNode struct**

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiNode {
    pub id: String,
    pub backend: String,
    pub role: String,
    pub subrole: Option<String>,
    pub title: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub value: Option<String>,
    pub identifier: Option<String>,
    pub app: String,
    pub pid: Option<i32>,
    pub window: Option<String>,
    pub bounds: Option<Bounds>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub selected: Option<bool>,
    pub depth: Option<i32>,
    pub index_in_parent: Option<i32>,
    pub parent_id: Option<String>,
    pub children: Option<Vec<UiNode>>,
    pub children_count: Option<i32>,
    pub actions: Vec<String>,
    pub attributes: std::collections::HashMap<String, String>,
    pub confidence: Option<f64>,
    pub source: Vec<String>,
    pub state: serde_json::Value,
}
```

- [ ] **Add BackendMetadata**

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendMetadata {
    pub backend: String,
    pub mode: ComputerUseMode,
    pub fallbacks_used: Vec<String>,
}
```

- [ ] **Add VisionElement for detection import**

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct VisionDetections {
    pub image: Option<String>,
    pub elements: Vec<VisionElement>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VisionElement {
    pub role: Option<String>,
    pub label: Option<String>,
    pub bounds: Option<Bounds>,
    pub confidence: Option<f64>,
}
```

- [ ] **Add UiNode conversion from UiElement**

Add `From<UiElement>` for `UiNode` that maps basic fields and sets `source: vec!["legacy".into()]`.

- [ ] **Update PeekabooError with InvalidMode**

Add `InvalidMode(String)` variant.

- [ ] **Run tests**

```bash
cd /Users/undivisible/projects/rs_peekaboo && cargo test --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/rs_peekaboo
git add src/models.rs src/error.rs
git commit -m "feat(core): add ComputerUseMode, UiNode, and BackendMetadata types"
```

---

### Task 2: Split macOS Platform into Dedicated Modules

**Files:**
- Create: `src/platform/macos_ax.rs`
- Create: `src/platform/macos_vision.rs`
- Create: `src/platform/macos_legacy.rs`
- Create: `src/platform/macos_cg.rs`
- Create: `src/platform/macos_permissions.rs`
- Create: `src/platform/backend_registry.rs`
- Modify: `src/platform/macos.rs` — refactor to dispatcher
- Modify: `src/platform/mod.rs` — add new modules

**Interfaces:**
- Consumes: types from Task 1 (`ComputerUseMode`, `UiNode`)
- Produces: `backend_registry::Backend` trait + 5 backend implementations

- [ ] **Move existing osascript code to macos_legacy.rs**

Extract all osascript-based functions (snapshot, click, type, etc.) into `macos_legacy.rs`. Keep signatures compatible.

```rust
// src/platform/macos_legacy.rs
pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiNode>> { ... }
pub fn click(point: Point, button: &str, count: u32) -> Result<Value> { ... }
pub fn set_value(element: &UiNode, value: &str) -> Result<Value> { ... }
pub fn perform_action(element: &UiNode, action: &str) -> Result<Value> { ... }
// ... all existing functions
```

- [ ] **Move CG event code to macos_cg.rs**

Extract CoreGraphics functions.

```rust
// src/platform/macos_cg.rs
pub fn capture_image(...) -> Result<()> { ... }
pub fn move_cursor(point: Point) -> Result<()> { ... }
pub fn click_cg(point: Point, button: &str, count: u32) -> Result<Value> { ... }
pub fn post_mouse(...) -> Result<()> { ... }
```

- [ ] **Move permission probes to macos_permissions.rs**

```rust
// src/platform/macos_permissions.rs
pub fn check_accessibility() -> bool { ... }
pub fn check_screen_recording() -> bool { ... }
pub fn check_clipboard() -> bool { ... }
pub fn permissions() -> Value { ... }
pub fn grant_permissions() -> Result<Value> { ... }
```

- [ ] **Create macos_vision.rs skeleton**

```rust
// src/platform/macos_vision.rs
pub fn capture_screenshot(path: &Path) -> Result<()> { ... }
pub fn import_detections(json_path: &Path) -> Result<VisionDetections> { ... }
pub fn merge_with_ax(ax_nodes: &[UiNode], detections: &VisionDetections) -> Vec<UiNode> { ... }
```

- [ ] **Create macos_ax.rs skeleton**

```rust
// src/platform/macos_ax.rs
pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiNode>> { ... }
pub fn click(...) -> Result<Value> { ... }
pub fn set_value(...) -> Result<Value> { ... }
pub fn perform_action(...) -> Result<Value> { ... }
```

- [ ] **Refactor macos.rs to dispatcher**

`macos.rs` becomes a thin dispatcher that selects backend based on `ComputerUseMode`.

```rust
// src/platform/macos.rs
use crate::ComputerUseMode;

pub fn get_mode() -> ComputerUseMode {
    // check env var, otherwise default
    ComputerUseMode::Hybrid
}

pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiNode>> {
    match get_mode() {
        ComputerUseMode::Native | ComputerUseMode::Hybrid => macos_ax::ui_elements(app_filter),
        ComputerUseMode::Legacy => macos_legacy::ui_elements(app_filter),
        ComputerUseMode::Vision | ComputerUseMode::Coords => Ok(Vec::new()),
    }
}
// ... same pattern for click, type_text, etc.
```

- [ ] **Update mod.rs**

```rust
pub mod backend_registry;
#[cfg(target_os = "macos")]
pub mod macos_ax;
#[cfg(target_os = "macos")]
pub mod macos_vision;
#[cfg(target_os = "macos")]
pub mod macos_legacy;
#[cfg(target_os = "macos")]
pub mod macos_cg;
#[cfg(target_os = "macos")]
pub mod macos_permissions;
```

- [ ] **Run build test on macOS**

```bash
cd /Users/undivisible/projects/rs_peekaboo && cargo check --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/rs_peekaboo
git add src/platform/
git commit -m "refactor(macos): split monolithic backend into dedicated modules (AX, legacy, CG, vision, permissions)"
```

---

### Task 3: Implement Native macOS AX Backend

**Files:**
- Modify: `src/platform/macos_ax.rs` — full implementation
- Modify: `Cargo.toml` — add `core-foundation2` dep
- Create: `src/platform/macos_ax_sys.rs` — FFI bindings

**Interfaces:**
- Consumes: `UiNode`, `Bounds`, `ComputerUseMode`
- Produces: full AX tree traversal + actions

- [ ] **Add core-foundation2 dependency**

```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation2 = "0.5"
core-graphics = "0.25"
```

- [ ] **Create FFI bindings in macos_ax_sys.rs**

```rust
//! Low-level FFI bindings to macOS Accessibility API (ApplicationServices framework).
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use core_foundation2::base::{CFIndex, CFTypeRef, OSStatus};
use core_foundation2::string::CFStringRef;
use core_foundation2::dictionary::CFDictionaryRef;
use core_foundation2::boolean::CFBooleanRef;
use core_foundation2::number::CFNumberRef;
use core_foundation2::array::CFArrayRef;
use core_foundation2::url::CFURLRef;
use core_foundation2::bundle::CFBundleRef;

pub type AXUIElementRef = CFTypeRef;
pub type AXValueRef = CFTypeRef;
pub type CFStringRef = CFTypeRef;

// Framework: ApplicationServices.framework
// AXUIElement.h functions

extern "C" {
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub fn CFRetain(cf: CFTypeRef);
    pub fn CFRelease(cf: CFTypeRef);

    // Attribute access
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> OSStatus;

    pub fn AXUIElementCopyAttributeValues(
        element: AXUIElementRef,
        attribute: CFStringRef,
        index: CFIndex,
        max_values: CFIndex,
        values: *mut CFArrayRef,
    ) -> OSStatus;

    pub fn AXUIElementGetAttributeValueCount(
        element: AXUIElementRef,
        attribute: CFStringRef,
        count: *mut CFIndex,
    ) -> OSStatus;

    // Actions
    pub fn AXUIElementCopyActionNames(
        element: AXUIElementRef,
        names: *mut CFArrayRef,
    ) -> OSStatus;

    pub fn AXUIElementPerformAction(
        element: AXUIElementRef,
        action: CFStringRef,
    ) -> OSStatus;

    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> OSStatus;

    // Position-based
    pub fn AXUIElementCopyElementAtPosition(
        element: AXUIElementRef,
        x: f64,
        y: f64,
        value: *mut AXUIElementRef,
    ) -> OSStatus;

    // Permissions
    pub fn AXIsProcessTrustedWithOptions(
        options: CFDictionaryRef,
    ) -> Boolean;

    // Value helpers
    pub fn AXValueGetType(value: AXValueRef) -> u32;
    pub fn AXValueGetValue(value: AXValueRef, the_type: u32, value_ptr: *mut std::ffi::c_void) -> Boolean;
}

// AXValueType constants
pub const kAXValueCGPointType: u32 = 1;
pub const kAXValueCGSizeType: u32 = 2;
pub const kAXValueCGRectType: u32 = 3;

// Standard AX attributes as CFString constants
// We use static CFString references via CFSTR macros later, but for now:
pub unsafe fn ax_attr(ptr: *const u8) -> CFStringRef {
    CFStringCreateWithCString(std::ptr::null_mut(), ptr as *const i8, 0x08000100) // kCFStringEncodingUTF8
}

// But actually, the simplest approach is to use CFString::new() from core-foundation2
```

Actually, simpler approach — use `core-foundation2` which has proper Rust types:

```rust
use core_foundation2::string::CFString;
use core_foundation2::base::TCFType;

// Attribute names
pub const AX_ROLE: &str = "AXRole";
pub const AX_SUBROLE: &str = "AXSubrole";
pub const AX_TITLE: &str = "AXTitle";
pub const AX_DESCRIPTION: &str = "AXDescription";
pub const AX_VALUE: &str = "AXValue";
pub const AX_IDENTIFIER: &str = "AXIdentifier";
pub const AX_ENABLED: &str = "AXEnabled";
pub const AX_FOCUSED: &str = "AXFocused";
pub const AX_SELECTED: &str = "AXSelected";
pub const AX_POSITION: &str = "AXPosition";
pub const AX_SIZE: &str = "AXSize";
pub const AX_CHILDREN: &str = "AXChildren";
pub const AX_WINDOWS: &str = "AXWindows";
pub const AX_FOCUSED_WINDOW: &str = "AXFocusedWindow";
pub const AX_FOCUSED_UI_ELEMENT: &str = "AXFocusedUIElement";
pub const AX_MENU_BAR: &str = "AXMenuBar";
pub const AX_PARENT: &str = "AXParent";
pub const AX_ROLE_DESCRIPTION: &str = "AXRoleDescription";

// Action names
pub const AX_PRESS: &str = "AXPress";
pub const AX_CONFIRM: &str = "AXConfirm";
pub const AX_CANCEL: &str = "AXCancel";
pub const AX_SHOW_MENU: &str = "AXShowMenu";
pub const AX_INCREMENT: &str = "AXIncrement";
pub const AX_DECREMENT: &str = "AXDecrement";
```

- [ ] **Implement AX tree traversal**

```rust
pub fn build_tree(app_filter: Option<&str>) -> Result<Vec<UiNode>> {
    let system_wide = ax_sys::AXUIElementCreateSystemWide();
    // Get list of running apps via NSWorkspace (or system_profiler)
    // For each app, create AXUIElementCreateApplication(pid)
    // Get AXWindows attribute
    // For each window, recursively walk children
    // Build UiNode tree with parent/child relationships
}
```

- [ ] **Implement attribute reading**

```rust
fn get_ax_attribute(element: AXUIElementRef, attr: &str) -> Result<Option<CFTypeRef>> { ... }
fn get_ax_string(element: AXUIElementRef, attr: &str) -> Result<Option<String>> { ... }
fn get_ax_bool(element: AXUIElementRef, attr: &str) -> Result<Option<bool>> { ... }
fn get_ax_position(element: AXUIElementRef) -> Result<Option<Point>> { ... }
fn get_ax_size(element: AXUIElementRef) -> Result<Option<Bounds>> { ... } // returns width/height only
fn get_ax_children(element: AXUIElementRef) -> Result<Vec<AXUIElementRef>> { ... }
fn get_ax_actions(element: AXUIElementRef) -> Result<Vec<String>> { ... }
```

- [ ] **Implement AX actions (click, set_value, perform_action)**

```rust
pub fn ax_click(lookup: &UiNode) -> Result<Value> {
    // Resolve AXUIElement from UiNode id/pid
    // Perform AXPress action
    // If AXPress fails or not available, fall back to CG click at bounds.center()
}
```

- [ ] **Write unit tests for FFI (compile-only on non-macOS)**

```rust
#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    #[test]
    fn test_ax_attribute_names_are_valid_cfstrings() {
        // Just verify the constant strings are valid UTF-8
    }
}
```

- [ ] **Build and run tests**

```bash
cd /Users/undivisible/projects/rs_peekaboo && cargo check --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/rs_peekaboo
git add src/platform/macos_ax.rs src/platform/macos_ax_sys.rs Cargo.toml Cargo.lock
git commit -m "feat(macos): add native accessibility API backend with AXUIElement FFI"
```

---

### Task 4: Add Selector Parser

**Files:**
- Create: `src/platform/selector.rs`

**Interfaces:**
- Produces: `struct Selector { ... }` + `fn matches(&self, node: &UiNode) -> bool`

- [ ] **Implement Selector type and parser**

```rust
#[derive(Debug, Clone)]
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
    pub index: Option<usize>,
    pub at: Option<Point>,
}

pub fn parse(query: &str) -> Result<Selector> {
    // tokenize by space
    // each token: key=value or key~=value (contains)
    // return Selector
}
```

- [ ] **Implement Selector::matches**

```rust
impl Selector {
    pub fn matches(&self, node: &UiNode) -> bool { ... }
    pub fn filter(&self, nodes: &[UiNode]) -> Vec<UiNode> { ... }
    pub fn first_match(&self, nodes: &[UiNode]) -> Option<UiNode> { ... }
}
```

- [ ] **Write unit tests**

```rust
#[test]
fn test_parse_role_button() {
    let sel = parse("role=button").unwrap();
    assert_eq!(sel.role, Some("button".into()));
}

#[test]
fn test_parse_title_contains() {
    let sel = parse(r#"title~="Continue""#).unwrap();
    assert_eq!(sel.title_contains, Some("Continue".into()));
}

#[test]
fn test_parse_app_and_role() {
    let sel = parse("app=Safari role=textfield").unwrap();
    assert_eq!(sel.app, Some("Safari".into()));
    assert_eq!(sel.role, Some("textfield".into()));
}

#[test]
fn test_parse_focused() {
    let sel = parse("focused=true").unwrap();
    assert_eq!(sel.focused, Some(true));
}

#[test]
fn test_parse_pid() {
    let sel = parse("pid=1234").unwrap();
    assert_eq!(sel.pid, Some(1234));
}

#[test]
fn test_matches_exact_role() {
    let node = UiNode { role: "button".into(), .. };
    let sel = Selector { role: Some("button".into()), .. };
    assert!(sel.matches(&node));
}

#[test]
fn test_matches_contains_title() {
    let node = UiNode { title: Some("Continue Anyway".into()), .. };
    let sel = Selector { title_contains: Some("Continue".into()), .. };
    assert!(sel.matches(&node));
}

#[test]
fn test_filter_respects_index() {
    let nodes = vec![
        UiNode { role: "button".into(), title: Some("A".into()), .. },
        UiNode { role: "button".into(), title: Some("B".into()), .. },
    ];
    let sel = Selector { role: Some("button".into()), index: Some(1), .. };
    let result = sel.filter(&nodes);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, Some("B".into()));
}
```

- [ ] **Run tests**

```bash
cd /Users/undivisible/projects/rs_peekaboo && cargo test --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/rs_peekaboo
git add src/platform/selector.rs
git commit -m "feat(core): add selector parser for UI element queries"
```

---

### Task 5: Wire Mode Dispatch into automation.rs + CLI

**Files:**
- Modify: `src/automation.rs` — add mode config, dispatch
- Modify: `src/cli.rs` — add `--mode` flag, `--focused`, `--at`, `--tree`, `--flat`
- Modify: `src/platform/macos.rs` — update dispatcher to use stored mode
- Modify: `src/lib.rs` — export new types

**Interfaces:**
- Consumes: `ComputerUseMode`, `Selector`, `UiNode`, backends
- Produces: `Peekaboo::with_mode(...)`, `Peekaboo::with_config(...)`, mode-aware CLI

- [ ] **Add mode to Peekaboo config**

```rust
#[derive(Clone, Debug)]
pub struct PeekabooConfig {
    pub mode: ComputerUseMode,
}

impl Default for PeekabooConfig {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            mode: ComputerUseMode::Hybrid,
            #[cfg(not(target_os = "macos"))]
            mode: ComputerUseMode::Legacy,
        }
    }
}

pub struct Peekaboo {
    pub(crate) config: PeekabooConfig,
}

impl Peekaboo {
    pub fn new() -> Self { Self::with_config(PeekabooConfig::default()) }
    pub fn with_config(config: PeekabooConfig) -> Self { Self { config } }
    pub fn config(&self) -> &PeekabooConfig { &self.config }
}
```

- [ ] **Add mode-aware dispatch to each action**

```rust
fn resolve_backend(&self) -> &'static str {
    match self.config.mode {
        ComputerUseMode::Native => "native",
        ComputerUseMode::Hybrid => "hybrid",
        ComputerUseMode::Vision => "vision",
        ComputerUseMode::Legacy => "legacy",
        ComputerUseMode::Coords => "coords",
    }
}

fn backend_metadata(&self, fallbacks: Vec<String>) -> BackendMetadata {
    BackendMetadata {
        backend: self.resolve_backend().to_string(),
        mode: self.config.mode,
        fallbacks_used: fallbacks,
    }
}

pub fn ui_elements(&self, app_filter: Option<&str>) -> Result<Vec<UiElement>> {
    let nodes = platform::backend::ui_elements_with_mode(app_filter, self.config.mode)?;
    // convert UiNode back to UiElement for backward compat
    Ok(nodes.into_iter().map(UiElement::from).collect())
}
```

- [ ] **Add CLI flags**

```rust
#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(long, global = true, env = "RS_PEEKABOO_MODE")]
    pub mode: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

// Add to SeeArgs
pub struct SeeArgs {
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long)]
    pub tree: bool,
    #[arg(long)]
    pub flat: bool,
    #[arg(long)]
    pub focused: bool,
    #[arg(long)]
    pub at: Option<String>,
    // ... existing fields
}
```

- [ ] **Add see --tree, --flat, --focused, --at handling**

```rust
Commands::See(args) => {
    let mut peekaboo = Peekaboo::with_config(config_from_mode(cli.mode));
    // If --focused, get only focused element
    // If --at x,y, get element at point
    // If --tree, output nested tree
    // If --flat, output flat list
    // Default: current behaviour
}
```

- [ ] **Add click --query support**

```rust
Commands::Click(args) => {
    // parse --query into Selector
    // resolve against current UI snapshot
    // click via mode-appropriate backend
}
```

- [ ] **Add perform-action and set-value via query**

- [ ] **Export new types from lib.rs**

```rust
pub use models::{ComputerUseMode, UiNode, BackendMetadata, ...};
pub use platform::selector::Selector;
pub use automation::{PeekabooConfig, Target};
```

- [ ] **Run tests**

```bash
cd /Users/undivisible/projects/rs_peekaboo && cargo test --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/rs_peekaboo
git add src/automation.rs src/cli.rs src/lib.rs src/platform/macos.rs
git commit -m "feat(core): wire ComputerUseMode dispatch into Peekaboo and CLI"
```

---

### Task 6: Vision Mode Implementation

**Files:**
- Modify: `src/platform/macos_vision.rs` — full implementation

**Interfaces:**
- Consumes: `VisionDetections`, `VisionElement`, `UiNode`
- Produces: detection import, hybrid merge

- [ ] **Implement capture_screenshot**

```rust
pub fn capture_screenshot(path: &Path) -> Result<()> {
    // Same as existing capture_image with screencapture
}
```

- [ ] **Implement import_detections**

```rust
pub fn import_detections(json_path: &Path) -> Result<VisionDetections> {
    let data = std::fs::read(json_path)?;
    Ok(serde_json::from_slice(&data)?)
}
```

- [ ] **Implement hybrid merge**

```rust
pub fn merge_with_ax(ax_nodes: &[UiNode], detections: &VisionDetections) -> Vec<UiNode> {
    // For each vision element, find overlapping AX nodes by bounds
    // Merge: prefer AX role/actions, prefer vision label if AX title empty
    // Set source: vec!["ax", "vision"] or single source
    // Return merged results
}
```

- [ ] **Add CLI command**

```rust
Commands::Vision(VisionArgs) => {
    // import detections, merge, output
}
```

- [ ] **Commit**

```bash
git add src/platform/macos_vision.rs src/cli.rs
git commit -m "feat(vision): add detection import and hybrid merge hooks"
```

---

### Task 7: Permissions Revamp

**Files:**
- Modify: `src/platform/macos_permissions.rs` — full impl

- [ ] **Add native permission checks**

```rust
pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null_mut()) != 0 }
}

pub fn check_screen_recording() -> bool {
    // Try screencapture, or CGDisplay::CGDisplayStream
    probe_screencapture()
}
```

- [ ] **Add enhanced JSON output**

```json
{
  "platform": "macos",
  "accessibility": true,
  "screen_recording": true,
  "clipboard": true,
  "recommended_mode": "hybrid"
}
```

- [ ] **Commit**

```bash
git add src/platform/macos_permissions.rs
git commit -m "feat(macos): native accessibility permission check with AXIsProcessTrustedWithOptions"
```

---

### Task 8: CI Workflows

**Files:**
- Modify: `.github/workflows/ci.yml` — update
- Create: `.github/workflows/release.yml`

- [ ] **Update CI to cover macOS + Linux, fmt, clippy, test**

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features

  build:
    needs: [lint, test]
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release
```

- [ ] **Create release.yml**

```yaml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  release:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
            suffix: ""
          - os: macos-latest
            target: x86_64-apple-darwin
            suffix: ""
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            suffix: ""
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            suffix: ".exe"
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release --target ${{ matrix.target }}
      - run: |
          BIN="target/${{ matrix.target }}/release/rs-peekaboo${{ matrix.suffix }}"
          ARCHIVE="rs-peekaboo-${{ matrix.target }}.zip"
          7z a "$ARCHIVE" "$BIN" 2>/dev/null || zip "$ARCHIVE" "$BIN"
          sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            rs-peekaboo-*.zip
            rs-peekaboo-*.sha256
          generate_release_notes: true

  publish:
    needs: [release]
    runs-on: ubuntu-latest
    if: env.CARGO_REGISTRY_TOKEN
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo publish --token ${{ secrets.CARGO_REGISTRY_TOKEN }}
```

- [ ] **Commit**

```bash
git add .github/workflows/
git commit -m "ci: add test workflows and release pipeline"
```

---

### Task 9: Version Bump to 0.3.0

**Files:**
- Modify: `Cargo.toml`

- [ ] **Bump version**

```toml
version = "0.3.0"
```

- [ ] **Run all checks**

```bash
cd /Users/undivisible/projects/rs_peekaboo
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo publish --dry-run
```

- [ ] **If all green, publish**

```bash
cargo publish
```

- [ ] **Tag and push**

```bash
git tag -d v0.3.0 2>/dev/null || true
git push origin :refs/tags/v0.3.0 2>/dev/null || true
git tag v0.3.0
git push origin master --tags
```

---

### Task 10: Update poke-around

**Files:**
- Modify: `/Users/undivisible/projects/poke-around/Cargo.toml` 
- Modify: `/Users/undivisible/projects/poke-around/crates/poke-around/src/mcp.rs`

- [ ] **Update dependency version**

```toml
# In workspace Cargo.toml
rs_peekaboo = "0.3.0"
```

- [ ] **Wire mode support in mcp.rs**

Add `PeekabooConfig` usage, default to hybrid on macOS.

- [ ] **Run tests**

```bash
cd /Users/undivisible/projects/poke-around && cargo test --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/poke-around
git add Cargo.toml crates/poke-around/src/mcp.rs Cargo.lock
git commit -m "chore: update rs-peekaboo to 0.3.0 with mode support"
```

---

### Task 11: Update folk-around

**Files:**
- Modify: `/Users/undivisible/projects/folk-around/Cargo.toml`
- Modify: `/Users/undivisible/projects/folk-around/crates/folk-computer-use/src/lib.rs`

- [ ] **Update dependency version**

```toml
rs_peekaboo = "0.3.0"
```

- [ ] **Wire mode support**

- [ ] **Run tests**

```bash
cd /Users/undivisible/projects/folk-around && cargo test --all-features
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/folk-around
git add Cargo.toml crates/folk-computer-use/Cargo.toml crates/folk-computer-use/src/lib.rs Cargo.lock
git commit -m "chore: update rs-peekaboo to 0.3.0 with mode support"
```

---

### Task 12: Homebrew Formula

**Files:**
- Create: `/Users/undivisible/projects/homebrew-tap/Formula/rs-peekaboo.rb`

- [ ] **Create formula**

```ruby
class RsPeekaboo < Formula
  desc "Rust-native cross-platform computer-use CLI and library"
  homepage "https://github.com/undivisible/rs_peekaboo"
  url "https://github.com/undivisible/rs_peekaboo/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "GENERATED_SHA256"
  license "MPL-2.0"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    system "#{bin}/rs-peekaboo", "--help"
  end
end
```

- [ ] **Generate SHA256 from release tarball**

```bash
curl -L -o /tmp/rs-peekaboo.tar.gz https://github.com/undivisible/rs_peekaboo/archive/refs/tags/v0.3.0.tar.gz
sha256sum /tmp/rs-peekaboo.tar.gz
```

- [ ] **Run brew audit and test**

```bash
cd /Users/undivisible/projects/homebrew-tap
brew audit --strict Formula/rs-peekaboo.rb
brew test rs-peekaboo
```

- [ ] **Commit**

```bash
cd /Users/undivisible/projects/homebrew-tap
git add Formula/rs-peekaboo.rb
git commit -m "brew: add rs-peekaboo formula v0.3.0"
```

---

### Task 13: Final Report & Push

- [ ] **Push all repos**

```bash
cd /Users/undivisible/projects/rs_peekaboo && git push origin master --tags
cd /Users/undivisible/projects/poke-around && git push
cd /Users/undivisible/projects/folk-around && git push
cd /Users/undivisible/projects/homebrew-tap && git push
```

- [ ] **Report results**

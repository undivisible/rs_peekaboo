//! Native macOS Accessibility API bindings (direct extern "C" FFI).
//! Links ApplicationServices.framework and CoreFoundation.framework.
//! No objc2 or extra crate dependencies beyond core-graphics (already present).

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use crate::Result;
use crate::models::{Bounds, Point, UiNode};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ptr;

// ── CF type aliases ────────────────────────────────────────────────────

type CFStringRef = *const std::ffi::c_void;
type CFTypeRef = *const std::ffi::c_void;
type CFArrayRef = *const std::ffi::c_void;
type CFNumberRef = *const std::ffi::c_void;
type CFIndex = isize;
type OSStatus = i32;
type Boolean = u8;
type AXUIElementRef = CFTypeRef;
type AXValueRef = CFTypeRef;
type CFAllocatorRef = *const std::ffi::c_void;

const kCFStringEncodingUTF8: u32 = 0x08000100;
const kCFNumberSInt32Type: u32 = 3;

// ── CoreFoundation FFI ─────────────────────────────────────────────────

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        cStr: *const std::ffi::c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFStringGetLength(cf: CFStringRef) -> CFIndex;
    fn CFStringGetCString(
        cf: CFStringRef,
        buffer: *mut std::ffi::c_char,
        bufferSize: CFIndex,
        encoding: u32,
    ) -> Boolean;
    fn CFStringGetCStringPtr(cf: CFStringRef, encoding: u32) -> *const std::ffi::c_char;
    fn CFArrayGetCount(array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, idx: CFIndex) -> CFTypeRef;
    fn CFNumberGetValue(
        number: CFNumberRef,
        theType: u32,
        valuePtr: *mut std::ffi::c_void,
    ) -> Boolean;
}

// ── ApplicationServices (AX) FFI ───────────────────────────────────────

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> Boolean;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> OSStatus;
    fn AXUIElementCopyAttributeValues(
        element: AXUIElementRef,
        attribute: CFStringRef,
        index: CFIndex,
        maxValues: CFIndex,
        values: *mut CFArrayRef,
    ) -> OSStatus;
    fn AXUIElementGetAttributeValueCount(
        element: AXUIElementRef,
        attribute: CFStringRef,
        count: *mut CFIndex,
    ) -> OSStatus;
    fn AXUIElementCopyActionNames(element: AXUIElementRef, names: *mut CFArrayRef) -> OSStatus;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> OSStatus;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> OSStatus;
    fn AXUIElementCopyElementAtPosition(
        element: AXUIElementRef,
        x: f64,
        y: f64,
        value: *mut AXUIElementRef,
    ) -> OSStatus;
    fn AXUIElementCopyAttributeNames(element: AXUIElementRef, names: *mut CFArrayRef) -> OSStatus;
    fn AXValueGetType(value: AXValueRef) -> u32;
    fn AXValueGetValue(value: AXValueRef, theType: u32, valuePtr: *mut std::ffi::c_void)
    -> Boolean;
}

const kAXValueCGPointType: u32 = 1;
const kAXValueCGSizeType: u32 = 2;
const kAXValueCGRectType: u32 = 3;
const kAXErrorSuccess: OSStatus = 0;

// ── CF helpers ─────────────────────────────────────────────────────────

fn cf_string(s: &str) -> CFStringRef {
    let c_str = CString::new(s).unwrap();
    unsafe { CFStringCreateWithCString(ptr::null(), c_str.as_ptr(), kCFStringEncodingUTF8) }
}

fn cf_string_to_string(cf: CFStringRef) -> Option<String> {
    unsafe {
        let raw = CFStringGetCStringPtr(cf, kCFStringEncodingUTF8);
        if !raw.is_null() {
            return Some(CStr::from_ptr(raw).to_string_lossy().into_owned());
        }
        let len = CFStringGetLength(cf);
        if len <= 0 {
            return None;
        }
        let buf_size = len * 4 + 1;
        let mut buf = vec![0u8; buf_size as usize];
        if CFStringGetCString(
            cf,
            buf.as_mut_ptr() as *mut _,
            buf_size as CFIndex,
            kCFStringEncodingUTF8,
        ) != 0
        {
            let pos = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            Some(String::from_utf8_lossy(&buf[..pos]).into_owned())
        } else {
            None
        }
    }
}

fn cf_array_count(array: CFArrayRef) -> usize {
    unsafe { CFArrayGetCount(array) as usize }
}

fn cf_array_get(array: CFArrayRef, idx: usize) -> CFTypeRef {
    unsafe { CFArrayGetValueAtIndex(array, idx as CFIndex) }
}

fn release(cf: CFTypeRef) {
    if !cf.is_null() {
        unsafe { CFRelease(cf) };
    }
}

// ── AX helpers ─────────────────────────────────────────────────────────

fn ax_copy_string(element: AXUIElementRef, attr: &str) -> Option<String> {
    let key = cf_string(attr);
    let mut value: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut value) };
    release(key);
    if status == kAXErrorSuccess && !value.is_null() {
        let s = cf_string_to_string(value);
        release(value);
        return s;
    }
    None
}

fn ax_copy_bool(element: AXUIElementRef, attr: &str) -> Option<bool> {
    let key = cf_string(attr);
    let mut value: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut value) };
    release(key);
    if status == kAXErrorSuccess && !value.is_null() {
        // CFBooleanRef non-null = true
        release(value);
        Some(true)
    } else {
        None
    }
}

fn ax_copy_position(element: AXUIElementRef) -> Option<Point> {
    let key = cf_string("AXPosition");
    let mut value: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut value) };
    release(key);
    if status == kAXErrorSuccess && !value.is_null() {
        let t = unsafe { AXValueGetType(value as AXValueRef) };
        if t == kAXValueCGPointType {
            let mut pt = core_graphics::geometry::CGPoint::new(0.0, 0.0);
            let ok = unsafe {
                AXValueGetValue(
                    value as AXValueRef,
                    kAXValueCGPointType,
                    &mut pt as *mut _ as *mut std::ffi::c_void,
                )
            };
            release(value);
            if ok != 0 {
                return Some(Point {
                    x: pt.x as i64,
                    y: pt.y as i64,
                });
            }
        } else {
            release(value);
        }
    }
    None
}

fn ax_copy_size(element: AXUIElementRef) -> Option<(i64, i64)> {
    let key = cf_string("AXSize");
    let mut value: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut value) };
    release(key);
    if status == kAXErrorSuccess && !value.is_null() {
        let t = unsafe { AXValueGetType(value as AXValueRef) };
        if t == kAXValueCGSizeType {
            let mut sz = core_graphics::geometry::CGSize::new(0.0, 0.0);
            let ok = unsafe {
                AXValueGetValue(
                    value as AXValueRef,
                    kAXValueCGSizeType,
                    &mut sz as *mut _ as *mut std::ffi::c_void,
                )
            };
            release(value);
            if ok != 0 {
                return Some((sz.width as i64, sz.height as i64));
            }
        } else {
            release(value);
        }
    }
    None
}

fn ax_copy_children(element: AXUIElementRef) -> Vec<AXUIElementRef> {
    let key = cf_string("AXChildren");
    let mut array: CFArrayRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut array) };
    release(key);
    if status == kAXErrorSuccess && !array.is_null() {
        let count = cf_array_count(array);
        let mut children = Vec::with_capacity(count);
        for i in 0..count {
            let child = cf_array_get(array, i);
            if !child.is_null() {
                children.push(child);
            }
        }
        release(array);
        children
    } else {
        Vec::new()
    }
}

fn ax_copy_action_names(element: AXUIElementRef) -> Vec<String> {
    let mut array: CFArrayRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyActionNames(element, &mut array) };
    if status == kAXErrorSuccess && !array.is_null() {
        let count = cf_array_count(array);
        let mut names = Vec::with_capacity(count);
        for i in 0..count {
            let item = cf_array_get(array, i);
            if let Some(s) = cf_string_to_string(item) {
                names.push(s);
            }
        }
        release(array);
        names
    } else {
        Vec::new()
    }
}

fn ax_copy_attribute_names(element: AXUIElementRef) -> Vec<String> {
    let mut array: CFArrayRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeNames(element, &mut array) };
    if status == kAXErrorSuccess && !array.is_null() {
        let count = cf_array_count(array);
        let mut names = Vec::with_capacity(count);
        for i in 0..count {
            let item = cf_array_get(array, i);
            if let Some(s) = cf_string_to_string(item) {
                names.push(s);
            }
        }
        release(array);
        names
    } else {
        Vec::new()
    }
}

fn ax_get_pid(element: AXUIElementRef) -> Option<i32> {
    let key = cf_string("AXPID");
    let mut value: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(element, key, &mut value) };
    release(key);
    if status == kAXErrorSuccess && !value.is_null() {
        let mut pid: i32 = 0;
        let ok = unsafe {
            CFNumberGetValue(
                value as CFNumberRef,
                kCFNumberSInt32Type,
                &mut pid as *mut _ as *mut std::ffi::c_void,
            )
        };
        release(value);
        if ok != 0 {
            return Some(pid);
        }
    }
    None
}

// ── Running apps ───────────────────────────────────────────────────────

fn running_app_pids() -> Vec<i32> {
    let script = r#"tell application "System Events" to get unix id of every process whose background only is false"#;
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .split([',', '\n'])
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

// ── Tree building ──────────────────────────────────────────────────────

fn build_node(
    element: AXUIElementRef,
    app_name: &str,
    pid: Option<i32>,
    depth: i32,
    parent_id: Option<String>,
    index: i32,
) -> Option<UiNode> {
    let role = ax_copy_string(element, "AXRole")?;
    let subrole = ax_copy_string(element, "AXSubrole");
    let title = ax_copy_string(element, "AXTitle");
    let desc = ax_copy_string(element, "AXDescription");
    let value = ax_copy_string(element, "AXValue");
    let ident = ax_copy_string(element, "AXIdentifier");
    let enabled = ax_copy_bool(element, "AXEnabled");
    let focused = ax_copy_bool(element, "AXFocused");
    let selected = ax_copy_bool(element, "AXSelected");
    let pos = ax_copy_position(element);
    let sz = ax_copy_size(element);
    let bounds = match (pos, sz) {
        (Some(p), Some((w, h))) => Some(Bounds {
            x: p.x,
            y: p.y,
            width: w,
            height: h,
        }),
        _ => None,
    };
    let actions = ax_copy_action_names(element);
    let attrs_list = ax_copy_attribute_names(element);
    let mut attributes = HashMap::new();
    for a in attrs_list {
        if let Some(v) = ax_copy_string(element, &a) {
            attributes.insert(a, v);
        }
    }

    let id = format!("{}-{}-{}", app_name, role, index);
    Some(UiNode {
        id: id.clone(),
        backend: "native".into(),
        role,
        subrole,
        title,
        label: None,
        description: desc,
        value,
        identifier: ident,
        app: app_name.to_string(),
        pid,
        window: None,
        bounds,
        enabled,
        focused,
        selected,
        depth: Some(depth),
        index_in_parent: Some(index),
        parent_id,
        children: None,
        children_count: None,
        actions,
        attributes,
        confidence: None,
        source: vec!["ax".into()],
        state: serde_json::json!({}),
    })
}

fn build_tree_recursive(
    element: AXUIElementRef,
    app_name: &str,
    pid: Option<i32>,
    depth: i32,
    parent_id: Option<String>,
) -> Vec<UiNode> {
    let children = ax_copy_children(element);
    let mut nodes = Vec::new();
    for (i, child) in children.iter().enumerate() {
        if let Some(mut node) = build_node(
            *child,
            app_name,
            pid,
            depth + 1,
            parent_id.clone(),
            i as i32,
        ) {
            let sub_children =
                build_tree_recursive(*child, app_name, pid, depth + 1, Some(node.id.clone()));
            node.children_count = Some(sub_children.len() as i32);
            nodes.push(node);
            nodes.extend(sub_children);
        }
    }
    nodes
}

// ── AX window tree ─────────────────────────────────────────────────────

fn get_windows_for_app(pid: i32, app_name: &str) -> Vec<UiNode> {
    let app_element = unsafe { AXUIElementCreateApplication(pid) };
    if app_element.is_null() {
        return Vec::new();
    }

    let key = cf_string("AXWindows");
    let mut array: CFArrayRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(app_element, key, &mut array) };
    release(key);

    if status != kAXErrorSuccess || array.is_null() {
        release(app_element);
        return Vec::new();
    }

    let count = cf_array_count(array);
    let mut nodes = Vec::new();
    for i in 0..count {
        let window = cf_array_get(array, i);
        if window.is_null() {
            continue;
        }
        let window_title = ax_copy_string(window, "AXTitle").unwrap_or_default();
        let mut root = build_node(window, app_name, Some(pid), 0, None, i as i32);
        if let Some(ref mut node) = root {
            node.window = Some(window_title.clone());
            let mut children =
                build_tree_recursive(window, app_name, Some(pid), 0, Some(node.id.clone()));
            node.children_count = Some(children.len() as i32);
            nodes.push(node.clone());
            nodes.append(&mut children);
        }
    }
    release(array);
    release(app_element);
    nodes
}

// ── Public API ─────────────────────────────────────────────────────────

/// Check if the current process has accessibility permissions.
pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(ptr::null()) != 0 }
}

/// Return UI nodes from native AX tree traversal.
pub fn ui_elements(app_filter: Option<&str>) -> Result<Vec<UiNode>> {
    let pids = running_app_pids();
    let app_names: Vec<(i32, String)> = pids
        .iter()
        .filter_map(|&pid| {
            let script = format!(
                r#"tell application "System Events" to get name of process whose unix id is {}"#,
                pid
            );
            let out = std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .ok()?;
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() {
                return None;
            }
            if app_filter.is_some_and(|f| !name.eq_ignore_ascii_case(f)) {
                return None;
            }
            Some((pid, name))
        })
        .collect();

    let mut all_nodes = Vec::new();
    for (pid, name) in app_names {
        all_nodes.append(&mut get_windows_for_app(pid, &name));
    }
    Ok(all_nodes)
}

pub fn element_at_point(point: Point) -> Option<UiNode> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }
    let mut element: AXUIElementRef = ptr::null_mut();
    let status = unsafe {
        AXUIElementCopyElementAtPosition(system, point.x as f64, point.y as f64, &mut element)
    };
    release(system);
    if status != kAXErrorSuccess || element.is_null() {
        return None;
    }

    let pid = ax_get_pid(element);
    let app_name = pid
        .and_then(|pid| {
            let script = format!(
                r#"tell application "System Events" to get name of process whose unix id is {}"#,
                pid
            );
            let out = std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .ok()?;
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() { None } else { Some(name) }
        })
        .unwrap_or_default();

    let node = build_node(element, &app_name, pid, 0, None, 0);
    release(element);
    node
}

pub fn focused_element() -> Option<UiNode> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }
    let key = cf_string("AXFocusedUIElement");
    let mut element: CFTypeRef = ptr::null_mut();
    let status = unsafe { AXUIElementCopyAttributeValue(system, key, &mut element) };
    release(key);
    release(system);
    if status != kAXErrorSuccess || element.is_null() {
        return None;
    }

    let pid = ax_get_pid(element);
    let app_name = pid
        .and_then(|pid| {
            let script = format!(
                r#"tell application "System Events" to get name of process whose unix id is {}"#,
                pid
            );
            let out = std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .ok()?;
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() { None } else { Some(name) }
        })
        .unwrap_or_default();

    let node = build_node(element, &app_name, pid, 0, None, 0);
    release(element);
    node
}

// ── Actions via AX ─────────────────────────────────────────────────────

/// # Safety
/// `element` must be a valid non-null AXUIElementRef.
pub unsafe fn ax_perform_action(element: AXUIElementRef, action: &str) -> bool {
    let key = cf_string(action);
    let status = unsafe { AXUIElementPerformAction(element, key) };
    release(key);
    status == kAXErrorSuccess
}

/// # Safety
/// `element` must be a valid non-null AXUIElementRef.
pub unsafe fn ax_set_value(element: AXUIElementRef, value: &str) -> bool {
    let key = cf_string("AXValue");
    let val = cf_string(value);
    let status = unsafe { AXUIElementSetAttributeValue(element, key, val) };
    release(key);
    release(val);
    status == kAXErrorSuccess
}

// ── Click / set-value / perform-action (fallback for now) ──────────────

pub fn click(point: Point, _button: &str, _count: u32) -> Result<Value> {
    super::macos_cg::move_cursor(point)
}

pub fn set_value(element: &UiNode, value: &str) -> Result<Value> {
    let el = crate::UiElement::from(element);
    super::macos_legacy::set_value(&el, value)
}

pub fn perform_action(element: &UiNode, action: &str) -> Result<Value> {
    let el = crate::UiElement::from(element);
    super::macos_legacy::perform_action(&el, action)
}

// Re-export CG functions for use by macos.rs dispatcher
pub use super::macos_cg::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cf_string_roundtrip() {
        let cf = cf_string("AXRole");
        assert!(!cf.is_null());
        let s = cf_string_to_string(cf).expect("should convert");
        assert_eq!(s, "AXRole");
        release(cf);
    }

    #[test]
    fn test_cf_string_empty() {
        let cf = cf_string("");
        assert!(!cf.is_null());
        let s = cf_string_to_string(cf).expect("should convert empty");
        assert_eq!(s, "");
        release(cf);
    }
}

use crate::automation::{Target, parse_point, split_keys};
use crate::models::{Direction, ImageMode, Point};
use crate::{Peekaboo, PeekabooError, Result};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";

pub fn serve(peekaboo: &Peekaboo) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(err) => {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": format!("parse error: {err}") }
                    }),
                )?;
                continue;
            }
        };
        if let Some(response) = handle_request(peekaboo, &request)? {
            write_message(&mut stdout, &response)?;
        }
    }
    Ok(())
}

fn write_message(stdout: &mut impl Write, value: &Value) -> Result<()> {
    writeln!(stdout, "{}", serde_json::to_string(value)?)?;
    stdout.flush()?;
    Ok(())
}

fn handle_request(peekaboo: &Peekaboo, request: &Value) -> Result<Option<Value>> {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let id = request.get("id").cloned();
    if id.is_none() {
        return Ok(None);
    }

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "rs-peekaboo",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_defs() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("name"))?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(peekaboo, name, &args) {
                Ok(data) => Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&data)? }],
                    "structuredContent": data,
                    "isError": false
                })),
                Err(err) => Ok(json!({
                    "content": [{ "type": "text", "text": err.to_string() }],
                    "isError": true
                })),
            }
        }
        "notifications/initialized" | "initialized" => return Ok(None),
        other => Err(PeekabooError::UnsupportedRunCommand(format!("mcp:{other}"))),
    };

    match result {
        Ok(value) => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": value
        }))),
        Err(err) => Ok(Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": err.to_string() }
        }))),
    }
}

fn tool_defs() -> Value {
    json!([
        { "name": "doctor", "description": "Health and permissions report", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "see", "description": "Capture UI snapshot", "inputSchema": { "type": "object", "properties": {
            "app": { "type": "string" }, "mode": { "type": "string" }, "path": { "type": "string" }, "retina": { "type": "boolean" }
        } } },
        { "name": "image", "description": "Capture screen or window image", "inputSchema": { "type": "object", "properties": {
            "mode": { "type": "string" }, "path": { "type": "string" }, "retina": { "type": "boolean" },
            "app": { "type": "string" }, "region": { "type": "object" }
        } } },
        { "name": "list_apps", "description": "List running apps", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "list_windows", "description": "List windows / UI elements", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "list_screens", "description": "List displays", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "click", "description": "Click coords or element query", "inputSchema": { "type": "object", "properties": {
            "coords": { "type": "string" }, "on": { "type": "string" }, "snapshot": { "type": "string" },
            "button": { "type": "string" }, "count": { "type": "integer" }, "background": { "type": "boolean" }
        } } },
        { "name": "type", "description": "Type text", "inputSchema": { "type": "object", "properties": {
            "text": { "type": "string" }, "clear": { "type": "boolean" }, "return": { "type": "boolean" }, "delay_ms": { "type": "integer" }
        }, "required": ["text"] } },
        { "name": "press", "description": "Press a key", "inputSchema": { "type": "object", "properties": {
            "key": { "type": "string" }, "count": { "type": "integer" }, "delay_ms": { "type": "integer" }
        }, "required": ["key"] } },
        { "name": "hotkey", "description": "Press key chord", "inputSchema": { "type": "object", "properties": {
            "keys": { "type": "string" }
        }, "required": ["keys"] } },
        { "name": "scroll", "description": "Scroll", "inputSchema": { "type": "object", "properties": {
            "direction": { "type": "string" }, "amount": { "type": "integer" }
        } } },
        { "name": "move", "description": "Move cursor", "inputSchema": { "type": "object", "properties": {
            "coords": { "type": "string" }, "on": { "type": "string" }, "snapshot": { "type": "string" }
        } } },
        { "name": "set_value", "description": "Set element value", "inputSchema": { "type": "object", "properties": {
            "on": { "type": "string" }, "value": { "type": "string" }, "snapshot": { "type": "string" }
        }, "required": ["on", "value"] } },
        { "name": "perform_action", "description": "AX action on element", "inputSchema": { "type": "object", "properties": {
            "on": { "type": "string" }, "action": { "type": "string" }, "snapshot": { "type": "string" }
        }, "required": ["on", "action"] } },
        { "name": "app", "description": "App control", "inputSchema": { "type": "object", "properties": {
            "action": { "type": "string" }, "app": { "type": "string" }
        }, "required": ["action"] } },
        { "name": "window", "description": "Window control", "inputSchema": { "type": "object", "properties": {
            "action": { "type": "string" }, "app": { "type": "string" }
        }, "required": ["action"] } },
        { "name": "shell", "description": "Run shell command", "inputSchema": { "type": "object", "properties": {
            "command": { "type": "string" }, "cwd": { "type": "string" }
        }, "required": ["command"] } },
        { "name": "permissions", "description": "Permission status", "inputSchema": { "type": "object", "properties": {} } }
    ])
}

fn call_tool(peekaboo: &Peekaboo, name: &str, args: &Value) -> Result<Value> {
    match name {
        "doctor" => peekaboo.doctor(),
        "see" => Ok(serde_json::to_value(
            peekaboo.see(
                args.get("app").and_then(Value::as_str),
                ImageMode::parse_or_err(
                    args.get("mode").and_then(Value::as_str).unwrap_or("screen"),
                )?,
                args.get("path")
                    .and_then(Value::as_str)
                    .map(std::path::PathBuf::from),
                args.get("retina").and_then(Value::as_bool).unwrap_or(false),
            )?,
        )?),
        "image" => {
            if let Some(app) = args.get("app").and_then(Value::as_str) {
                return Ok(serde_json::to_value(
                    peekaboo.image_app(
                        app,
                        args.get("path")
                            .and_then(Value::as_str)
                            .map(std::path::PathBuf::from),
                        args.get("retina").and_then(Value::as_bool).unwrap_or(false),
                    )?,
                )?);
            }
            if let Some(region) = args.get("region") {
                let bounds = crate::Bounds {
                    x: region.get("x").and_then(Value::as_i64).unwrap_or(0),
                    y: region.get("y").and_then(Value::as_i64).unwrap_or(0),
                    width: region.get("width").and_then(Value::as_i64).unwrap_or(0),
                    height: region.get("height").and_then(Value::as_i64).unwrap_or(0),
                };
                return Ok(serde_json::to_value(
                    peekaboo.image_region(
                        bounds,
                        args.get("path")
                            .and_then(Value::as_str)
                            .map(std::path::PathBuf::from),
                        args.get("retina").and_then(Value::as_bool).unwrap_or(false),
                    )?,
                )?);
            }
            Ok(serde_json::to_value(
                peekaboo.image(
                    ImageMode::parse_or_err(
                        args.get("mode").and_then(Value::as_str).unwrap_or("screen"),
                    )?,
                    args.get("path")
                        .and_then(Value::as_str)
                        .map(std::path::PathBuf::from),
                    args.get("retina").and_then(Value::as_bool).unwrap_or(false),
                )?,
            )?)
        }
        "list_apps" => peekaboo.list_apps(),
        "list_windows" => peekaboo.list_windows(),
        "list_screens" => peekaboo.list_screens(),
        "click" => {
            let background = args
                .get("background")
                .and_then(Value::as_bool)
                .unwrap_or(peekaboo.config.background);
            peekaboo.click_with_options(
                target_from_args(args)?,
                args.get("button").and_then(Value::as_str).unwrap_or("left"),
                args.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
                background,
            )
        }
        "type" => peekaboo.type_text(
            args.get("text").and_then(Value::as_str).unwrap_or(""),
            args.get("clear").and_then(Value::as_bool).unwrap_or(false),
            args.get("return")
                .or_else(|| args.get("press_return"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            args.get("delay_ms").and_then(Value::as_u64),
            args.get("app").and_then(Value::as_str),
        ),
        "press" => peekaboo.press(
            args.get("key")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("key"))?,
            args.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
            args.get("delay_ms").and_then(Value::as_u64),
        ),
        "hotkey" => {
            let keys = split_keys(
                args.get("keys")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("keys"))?,
            );
            peekaboo.hotkey(&keys)
        }
        "scroll" => peekaboo.scroll(
            Direction::parse_or_err(
                args.get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("down"),
            )?,
            args.get("amount").and_then(Value::as_u64).unwrap_or(3) as u32,
        ),
        "move" => peekaboo.move_cursor(target_from_args(args)?),
        "set_value" => peekaboo.set_value(
            Target::Query {
                query: args
                    .get("on")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("on"))?
                    .to_string(),
                snapshot: args
                    .get("snapshot")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
            args.get("value")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("value"))?,
        ),
        "perform_action" => peekaboo.perform_action(
            Target::Query {
                query: args
                    .get("on")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("on"))?
                    .to_string(),
                snapshot: args
                    .get("snapshot")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            },
            args.get("action")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("action"))?,
        ),
        "app" => peekaboo.app(
            args.get("action")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("action"))?,
            args.get("app").and_then(Value::as_str),
        ),
        "window" => peekaboo.window(
            args.get("action")
                .and_then(Value::as_str)
                .ok_or(PeekabooError::MissingArgument("action"))?,
            args.get("app").and_then(Value::as_str),
            args.get("title").and_then(Value::as_str),
            None,
        ),
        "shell" => Ok(serde_json::to_value(
            peekaboo.shell(
                args.get("command")
                    .and_then(Value::as_str)
                    .ok_or(PeekabooError::MissingArgument("command"))?,
                args.get("cwd")
                    .and_then(Value::as_str)
                    .map(std::path::Path::new),
            )?,
        )?),
        "permissions" => Ok(peekaboo.permissions()),
        other => Err(PeekabooError::UnsupportedRunCommand(format!(
            "mcp tool:{other}"
        ))),
    }
}

fn target_from_args(args: &Value) -> Result<Target> {
    if let Some(coords) = args.get("coords").and_then(Value::as_str) {
        return Ok(Target::Point(parse_point(coords)?));
    }
    if let (Some(x), Some(y)) = (
        args.get("x").and_then(Value::as_i64),
        args.get("y").and_then(Value::as_i64),
    ) {
        return Ok(Target::Point(Point { x, y }));
    }
    let query = args
        .get("on")
        .or_else(|| args.get("query"))
        .or_else(|| args.get("target"))
        .and_then(Value::as_str)
        .ok_or(PeekabooError::MissingArgument("target"))?;
    Ok(Target::Query {
        query: query.to_string(),
        snapshot: args
            .get("snapshot")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_defs_include_doctor_and_click() {
        let tools = tool_defs();
        let names: Vec<&str> = tools
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .collect();
        assert!(names.contains(&"doctor"));
        assert!(names.contains(&"click"));
        assert!(names.contains(&"see"));
    }

    #[test]
    fn target_from_args_accepts_coords() {
        let target = target_from_args(&json!({ "coords": "1,2" })).unwrap();
        match target {
            Target::Point(p) => assert_eq!(p, Point { x: 1, y: 2 }),
            _ => panic!("expected point"),
        }
    }
}

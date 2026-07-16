# rs_peekaboo

Rust-native cross-platform computer-use CLI and library for macOS, Windows, and Linux.

`rs_peekaboo` focuses on capture, UI inspection, input, app/window/menu control,
clipboard, permissions, scripts, and structured JSON output. It intentionally
does not include model providers, hosted API keys, telemetry, or an assistant UI.

## Install

From crates.io after release:

```bash
cargo install rs_peekaboo
```

From source:

```bash
git clone https://github.com/undivisible/rs_peekaboo
cd rs_peekaboo
cargo build --release
cargo install --path .
```

## CLI

```bash
rs-peekaboo image --mode screen --path ~/Desktop/screen.png
rs-peekaboo image --app Safari --path ~/Desktop/safari.png
rs-peekaboo see --app Safari --json
rs-peekaboo click --coords 500,300
rs-peekaboo click --index 3 --snapshot snap-... --background
rs-peekaboo type "hello" --return
rs-peekaboo hotkey cmd,l
rs-peekaboo window list --json
rs-peekaboo app launch --app Safari
rs-peekaboo clipboard read --json
rs-peekaboo doctor --json
rs-peekaboo shell "fastfetch --logo none" --json
rs-peekaboo mcp   # MCP stdio server for agent hosts
```

Global flags:

| Flag | Description |
| --- | --- |
| `--json` | Print structured JSON instead of human output. |
| `--json-output` | Alias for `--json`. |
| `--background` | Prefer AX/background actions that avoid focus steal. |
| `--mode` | Computer-use backend: `hybrid` (default macOS), `native`, `vision`, `legacy`, `coords`. |

Commands:

| Command | Purpose |
| --- | --- |
| `see` | Capture an image and cache a UI snapshot. |
| `image` | Capture a screen or focused window image. |
| `list apps` | List running apps/processes. |
| `list windows` | Return app/window accessibility context. |
| `list screens` | Return display information. |
| `list menubar` | Inspect the system menu bar. |
| `list permissions` | Probe screen recording, accessibility, and clipboard access. |
| `click` | Click a coordinate or UI element query. |
| `type` | Type text into the active UI. |
| `press` | Press a named key. |
| `hotkey` | Press a key combination such as `cmd,l`. |
| `paste` | Paste text while restoring the previous clipboard where possible. |
| `scroll` | Scroll up, down, left, or right. |
| `swipe` | Swipe between two coordinates. |
| `drag` | Drag between two coordinates. |
| `move` | Move the pointer to a coordinate or UI element query. |
| `set-value` | Set the value of a resolved UI element. |
| `perform-action` | Perform an accessibility action on a resolved UI element. |
| `window` | List, focus, close, minimize, move, resize, or set window bounds. |
| `app` | List, launch, switch, quit, hide, or unhide apps. |
| `open` | Open a path or URL, optionally with a specific app. |
| `menu` | List or click menu items. |
| `clipboard` | Read or write the clipboard. |
| `permissions` | Probe automation permissions. |
| `shell` | Run a shell command and return stdout, stderr, and exit status. |
| `run` | Execute a JSON automation script. |
| `sleep` | Sleep for a number of seconds. |
| `clean` | Remove cached snapshots. |
| `doctor` | Health report: permissions, tools, capabilities. |
| `mcp` | Speak MCP over stdio for agent harnesses. |
| `tools` | Print the command catalog. |
| `completions` | Generate shell completions. |

## JSON Output

```bash
rs-peekaboo image --mode screen --json
```

```json
{
  "ok": true,
  "data": {
    "path": "/var/folders/.../rs_peekaboo_x.png",
    "mode": "screen",
    "bytes": 123456,
    "mime_type": "image/png"
  }
}
```

```bash
rs-peekaboo see --app Safari --json
```

```json
{
  "ok": true,
  "data": {
    "snapshot_id": "snap-...",
    "elements": [
      {
        "id": "window:Safari:Example",
        "role": "window",
        "label": "Example",
        "app": "Safari",
        "window": "Example",
        "index": 0,
        "bounds": {
          "x": 0,
          "y": 25,
          "width": 1200,
          "height": 800
        },
        "state": {
          "minimized": false
        }
      }
    ]
  }
}
```

Click by stable snapshot index:

```bash
rs-peekaboo click --index 0 --snapshot snap-... --background --json
```

## Library

```rust
use rs_peekaboo::automation::Target;
use rs_peekaboo::{Bounds, Direction, ImageMode, Peekaboo, Point};

fn main() -> rs_peekaboo::Result<()> {
    let peekaboo = Peekaboo::new();

    let image = peekaboo.image(ImageMode::Screen, None, true)?;
    let region = peekaboo.image_region(
        Bounds {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        None,
        true,
    )?;
    let elements = peekaboo.ui_elements(None)?;

    peekaboo.click(Target::Point(Point { x: 500, y: 300 }), "left", 1)?;
    peekaboo.type_text("hello", false, false, None)?;
    peekaboo.hotkey(&["cmd", "l"])?;
    peekaboo.scroll(Direction::Down, 3)?;
    peekaboo.window("focus", Some("Safari"), None)?;
    peekaboo.app("launch", Some("Safari"))?;
    peekaboo.menu("list", "Safari", None, None)?;
    peekaboo.clipboard_write("copied")?;

    println!("{} {} {}", image.bytes, region.bytes, elements.len());
    Ok(())
}
```

## Scripts

`run` accepts a JSON file with ordered steps:

```json
{
  "steps": [
    {
      "command": "hotkey",
      "args": {
        "keys": "cmd,l"
      }
    },
    {
      "command": "type",
      "args": {
        "text": "https://example.com"
      }
    },
    {
      "command": "sleep",
      "args": {
        "duration_ms": 250
      }
    }
  ]
}
```

```bash
rs-peekaboo run ./script.json --json
```

Shell steps are also supported:

```json
{
  "steps": [
    {
      "command": "shell",
      "args": {
        "command": "pwd",
        "cwd": "/tmp"
      }
    }
  ]
}
```

## Permissions

Most actions require macOS Accessibility permission for the terminal or host app
running `rs-peekaboo`. Screen capture requires Screen Recording permission.
Clipboard commands require clipboard access in the current user session.

Probe status:

```bash
rs-peekaboo permissions status --json
```

## Platform

| Platform | Capture | Input | UI snapshot | Notes |
| --- | --- | --- | --- | --- |
| macOS | `screencapture` | Accessibility / CoreGraphics | AppleScript | Full parity |
| Windows | GDI+ screenshot | `SendInput` + `SendKeys` | UI Automation | PowerShell backend |
| Linux | `grim` / `scrot` / ImageMagick | `xdotool` | `wmctrl` | X11-focused; Wayland uses `grim` + `wl-clipboard` where available |

Shell execution is cross-platform and uses the host shell.

## Folk Around

`rs_peekaboo` is standalone. It is designed so MCP hosts such as
`folk-around` can call the Rust computer-use engine directly instead of
shelling out to AppleScript helpers.

## License

MPL-2.0.

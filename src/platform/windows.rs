use crate::PeekabooError;
use crate::Result;
use crate::models::{Bounds, Direction, ImageMode, Point, UiElement};
use crate::platform::process;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;

const INPUT_ENV_VAR: &str = "RS_PEEKABOO_INPUT";

pub fn capture_image(
    mode: ImageMode,
    path: &Path,
    _retina: bool,
    region: Option<&Bounds>,
) -> Result<()> {
    if mode != ImageMode::Screen {
        return Err(PeekabooError::UnsupportedPlatform("image mode"));
    }
    let input = json!({
        "path": path.to_string_lossy(),
        "region": region,
    });
    run_json(IMAGE_SCRIPT, &input)?;
    Ok(())
}

pub fn ui_elements(app: Option<&str>) -> Result<Vec<UiElement>> {
    let value = run_json(UI_ELEMENTS_SCRIPT, &json!({ "app": app }))?;
    Ok(serde_json::from_value(value)?)
}

pub fn list_screens() -> Result<Value> {
    run_json(LIST_SCREENS_SCRIPT, &json!({}))
}

pub fn permissions() -> Value {
    json!({
        "screen_recording": true,
        "accessibility": true,
        "clipboard": true,
        "platform": "windows"
    })
}

pub fn click(point: Point, button: &str, count: u32) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "click",
            "x": point.x,
            "y": point.y,
            "button": button,
            "count": count.max(1),
        }),
    )
}

pub fn move_cursor(point: Point) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "move",
            "x": point.x,
            "y": point.y,
        }),
    )
}

pub fn drag(from: Point, to: Point, duration_ms: u64) -> Result<Value> {
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "drag",
            "from_x": from.x,
            "from_y": from.y,
            "to_x": to.x,
            "to_y": to.y,
            "duration_ms": duration_ms,
        }),
    )
}

pub fn scroll(direction: Direction, amount: u32) -> Result<Value> {
    let amount = amount.max(1) as i64;
    let (dx, dy) = match direction {
        Direction::Left => (-amount, 0),
        Direction::Right => (amount, 0),
        Direction::Up => (0, -amount),
        Direction::Down => (0, amount),
    };
    run_json(
        MOUSE_SCRIPT,
        &json!({
            "action": "scroll",
            "dx": dx,
            "dy": dy,
        }),
    )
}

pub fn press(key: &str, count: u32, delay_ms: Option<u64>) -> Result<Value> {
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "press",
            "keys": send_keys_for_key(key),
            "count": count.max(1),
            "delay_ms": delay_ms.unwrap_or(0),
        }),
    )
}

pub fn hotkey(keys: &[&str]) -> Result<Value> {
    let chord = keys.iter().copied().collect::<Vec<_>>().join("+");
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "hotkey",
            "keys": send_keys_for_hotkey(&chord),
        }),
    )
}

pub fn type_text(
    text: &str,
    clear: bool,
    press_return: bool,
    delay_ms: Option<u64>,
) -> Result<Value> {
    run_json(
        KEYBOARD_SCRIPT,
        &json!({
            "action": "type",
            "text": text,
            "clear": clear,
            "return": press_return,
            "delay_ms": delay_ms.unwrap_or(0),
        }),
    )
}

pub fn paste(text: &str) -> Result<Value> {
    clipboard_write(text)?;
    hotkey(&["ctrl", "v"])
}

pub fn set_value(point: Point, value: &str) -> Result<Value> {
    click(point, "left", 1)?;
    type_text(value, true, false, None)
}

pub fn perform_action(point: Point, action: &str) -> Result<Value> {
    match action {
        "right_click" | "right-click" => click(point, "right", 1),
        "double_click" | "double-click" | "open" | "press" => click(point, "left", 2),
        _ => click(point, "left", 1),
    }
}

pub fn list_apps() -> Result<Value> {
    run_json(APP_SCRIPT, &json!({ "action": "list" }))
}

pub fn app(action: &str, name: Option<&str>) -> Result<Value> {
    run_json(
        APP_SCRIPT,
        &json!({
            "action": action,
            "app": name,
        }),
    )
}

pub fn open(path_or_url: &str, app: Option<&str>, _no_focus: bool) -> Result<Value> {
    run_json(
        OPEN_SCRIPT,
        &json!({
            "target": path_or_url,
            "app": app,
        }),
    )
}

pub fn window(
    action: &str,
    app: Option<&str>,
    title: Option<&str>,
    bounds: Option<Bounds>,
) -> Result<Value> {
    run_json(
        WINDOW_SCRIPT,
        &json!({
            "action": action,
            "app": app,
            "title": title,
            "bounds": bounds,
        }),
    )
}

pub fn menu(action: &str, app: &str, menu: Option<&str>, item: Option<&str>) -> Result<Value> {
    run_json(
        MENU_SCRIPT,
        &json!({
            "action": action,
            "app": app,
            "menu": menu,
            "item": item,
        }),
    )
}

pub fn clipboard_read() -> Result<String> {
    let value = run_json(CLIPBOARD_SCRIPT, &json!({ "action": "read" }))?;
    Ok(value
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

pub fn clipboard_write(text: &str) -> Result<Value> {
    run_json(
        CLIPBOARD_SCRIPT,
        &json!({ "action": "write", "text": text }),
    )
}

pub fn send_keys_for_hotkey(keys: &str) -> String {
    keys.split('+')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| match key.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "win" | "windows" | "super" | "meta" | "ctrl" | "control" => {
                "^".to_string()
            }
            "shift" => "+".to_string(),
            "alt" | "option" => "%".to_string(),
            other => send_keys_for_key(other),
        })
        .collect::<String>()
}

pub fn send_keys_for_key(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "{ENTER}".to_string(),
        "escape" | "esc" => "{ESC}".to_string(),
        "tab" => "{TAB}".to_string(),
        "space" => " ".to_string(),
        "backspace" => "{BACKSPACE}".to_string(),
        "delete" | "del" => "{DELETE}".to_string(),
        "up" | "arrowup" => "{UP}".to_string(),
        "down" | "arrowdown" => "{DOWN}".to_string(),
        "left" | "arrowleft" => "{LEFT}".to_string(),
        "right" | "arrowright" => "{RIGHT}".to_string(),
        "home" => "{HOME}".to_string(),
        "end" => "{END}".to_string(),
        "pageup" => "{PGUP}".to_string(),
        "pagedown" => "{PGDN}".to_string(),
        f if function_key(f).is_some() => format!("{{{}}}", function_key(f).unwrap_or("")),
        other => other.to_string(),
    }
}

fn function_key(key: &str) -> Option<&'static str> {
    match key {
        "f1" => Some("F1"),
        "f2" => Some("F2"),
        "f3" => Some("F3"),
        "f4" => Some("F4"),
        "f5" => Some("F5"),
        "f6" => Some("F6"),
        "f7" => Some("F7"),
        "f8" => Some("F8"),
        "f9" => Some("F9"),
        "f10" => Some("F10"),
        "f11" => Some("F11"),
        "f12" => Some("F12"),
        _ => None,
    }
}

fn run_json(script: &str, input: &Value) -> Result<Value> {
    let input_json = serde_json::to_string(input)?;
    let command = prepare_powershell_command(script);
    let output = std::process::Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-STA")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(command)
        .env(INPUT_ENV_VAR, input_json)
        .stdin(Stdio::null())
        .output()
        .map_err(PeekabooError::Io)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(PeekabooError::System(detail));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).map_err(PeekabooError::from)
}

pub fn prepare_powershell_command(script: &str) -> String {
    let body = script
        .trim()
        .strip_prefix("$inputObject = $args[0] | ConvertFrom-Json")
        .unwrap_or(script)
        .trim_start_matches(['\r', '\n']);
    format!("$inputObject = $env:{INPUT_ENV_VAR} | ConvertFrom-Json\n{body}")
}

const IMAGE_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$path = $inputObject.path
$parent = Split-Path -Parent $path
if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
if ($inputObject.region) {
  $sourceX = [int]$inputObject.region.x
  $sourceY = [int]$inputObject.region.y
  $width = [int]$inputObject.region.width
  $height = [int]$inputObject.region.height
} else {
  $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
  $sourceX = $bounds.Left
  $sourceY = $bounds.Top
  $width = $bounds.Width
  $height = $bounds.Height
}
$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($sourceX, $sourceY, 0, 0, $bitmap.Size)
$bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
[pscustomobject]@{ ok = $true; path = $path } | ConvertTo-Json -Compress
"#;

const UI_ELEMENTS_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$condition = [System.Windows.Automation.Condition]::TrueCondition
$root = [System.Windows.Automation.AutomationElement]::RootElement
$collection = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $condition)
$items = New-Object System.Collections.Generic.List[object]
$index = 0
foreach ($element in $collection) {
  $name = $element.Current.Name
  if ($inputObject.app -and ($name -notlike ("*" + $inputObject.app + "*"))) { continue }
  $rect = $element.Current.BoundingRectangle
  $bounds = if ($rect.IsEmpty) { $null } else { [pscustomobject]@{ x = [int64]$rect.X; y = [int64]$rect.Y; width = [int64]$rect.Width; height = [int64]$rect.Height } }
  $items.Add([pscustomobject]@{ id = "win-$index"; role = $element.Current.ControlType.ProgrammaticName; label = $name; app = $name; window = $name; bounds = $bounds; state = [pscustomobject]@{ native_window_handle = $element.Current.NativeWindowHandle; enabled = $element.Current.IsEnabled } })
  $index += 1
}
$items | ConvertTo-Json -Compress -Depth 8
"#;

const LIST_SCREENS_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
$screens = [System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
  [pscustomobject]@{ name = $_.DeviceName; primary = $_.Primary; x = $_.Bounds.X; y = $_.Bounds.Y; width = $_.Bounds.Width; height = $_.Bounds.Height; scale_factor = 1 }
}
[pscustomobject]@{ screens = @($screens) } | ConvertTo-Json -Compress -Depth 4
"#;

const MOUSE_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName WindowsBase
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Threading;

[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }

[StructLayout(LayoutKind.Sequential)]
public struct MOUSEINPUT {
  public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
}

[StructLayout(LayoutKind.Explicit)]
public struct INPUT {
  [FieldOffset(0)] public uint type;
  [FieldOffset(8)] public MOUSEINPUT mi;
}

public class NativeMouse {
  public const uint INPUT_MOUSE = 0;
  public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
  public const uint MOUSEEVENTF_LEFTUP = 0x0004;
  public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
  public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
  public const uint MOUSEEVENTF_WHEEL = 0x0800;
  public static readonly int InputSize = Marshal.SizeOf(typeof(INPUT));

  [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
  [DllImport("user32.dll", SetLastError = true)] static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT pt);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr ProcessId);
  [DllImport("user32.dll")] static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint attachTo, bool attach);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT lpPoint);

  public static void EnsureDpiAware() {
    try { SetProcessDpiAwarenessContext((IntPtr)(-4)); } catch {}
  }

  public static void SendMouse(uint flags, uint data = 0) {
    var inputs = new INPUT[1];
    inputs[0].type = INPUT_MOUSE;
    inputs[0].mi.dwFlags = flags;
    inputs[0].mi.mouseData = data;
    if (SendInput(1, inputs, InputSize) != 1) {
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
  }

  public static void FocusAt(int x, int y) {
    var hwnd = WindowFromPoint(new POINT { X = x, Y = y });
    if (hwnd == IntPtr.Zero) return;
    var fg = GetForegroundWindow();
    if (fg == hwnd) return;
    uint fgThread = GetWindowThreadProcessId(fg, IntPtr.Zero);
    uint curThread = GetCurrentThreadId();
    if (fgThread != curThread) AttachThreadInput(curThread, fgThread, true);
    SetForegroundWindow(hwnd);
    if (fgThread != curThread) AttachThreadInput(curThread, fgThread, false);
    Thread.Sleep(50);
  }

  public static void MoveTo(int x, int y) {
    if (!SetCursorPos(x, y)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    Thread.Sleep(20);
  }

  public static void Click(string button, int count) {
    uint down = button == "right" ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_LEFTDOWN;
    uint up = button == "right" ? MOUSEEVENTF_RIGHTUP : MOUSEEVENTF_LEFTUP;
    for (int i = 0; i < count; i++) {
      SendMouse(down);
      Thread.Sleep(10);
      SendMouse(up);
      Thread.Sleep(20);
    }
  }
}
"@

function Invoke-UiClick([int]$x, [int]$y) {
  $point = New-Object System.Windows.Point $x, $y
  $element = [System.Windows.Automation.AutomationElement]::FromPoint($point)
  if (-not $element) { return $false }
  $invoke = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
  if ($invoke) { $invoke.Invoke(); return $true }
  $toggle = $element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
  if ($toggle) { $toggle.Toggle(); return $true }
  return $false
}

[NativeMouse]::EnsureDpiAware()
$usedFallback = $false
if ($inputObject.action -eq "move") {
  [NativeMouse]::MoveTo([int]$inputObject.x, [int]$inputObject.y)
} elseif ($inputObject.action -eq "click") {
  $x = [int]$inputObject.x
  $y = [int]$inputObject.y
  [NativeMouse]::FocusAt($x, $y)
  [NativeMouse]::MoveTo($x, $y)
  if ($inputObject.button -eq "left" -and [int]$inputObject.count -eq 1 -and (Invoke-UiClick $x $y)) {
    $usedFallback = $true
  } else {
    try {
      [NativeMouse]::Click([string]$inputObject.button, [int]$inputObject.count)
    } catch {
      if (-not (Invoke-UiClick $x $y)) { throw }
      $usedFallback = $true
    }
  }
} elseif ($inputObject.action -eq "drag") {
  [NativeMouse]::MoveTo([int]$inputObject.from_x, [int]$inputObject.from_y)
  [NativeMouse]::FocusAt([int]$inputObject.from_x, [int]$inputObject.from_y)
  [NativeMouse]::SendMouse([NativeMouse]::MOUSEEVENTF_LEFTDOWN)
  Start-Sleep -Milliseconds ([Math]::Max(50, [int]$inputObject.duration_ms))
  [NativeMouse]::MoveTo([int]$inputObject.to_x, [int]$inputObject.to_y)
  [NativeMouse]::SendMouse([NativeMouse]::MOUSEEVENTF_LEFTUP)
} elseif ($inputObject.action -eq "scroll") {
  $delta = if ([int]$inputObject.dy -ne 0) { -120 * [Math]::Sign([int]$inputObject.dy) } else { -120 * [Math]::Sign([int]$inputObject.dx) }
  [NativeMouse]::SendMouse([NativeMouse]::MOUSEEVENTF_WHEEL, [uint32]$delta)
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; fallback = $usedFallback; input_size = [NativeMouse]::InputSize } | ConvertTo-Json -Compress
"#;

const KEYBOARD_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Threading;

[StructLayout(LayoutKind.Sequential)]
public struct POINT { public int X; public int Y; }

[StructLayout(LayoutKind.Sequential)]
public struct KEYBDINPUT {
  public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo;
}

[StructLayout(LayoutKind.Explicit)]
public struct INPUT {
  [FieldOffset(0)] public uint type;
  [FieldOffset(8)] public KEYBDINPUT ki;
}

public class NativeKeyboard {
  public const uint INPUT_KEYBOARD = 1;
  public const uint KEYEVENTF_KEYUP = 0x0002;
  public static readonly int InputSize = Marshal.SizeOf(typeof(INPUT));

  [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
  [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(POINT pt);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr ProcessId);
  [DllImport("user32.dll")] static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint attachTo, bool attach);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT lpPoint);

  public static void EnsureDpiAware() {
    try { SetProcessDpiAwarenessContext((IntPtr)(-4)); } catch {}
  }

  public static void FocusCursorWindow() {
    POINT pt;
    if (!GetCursorPos(out pt)) return;
    var hwnd = WindowFromPoint(pt);
    if (hwnd == IntPtr.Zero) return;
    var fg = GetForegroundWindow();
    if (fg == hwnd) return;
    uint fgThread = GetWindowThreadProcessId(fg, IntPtr.Zero);
    uint curThread = GetCurrentThreadId();
    if (fgThread != curThread) AttachThreadInput(curThread, fgThread, true);
    SetForegroundWindow(hwnd);
    if (fgThread != curThread) AttachThreadInput(curThread, fgThread, false);
    Thread.Sleep(50);
  }

  static void SendKey(ushort vk, bool keyUp) {
    var inputs = new INPUT[1];
    inputs[0].type = INPUT_KEYBOARD;
    inputs[0].ki.wVk = vk;
    if (keyUp) inputs[0].ki.dwFlags = KEYEVENTF_KEYUP;
    if (SendInput(1, inputs, InputSize) != 1) {
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
  }

  public static void Tap(ushort vk) {
    SendKey(vk, false);
    Thread.Sleep(10);
    SendKey(vk, true);
  }

  public static void Chord(ushort mod, ushort key) {
    SendKey(mod, false);
    SendKey(key, false);
    Thread.Sleep(10);
    SendKey(key, true);
    SendKey(mod, true);
  }
}
"@

function Send-KeysWait([string]$keys) {
  [NativeKeyboard]::FocusCursorWindow()
  [System.Windows.Forms.SendKeys]::SendWait($keys)
  Start-Sleep -Milliseconds 40
}

[NativeKeyboard]::EnsureDpiAware()
[NativeKeyboard]::FocusCursorWindow()
if ($inputObject.action -eq "type") {
  if ($inputObject.clear) {
    Send-KeysWait("^a")
    Send-KeysWait("{BACKSPACE}")
  }
  $text = [string]$inputObject.text
  [System.Windows.Forms.Clipboard]::SetText($text)
  Start-Sleep -Milliseconds 80
  Send-KeysWait("^v")
  if ($inputObject.return) { Send-KeysWait("{ENTER}") }
} else {
  for ($i = 0; $i -lt [int]$inputObject.count; $i++) {
    Send-KeysWait([string]$inputObject.keys)
    if ([int]$inputObject.delay_ms -gt 0) { Start-Sleep -Milliseconds ([int]$inputObject.delay_ms) }
  }
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; input_size = [NativeKeyboard]::InputSize } | ConvertTo-Json -Compress
"#;

const WINDOW_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class NativeWindow {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
}
"@
$windows = Get-Process | Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle }
if ($inputObject.action -eq "list") {
  $windows | ForEach-Object { [pscustomobject]@{ app = $_.ProcessName; title = $_.MainWindowTitle; pid = $_.Id; handle = $_.MainWindowHandle.ToInt64() } } | ConvertTo-Json -Compress
  exit
}
$target = $windows | Where-Object {
  (-not $inputObject.app -or $_.ProcessName -like ("*" + $inputObject.app + "*")) -and
  (-not $inputObject.title -or $_.MainWindowTitle -like ("*" + $inputObject.title + "*"))
} | Select-Object -First 1
if (-not $target) { throw "window not found" }
if ($inputObject.action -in @("focus", "activate", "switch")) {
  [NativeWindow]::SetForegroundWindow($target.MainWindowHandle) | Out-Null
} elseif ($inputObject.action -eq "minimize") {
  [NativeWindow]::ShowWindow($target.MainWindowHandle, 6) | Out-Null
} elseif ($inputObject.action -in @("restore", "unhide")) {
  [NativeWindow]::ShowWindow($target.MainWindowHandle, 9) | Out-Null
  [NativeWindow]::SetForegroundWindow($target.MainWindowHandle) | Out-Null
} elseif ($inputObject.action -eq "close") {
  [NativeWindow]::PostMessage($target.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
} elseif ($inputObject.action -in @("move", "resize", "set-bounds")) {
  $b = $inputObject.bounds
  [NativeWindow]::SetWindowPos($target.MainWindowHandle, [IntPtr]::Zero, [int]$b.x, [int]$b.y, [int]$b.width, [int]$b.height, 0x0040) | Out-Null
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; app = $target.ProcessName; title = $target.MainWindowTitle } | ConvertTo-Json -Compress
"#;

const APP_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.action -eq "list") {
  Get-Process | Where-Object { $_.MainWindowTitle } | ForEach-Object { [pscustomobject]@{ app = $_.ProcessName; title = $_.MainWindowTitle; pid = $_.Id } } | ConvertTo-Json -Compress
  exit
}
if ($inputObject.action -in @("launch", "open", "switch", "activate")) {
  Start-Process -FilePath ([string]$inputObject.app)
} else {
  $process = Get-Process | Where-Object { $_.ProcessName -like ("*" + $inputObject.app + "*") } | Select-Object -First 1
  if (-not $process) { throw "app not found" }
  if ($inputObject.action -in @("quit", "close")) {
    $process.CloseMainWindow() | Out-Null
  }
}
[pscustomobject]@{ ok = $true; action = $inputObject.action; app = $inputObject.app } | ConvertTo-Json -Compress
"#;

const OPEN_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.app) {
  Start-Process -FilePath ([string]$inputObject.app) -ArgumentList ([string]$inputObject.target)
} else {
  Start-Process -FilePath ([string]$inputObject.target)
}
[pscustomobject]@{ ok = $true; target = $inputObject.target } | ConvertTo-Json -Compress
"#;

const MENU_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
if ($inputObject.action -in @("list", "list-all", "inspect")) {
  [pscustomobject]@{ menus = @(); note = "Windows menu inspection is exposed through UI Automation snapshots." } | ConvertTo-Json -Compress
  exit
}
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait("%")
[pscustomobject]@{ ok = $true; action = $inputObject.action; menu = $inputObject.menu; item = $inputObject.item } | ConvertTo-Json -Compress
"#;

const CLIPBOARD_SCRIPT: &str = r#"
$inputObject = $args[0] | ConvertFrom-Json
Add-Type -AssemblyName System.Windows.Forms
if ($inputObject.action -eq "write") {
  [System.Windows.Forms.Clipboard]::SetText([string]$inputObject.text)
  [pscustomobject]@{ ok = $true; bytes = ([string]$inputObject.text).Length } | ConvertTo-Json -Compress
} else {
  [pscustomobject]@{ text = [System.Windows.Forms.Clipboard]::GetText() } | ConvertTo-Json -Compress
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_powershell_command_should_read_input_from_env() {
        let command = prepare_powershell_command(
            "$inputObject = $args[0] | ConvertFrom-Json\nWrite-Output ok",
        );
        assert!(command.contains("RS_PEEKABOO_INPUT"));
        assert!(!command.contains("$args[0]"));
    }

    #[test]
    fn send_keys_for_hotkey_should_build_chords() {
        assert_eq!(send_keys_for_hotkey("ctrl+shift+p"), "^+p");
        assert_eq!(send_keys_for_hotkey("alt+f4"), "%{F4}");
    }
}

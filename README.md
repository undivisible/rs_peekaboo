# rs_peekaboo

Rust-native rewrite of Peekaboo's computer-use core.

`rs_peekaboo` focuses on capture, UI inspection, input, app/window/menu control,
clipboard, permissions, scripts, and structured JSON output. It intentionally
does not include model providers, hosted API keys, telemetry, or an assistant UI.

## Build

```bash
cargo build --release
```

## Examples

```bash
rs-peekaboo image --mode screen --path ~/Desktop/screen.png
rs-peekaboo see --app Safari --json
rs-peekaboo click --coords 500,300
rs-peekaboo type "hello" --return
rs-peekaboo hotkey cmd,l
rs-peekaboo window list --json
rs-peekaboo app launch --app Safari
rs-peekaboo clipboard read --json
```

## Commands

`see`, `image`, `list`, `click`, `type`, `press`, `hotkey`, `paste`, `scroll`,
`swipe`, `drag`, `move`, `set-value`, `perform-action`, `window`, `app`, `open`,
`menu`, `clipboard`, `permissions`, `run`, `sleep`, `clean`, `tools`, and
`completions`.

## Platform

macOS is the primary target. Non-macOS builds compile and return explicit
unsupported-platform errors for macOS automation commands.

## License

MPL-2.0.

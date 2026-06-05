use crate::automation::{Target, parse_point, split_keys};
use crate::cache;
use crate::{Bounds, CommandResult, Direction, ImageMode, Peekaboo, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, shells};
use serde_json::{Value, json};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "rs-peekaboo")]
#[command(about = "Rust-native macOS automation CLI")]
pub struct Cli {
    #[arg(long, global = true, visible_alias = "json-output")]
    pub json: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    See(SeeArgs),
    Image(ImageArgs),
    List(ListArgs),
    Click(ClickArgs),
    Type(TypeArgs),
    Press(PressArgs),
    Hotkey(HotkeyArgs),
    Paste(PasteArgs),
    Scroll(ScrollArgs),
    Swipe(DragArgs),
    Drag(DragArgs),
    Move(MoveArgs),
    SetValue(SetValueArgs),
    PerformAction(PerformActionArgs),
    Window(WindowArgs),
    App(AppArgs),
    Open(OpenArgs),
    Menu(MenuArgs),
    Clipboard(ClipboardArgs),
    Permissions(PermissionsArgs),
    Shell(ShellArgs),
    Run(RunArgs),
    Sleep(SleepArgs),
    Clean(CleanArgs),
    Tools(ToolsArgs),
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub struct SeeArgs {
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long, default_value = "screen")]
    pub mode: String,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub retina: bool,
}

#[derive(Args, Debug)]
pub struct ImageArgs {
    #[arg(long, default_value = "screen")]
    pub mode: String,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub retina: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    #[command(subcommand)]
    pub kind: ListKind,
}

#[derive(Subcommand, Debug)]
pub enum ListKind {
    Apps,
    Windows,
    Screens,
    Menubar,
    Permissions,
}

#[derive(Args, Debug)]
pub struct ClickArgs {
    pub target: Option<String>,
    #[arg(long)]
    pub on: Option<String>,
    #[arg(long)]
    pub coords: Option<String>,
    #[arg(long)]
    pub snapshot: Option<String>,
    #[arg(long, default_value = "left")]
    pub button: String,
    #[arg(long, default_value_t = 1)]
    pub count: u32,
}

#[derive(Args, Debug)]
pub struct TypeArgs {
    pub text: Option<String>,
    #[arg(long)]
    pub clear: bool,
    #[arg(long = "return")]
    pub press_return: bool,
    #[arg(long)]
    pub delay: Option<u64>,
}

#[derive(Args, Debug)]
pub struct PressArgs {
    pub key: String,
    #[arg(long, default_value_t = 1)]
    pub count: u32,
    #[arg(long)]
    pub delay: Option<u64>,
}

#[derive(Args, Debug)]
pub struct HotkeyArgs {
    pub keys: String,
}

#[derive(Args, Debug)]
pub struct PasteArgs {
    pub text: String,
}

#[derive(Args, Debug)]
pub struct ScrollArgs {
    #[arg(long, default_value = "down")]
    pub direction: String,
    #[arg(long, default_value_t = 3)]
    pub amount: u32,
}

#[derive(Args, Debug)]
pub struct DragArgs {
    #[arg(long)]
    pub from: String,
    #[arg(long)]
    pub to: String,
    #[arg(long, default_value_t = 250)]
    pub duration: u64,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    pub target: Option<String>,
    #[arg(long)]
    pub coords: Option<String>,
    #[arg(long)]
    pub snapshot: Option<String>,
}

#[derive(Args, Debug)]
pub struct SetValueArgs {
    #[arg(long)]
    pub on: String,
    #[arg(long)]
    pub value: String,
    #[arg(long)]
    pub snapshot: Option<String>,
}

#[derive(Args, Debug)]
pub struct PerformActionArgs {
    #[arg(long)]
    pub on: String,
    #[arg(long)]
    pub action: String,
    #[arg(long)]
    pub snapshot: Option<String>,
}

#[derive(Args, Debug)]
pub struct WindowArgs {
    #[command(subcommand)]
    pub action: WindowAction,
}

#[derive(Subcommand, Debug)]
pub enum WindowAction {
    List,
    Focus(AppOnly),
    Close(AppOnly),
    Minimize(AppOnly),
    Move(WindowBounds),
    Resize(WindowBounds),
    SetBounds(WindowBounds),
}

#[derive(Args, Debug)]
pub struct AppOnly {
    #[arg(long)]
    pub app: String,
}

#[derive(Args, Debug)]
pub struct WindowBounds {
    #[arg(long)]
    pub app: String,
    #[arg(long, default_value_t = 0)]
    pub x: i64,
    #[arg(long, default_value_t = 0)]
    pub y: i64,
    #[arg(long, default_value_t = 0)]
    pub width: i64,
    #[arg(long, default_value_t = 0)]
    pub height: i64,
}

#[derive(Args, Debug)]
pub struct AppArgs {
    #[command(subcommand)]
    pub action: AppAction,
}

#[derive(Subcommand, Debug)]
pub enum AppAction {
    List,
    Launch(AppOnly),
    Quit(AppOnly),
    Hide(AppOnly),
    Unhide(AppOnly),
    Switch(AppOnly),
}

#[derive(Args, Debug)]
pub struct OpenArgs {
    pub target: String,
    #[arg(long)]
    pub app: Option<String>,
    #[arg(long)]
    pub no_focus: bool,
}

#[derive(Args, Debug)]
pub struct MenuArgs {
    #[command(subcommand)]
    pub action: MenuAction,
}

#[derive(Subcommand, Debug)]
pub enum MenuAction {
    List(MenuList),
    ListAll(MenuList),
    Click(MenuClick),
}

#[derive(Args, Debug)]
pub struct MenuList {
    #[arg(long)]
    pub app: String,
}

#[derive(Args, Debug)]
pub struct MenuClick {
    #[arg(long)]
    pub app: String,
    #[arg(long)]
    pub menu: String,
    #[arg(long)]
    pub item: String,
}

#[derive(Args, Debug)]
pub struct ClipboardArgs {
    #[command(subcommand)]
    pub action: ClipboardAction,
}

#[derive(Subcommand, Debug)]
pub enum ClipboardAction {
    Read,
    Write(ClipboardWrite),
}

#[derive(Args, Debug)]
pub struct ClipboardWrite {
    pub text: String,
}

#[derive(Args, Debug)]
pub struct PermissionsArgs {
    #[command(subcommand)]
    pub action: Option<PermissionAction>,
}

#[derive(Subcommand, Debug)]
pub enum PermissionAction {
    Status,
    Grant,
}

#[derive(Args, Debug)]
pub struct ShellArgs {
    pub command: String,
    #[arg(long)]
    pub cwd: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    pub file: PathBuf,
}

#[derive(Args, Debug)]
pub struct SleepArgs {
    pub duration: f64,
}

#[derive(Args, Debug)]
pub struct CleanArgs {
    #[arg(long)]
    pub all_snapshots: bool,
    #[arg(long)]
    pub snapshot: Option<String>,
}

#[derive(Args, Debug)]
pub struct ToolsArgs;

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

pub fn execute(cli: Cli) -> Result<()> {
    let peekaboo = Peekaboo::new();
    let result = match cli.command {
        Commands::See(args) => CommandResult::ok(peekaboo.see(
            args.app.as_deref(),
            ImageMode::parse(&args.mode),
            args.path,
            args.retina,
        )?)?,
        Commands::Image(args) => CommandResult::ok(peekaboo.image(
            ImageMode::parse(&args.mode),
            args.path,
            args.retina,
        )?)?,
        Commands::List(args) => CommandResult::ok(match args.kind {
            ListKind::Apps => peekaboo.list_apps()?,
            ListKind::Windows => peekaboo.list_windows()?,
            ListKind::Screens => peekaboo.list_screens()?,
            ListKind::Menubar => peekaboo.menu("list", "SystemUIServer", None, None)?,
            ListKind::Permissions => peekaboo.permissions(),
        })?,
        Commands::Click(args) => {
            CommandResult::ok(peekaboo.click(target_from_click(args)?, "left", 1)?)?
        }
        Commands::Type(args) => CommandResult::ok(peekaboo.type_text(
            args.text.as_deref().unwrap_or_default(),
            args.clear,
            args.press_return,
            args.delay,
        )?)?,
        Commands::Press(args) => {
            CommandResult::ok(peekaboo.press(&args.key, args.count, args.delay)?)?
        }
        Commands::Hotkey(args) => {
            let keys = split_keys(&args.keys);
            CommandResult::ok(peekaboo.hotkey(&keys)?)?
        }
        Commands::Paste(args) => CommandResult::ok(peekaboo.paste(&args.text)?)?,
        Commands::Scroll(args) => {
            CommandResult::ok(peekaboo.scroll(Direction::parse(&args.direction), args.amount)?)?
        }
        Commands::Swipe(args) => CommandResult::ok(peekaboo.swipe(
            Target::Point(parse_point(&args.from)?),
            Target::Point(parse_point(&args.to)?),
            args.duration,
        )?)?,
        Commands::Drag(args) => CommandResult::ok(peekaboo.drag(
            Target::Point(parse_point(&args.from)?),
            Target::Point(parse_point(&args.to)?),
            args.duration,
        )?)?,
        Commands::Move(args) => CommandResult::ok(peekaboo.move_cursor(target_from_move(args)?)?)?,
        Commands::SetValue(args) => CommandResult::ok(peekaboo.set_value(
            Target::Query {
                query: args.on,
                snapshot: args.snapshot,
            },
            &args.value,
        )?)?,
        Commands::PerformAction(args) => CommandResult::ok(peekaboo.perform_action(
            Target::Query {
                query: args.on,
                snapshot: args.snapshot,
            },
            &args.action,
        )?)?,
        Commands::Window(args) => CommandResult::ok(window(&peekaboo, args.action)?)?,
        Commands::App(args) => CommandResult::ok(app(&peekaboo, args.action)?)?,
        Commands::Open(args) => {
            CommandResult::ok(peekaboo.open(&args.target, args.app.as_deref(), args.no_focus)?)?
        }
        Commands::Menu(args) => CommandResult::ok(menu(&peekaboo, args.action)?)?,
        Commands::Clipboard(args) => CommandResult::ok(match args.action {
            ClipboardAction::Read => json!({ "text": peekaboo.clipboard_read()? }),
            ClipboardAction::Write(write) => peekaboo.clipboard_write(&write.text)?,
        })?,
        Commands::Permissions(_) => CommandResult::ok(peekaboo.permissions())?,
        Commands::Shell(args) => {
            CommandResult::ok(peekaboo.shell(&args.command, args.cwd.as_deref())?)?
        }
        Commands::Run(args) => CommandResult::ok(peekaboo.run_file(&args.file)?)?,
        Commands::Sleep(args) => {
            let millis = (args.duration * 1000.0).max(0.0) as u64;
            std::thread::sleep(Duration::from_millis(millis));
            CommandResult::ok(json!({ "slept_ms": millis }))?
        }
        Commands::Clean(args) => CommandResult::ok(
            json!({ "removed": cache::clean_snapshots(args.all_snapshots, args.snapshot.as_deref())? }),
        )?,
        Commands::Tools(_) => CommandResult::ok(tool_catalog())?,
        Commands::Completions(args) => return completions(args.shell),
    };
    emit(result, cli.json)
}

fn target_from_click(args: ClickArgs) -> Result<Target> {
    if let Some(coords) = args.coords {
        return Ok(Target::Point(parse_point(&coords)?));
    }
    let query = args
        .on
        .or(args.target)
        .ok_or(crate::PeekabooError::MissingArgument("target"))?;
    Ok(Target::Query {
        query,
        snapshot: args.snapshot,
    })
}

fn target_from_move(args: MoveArgs) -> Result<Target> {
    if let Some(coords) = args.coords {
        return Ok(Target::Point(parse_point(&coords)?));
    }
    let query = args
        .target
        .ok_or(crate::PeekabooError::MissingArgument("target"))?;
    Ok(Target::Query {
        query,
        snapshot: args.snapshot,
    })
}

fn window(peekaboo: &Peekaboo, action: WindowAction) -> Result<Value> {
    match action {
        WindowAction::List => peekaboo.window("list", None, None),
        WindowAction::Focus(args) => peekaboo.window("focus", Some(&args.app), None),
        WindowAction::Close(args) => peekaboo.window("close", Some(&args.app), None),
        WindowAction::Minimize(args) => peekaboo.window("minimize", Some(&args.app), None),
        WindowAction::Move(args) => {
            let app = args.app.clone();
            peekaboo.window("move", Some(&app), Some(bounds(args)))
        }
        WindowAction::Resize(args) => {
            let app = args.app.clone();
            peekaboo.window("resize", Some(&app), Some(bounds(args)))
        }
        WindowAction::SetBounds(args) => {
            let app = args.app.clone();
            peekaboo.window("set-bounds", Some(&app), Some(bounds(args)))
        }
    }
}

fn bounds(args: WindowBounds) -> Bounds {
    Bounds {
        x: args.x,
        y: args.y,
        width: args.width,
        height: args.height,
    }
}

fn app(peekaboo: &Peekaboo, action: AppAction) -> Result<Value> {
    match action {
        AppAction::List => peekaboo.app("list", None),
        AppAction::Launch(args) => peekaboo.app("launch", Some(&args.app)),
        AppAction::Quit(args) => peekaboo.app("quit", Some(&args.app)),
        AppAction::Hide(args) => peekaboo.app("hide", Some(&args.app)),
        AppAction::Unhide(args) => peekaboo.app("unhide", Some(&args.app)),
        AppAction::Switch(args) => peekaboo.app("switch", Some(&args.app)),
    }
}

fn menu(peekaboo: &Peekaboo, action: MenuAction) -> Result<Value> {
    match action {
        MenuAction::List(args) => peekaboo.menu("list", &args.app, None, None),
        MenuAction::ListAll(args) => peekaboo.menu("list-all", &args.app, None, None),
        MenuAction::Click(args) => {
            peekaboo.menu("click", &args.app, Some(&args.menu), Some(&args.item))
        }
    }
}

fn tool_catalog() -> Value {
    json!([
        "see",
        "image",
        "list",
        "click",
        "type",
        "press",
        "hotkey",
        "paste",
        "scroll",
        "swipe",
        "drag",
        "move",
        "set-value",
        "perform-action",
        "window",
        "app",
        "open",
        "menu",
        "clipboard",
        "permissions",
        "shell",
        "run",
        "sleep",
        "clean",
        "tools",
        "completions"
    ])
}

fn completions(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    let mut stdout = io::stdout();
    match shell {
        CompletionShell::Bash => generate(shells::Bash, &mut command, "rs-peekaboo", &mut stdout),
        CompletionShell::Zsh => generate(shells::Zsh, &mut command, "rs-peekaboo", &mut stdout),
        CompletionShell::Fish => generate(shells::Fish, &mut command, "rs-peekaboo", &mut stdout),
    };
    Ok(())
}

fn emit(result: CommandResult, json_output: bool) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_human(&result.data);
    }
    Ok(())
}

fn print_human(value: &Value) {
    match value {
        Value::String(text) => println!("{text}"),
        _ => println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        ),
    }
}

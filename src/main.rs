mod app;
mod config;
mod engine;
mod error;
mod model;
mod preview;
mod sandbox;
mod ui;

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand};
use crossterm::event::{self, Event, KeyEventKind};

use app::{App, AppInit};
use error::{Error, Result};

#[derive(Parser, Debug)]
#[command(name = "custtymize", version, about = "Design your shell prompt in a hacker-console TUI, then run it")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Config to edit (default: $CUSTTYMIZE_CONFIG or ~/.config/custtymize/prompt.toml)
    #[arg(short, long, value_name = "PATH", global = true)]
    config: Option<PathBuf>,
    /// Print the generated TOML to stdout instead of opening the UI
    #[arg(long)]
    dry_run: bool,
    /// Apply a palette preset before printing/opening (dracula, tokyo-night, catppuccin-mocha, gruvbox, phosphor, amber, …)
    #[arg(long, value_name = "NAME")]
    preset: Option<String>,
    /// Apply a segment shape (powerline, rounded, slanted, flat, plain)
    #[arg(long, value_name = "SHAPE")]
    shape: Option<String>,
    /// Render the prompt once in the preview sandbox, print the ANSI and exit
    #[arg(long)]
    print_preview: bool,
    /// Skip the boot sequence
    #[arg(long)]
    no_boot: bool,
    /// Force 256-colour output (auto-detected otherwise)
    #[arg(long, conflicts_with = "truecolor")]
    ansi256: bool,
    /// Force 24-bit colour output
    #[arg(long)]
    truecolor: bool,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the prompt (called by the shell integration on every prompt)
    Prompt(PromptArgs),
    /// Print the shell integration script: eval "$(custtymize init zsh)"
    Init {
        /// zsh, bash or fish
        shell: String,
    },
    /// Show how long each module takes in the current directory
    Timings(PromptArgs),
}

/// Numbers arrive as strings so an empty value from the shell (`--cmd-duration=`)
/// means "unknown" instead of a parse error that would blank the prompt.
#[derive(Args, Debug, Default)]
struct PromptArgs {
    /// Print the right prompt instead of the left
    #[arg(long)]
    right: bool,
    /// Print left, a 0x1e separator, then right (used by the zsh integration)
    #[arg(long)]
    both: bool,
    #[arg(long, default_value = "")]
    status: String,
    #[arg(long, default_value = "")]
    cmd_duration: String,
    #[arg(long, default_value = "")]
    jobs: String,
    #[arg(long, default_value = "")]
    terminal_width: String,
    #[arg(long, default_value = "")]
    keymap: String,
    #[arg(long, default_value = "")]
    shell: String,
}

/// The config to edit and, on first run, a starship config to start from.
fn editor_paths() -> Result<(PathBuf, Option<PathBuf>)> {
    if let Some(p) = std::env::var_os("CUSTTYMIZE_CONFIG") {
        return Ok((PathBuf::from(p), None));
    }
    let own = engine::own_config_path().ok_or(Error::NoHome)?;
    if own.exists() {
        return Ok((own, None));
    }
    let starship = std::env::var_os("STARSHIP_CONFIG")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config/starship.toml")))
        .filter(|p| p.exists());
    Ok((own, starship))
}

/// COLORTERM is the reliable signal. Apple Terminal only sets TERM_PROGRAM, and gained
/// 24-bit colour in macOS 26 (Terminal 460+); older versions need the 256 fallback.
fn detect_truecolor() -> bool {
    if let Ok(ct) = std::env::var("COLORTERM")
        && (ct.contains("truecolor") || ct.contains("24bit")) {
            return true;
        }
    match std::env::var("TERM_PROGRAM").as_deref() {
        Ok("Apple_Terminal") => std::env::var("TERM_PROGRAM_VERSION")
            .ok()
            .and_then(|v| v.split('.').next().and_then(|m| m.parse::<u32>().ok()))
            .is_some_and(|major| major >= 460),
        Ok("iTerm.app" | "WezTerm" | "ghostty" | "vscode" | "WarpTerminal") => true,
        _ => std::env::var("TERM").is_ok_and(|t| t.contains("kitty") || t.contains("alacritty") || t.contains("direct")),
    }
}

fn prompt_context(args: &PromptArgs, doc: &toml_edit::DocumentMut) -> engine::Context {
    let mut ctx = engine::Context::from_shell();
    ctx.status = args.status.parse().unwrap_or(0);
    ctx.duration_ms = args.cmd_duration.parse().ok();
    ctx.jobs = args.jobs.parse().unwrap_or(0);
    ctx.width = args.terminal_width.parse().unwrap_or(80);
    ctx.keymap = args.keymap.clone();
    ctx.timeout = engine::config_timeout(doc);
    ctx
}

fn load_prompt_config(explicit: Option<PathBuf>) -> std::result::Result<toml_edit::DocumentMut, String> {
    match explicit.or_else(engine::config_path) {
        Some(p) => engine::load_doc(&p),
        None => Ok(toml_edit::DocumentMut::new()),
    }
}

/// Runs on every prompt, so it never fails: a broken config still gets a usable prompt
/// and the reason goes to a log file instead of the terminal.
fn prompt(args: PromptArgs, config: Option<PathBuf>) {
    let shell = engine::Shell::parse(&args.shell).unwrap_or(engine::Shell::None);
    let log = sandbox::cache_dir().ok().map(|d| d.join("prompt.log"));
    let doc = match load_prompt_config(config) {
        Ok(d) => d,
        Err(e) => {
            if let Some(l) = &log {
                let _ = std::fs::write(l, format!("{e}\n"));
            }
            let fallback = "custtymize: config error (run `custtymize timings`) ❯ ";
            print!("{fallback}");
            return;
        }
    };
    let ctx = prompt_context(&args, &doc);
    let r = engine::render(&doc, &ctx, shell);
    if let Some(l) = &log
        && !r.warnings.is_empty()
    {
        let _ = std::fs::write(l, r.warnings.join("\n") + "\n");
    }
    let mut out = std::io::stdout().lock();
    let _ = if args.both {
        write!(out, "{}{}{}", r.left, engine::init::SEPARATOR, r.right)
    } else if args.right {
        write!(out, "{}", r.right)
    } else {
        write!(out, "{}", r.left)
    };
}

fn timings(args: PromptArgs, config: Option<PathBuf>) -> Result<()> {
    let doc = load_prompt_config(config).map_err(Error::Msg)?;
    let ctx = prompt_context(&args, &doc);
    let start = Instant::now();
    let r = engine::render(&doc, &ctx, engine::Shell::None);
    let total = start.elapsed();
    let mut t = r.timings;
    t.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("{}", r.left.trim_start_matches('\n'));
    if !r.right.is_empty() {
        println!("right: {}", r.right);
    }
    println!();
    let ms = |d: Duration| format!("{:>8.2}ms", d.as_secs_f64() * 1000.0);
    for (name, d) in t {
        println!("  {} {name}", ms(d));
    }
    println!("  {} total (modules run in parallel)", ms(total));
    for w in r.warnings {
        eprintln!("warning: {w}");
    }
    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Prompt(args)) => {
            prompt(args, cli.config);
            return Ok(());
        }
        Some(Cmd::Timings(args)) => return timings(args, cli.config),
        Some(Cmd::Init { shell }) => {
            let sh = engine::Shell::parse(&shell)
                .filter(|s| *s != engine::Shell::None)
                .ok_or_else(|| Error::Msg(format!("unsupported shell {shell:?} (zsh, bash or fish)")))?;
            let exe = std::env::current_exe().map_err(Error::Terminal)?;
            print!("{}", engine::init::script(sh, &exe.to_string_lossy()).unwrap_or_default());
            return Ok(());
        }
        None => {}
    }

    let (path, import) = match cli.config {
        Some(p) => (p, None),
        None => editor_paths()?,
    };
    let mut loaded = config::load::load(import.as_deref().unwrap_or(&path))?;
    let imported = match &import {
        Some(src) => {
            loaded.existed = false;
            loaded.notes.push(format!(
                "imported {} — saving writes {}; the starship file is left alone",
                src.display(),
                path.display()
            ));
            std::fs::read_to_string(src).ok()
        }
        None => None,
    };
    let base_model = loaded.model.clone();
    if let Some(s) = &cli.shape {
        let shape = model::Shape::parse(s).ok_or_else(|| Error::Msg(format!("unknown shape {s:?}")))?;
        loaded.model.set_shape(shape);
        for seg in &mut loaded.model.segments {
            seg.format_override = None;
        }
    }
    if let Some(p) = &cli.preset {
        let pal = model::palette::find(p).ok_or_else(|| Error::Msg(format!("unknown preset {p:?}")))?;
        loaded.model.apply_palette(pal);
    }

    if cli.dry_run {
        print!("{}", config::generate::render(&loaded.model, &loaded.doc, Some(&base_model)));
        return Ok(());
    }

    let (sandbox, sandbox_error) = match sandbox::ensure() {
        Ok(sb) => (Some(sb), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let env = preview::Env { sandbox };

    if cli.print_preview {
        let toml = config::generate::render(&loaded.model, &loaded.doc, Some(&base_model));
        let out = preview::render_sync(&env, &toml, sandbox::Scenario::DirtyRepo, 100);
        println!("{}", out.success.left.trim_start_matches('\n'));
        println!("{}", out.failure.left.trim_start_matches('\n'));
        if !out.success.right.is_empty() {
            println!("right: {}", out.success.right);
        }
        for w in out.warnings {
            eprintln!("{w}");
        }
        if let Some(e) = out.error {
            return Err(Error::Rejected(e));
        }
        return Ok(());
    }

    let truecolor = if cli.ansi256 { false } else { cli.truecolor || detect_truecolor() };
    // Edits made via --preset/--shape count as unsaved changes on top of the loaded file.
    let edited = loaded.model.clone();
    loaded.model = base_model;
    let mut app = App::new(AppInit {
        config_path: path,
        loaded,
        env: Some(env),
        imported,
        sandbox_error,
        truecolor,
        skip_boot: cli.no_boot,
    });
    if edited != app.model {
        app.replace_model(edited, "applied command-line --preset/--shape");
    }

    let mut terminal = ratatui::try_init()?;
    let res = event_loop(&mut terminal, &mut app);
    ratatui::try_restore()?;
    res
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        app.tick();
        terminal.draw(|f| ui::draw(f, app))?;
        if app.quit {
            return Ok(());
        }
        // ~30 fps for the blinking cursor, typewriter and spinner.
        if event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => app.on_key(k),
                Event::Resize(..) => app.request_preview(),
                _ => {}
            }
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("custtymize: {e}");
        std::process::exit(1);
    }
}

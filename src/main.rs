mod app;
mod config;
mod error;
mod model;
mod preview;
mod sandbox;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};

use app::{App, AppInit};
use error::{Error, Result};

#[derive(Parser, Debug)]
#[command(name = "custtymize", version, about = "A hacker-console TUI for designing your Starship prompt")]
struct Cli {
    /// Config to edit (default: $STARSHIP_CONFIG or ~/.config/starship.toml)
    #[arg(short, long, value_name = "PATH")]
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
    /// Render the prompt once with the generated config, print the ANSI and exit
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

fn default_config_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("STARSHIP_CONFIG") {
        return Ok(PathBuf::from(p));
    }
    Ok(dirs::home_dir().ok_or(Error::NoHome)?.join(".config").join("starship.toml"))
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

fn run() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.config {
        Some(p) => p,
        None => default_config_path()?,
    };
    let mut loaded = config::load::load(&path)?;
    let base_model = loaded.model.clone();
    if let Some(s) = &cli.shape {
        let shape = model::Shape::parse(s).ok_or_else(|| Error::Starship(format!("unknown shape {s:?}")))?;
        loaded.model.set_shape(shape);
        for seg in &mut loaded.model.segments {
            seg.format_override = None;
        }
    }
    if let Some(p) = &cli.preset {
        let pal = model::palette::find(p).ok_or_else(|| Error::Starship(format!("unknown preset {p:?}")))?;
        loaded.model.apply_palette(pal);
    }

    if cli.dry_run {
        print!("{}", config::generate::render(&loaded.model, &loaded.doc, Some(&base_model)));
        return Ok(());
    }

    let starship = preview::find_starship();
    let starship_version = starship.as_deref().and_then(preview::starship_version);
    let (sandbox, sandbox_error) = match sandbox::ensure() {
        Ok(sb) => (Some(sb), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let env = match starship {
        Some(bin) => Some(preview::Env { starship: bin, sandbox, work_dir: sandbox::cache_dir()? }),
        None => None,
    };

    if cli.print_preview {
        let env = env.ok_or_else(|| Error::StarshipMissing("not on PATH".into()))?;
        let toml = config::generate::render(&loaded.model, &loaded.doc, Some(&base_model));
        let runner = preview::Runner::spawn(env);
        runner.request(preview::Request { id: 1, toml, scenario: sandbox::Scenario::DirtyRepo, width: 100 });
        let out = runner.rx.recv().map_err(|e| Error::Starship(e.to_string()))?;
        println!("{}", out.success.left.trim_start_matches('\n'));
        println!("{}", out.failure.left.trim_start_matches('\n'));
        if !out.success.right.is_empty() {
            println!("right: {}", out.success.right);
        }
        for w in out.warnings {
            eprintln!("{w}");
        }
        if let Some(e) = out.error {
            return Err(Error::Starship(e));
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
        env,
        starship_version,
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

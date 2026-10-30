//! Everything that isn't a path, git or a language: user, host, clock, battery, …

use std::path::PathBuf;

use super::super::format::Vars;
use super::super::style::Seg;
use super::super::{Cfg, Context, util};
use super::{detected, finish, sub_format};

pub fn render(ctx: &Context, name: &str, cfg: Cfg) -> Option<Vec<Seg>> {
    match name {
        "character" => character(ctx, cfg),
        "cmd_duration" => cmd_duration(ctx, cfg),
        "time" => time(ctx, cfg),
        "username" => username(ctx, cfg),
        "hostname" => hostname(ctx, cfg),
        "os" => os(ctx, cfg),
        "jobs" => jobs(ctx, cfg),
        "status" => status(ctx, cfg),
        "sudo" => sudo(ctx, cfg),
        "battery" => battery(ctx, cfg),
        "memory_usage" => memory(ctx, cfg),
        "docker_context" => docker(ctx, cfg),
        "kubernetes" => kubernetes(ctx, cfg),
        "aws" => aws(ctx, cfg),
        _ => {
            ctx.warn(format!("module `{name}` isn't supported by the custtymize engine"));
            None
        }
    }
}

fn character(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let vicmd = matches!(ctx.keymap.as_str(), "vicmd");
    let src = if vicmd {
        cfg.str("vimcmd_symbol", "[❮](bold green)")
    } else if ctx.status == 0 {
        cfg.str("success_symbol", "[❯](bold green)")
    } else {
        cfg.str("error_symbol", "[❯](bold red)")
    };
    let mut v = Vars::new();
    v.segs("symbol", sub_format(ctx, "character", &src, &Vars::new()));
    finish(ctx, "character", cfg, "$symbol ", "", "", &mut v)
}

fn cmd_duration(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let ms = ctx.duration_ms?;
    if (ms as i64) < cfg.int("min_time", 2000) {
        return None;
    }
    let mut v = Vars::new();
    v.text("duration", util::humanize_ms(ms, cfg.bool("show_milliseconds", false)));
    finish(ctx, "cmd_duration", cfg, "took [$duration]($style) ", "bold yellow", "", &mut v)
}

fn time(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let default_fmt = if cfg.bool("use_12hr", false) { "%r" } else { "%T" };
    let fmt = cfg.str("time_format", default_fmt);
    let now = chrono::Local::now();
    let text = match cfg.opt_str("utc_time_offset").filter(|o| *o != "local").and_then(|o| o.parse::<f64>().ok()) {
        Some(hours) => {
            let off = chrono::FixedOffset::east_opt((hours * 3600.0) as i32)?;
            now.with_timezone(&off).format(&fmt).to_string()
        }
        None => now.format(&fmt).to_string(),
    };
    let mut v = Vars::new();
    v.text("time", text);
    finish(ctx, "time", cfg, "at [$time]($style) ", "bold yellow", "", &mut v)
}

fn is_ssh(ctx: &Context) -> bool {
    ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"].iter().any(|k| ctx.var(k).is_some())
}

fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

fn username(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let user = ctx.var("USER").or_else(|| ctx.var("LOGNAME"))?;
    let root = is_root();
    if !(root || is_ssh(ctx) || cfg.bool("show_always", false)) {
        return None;
    }
    let mut v = Vars::new();
    v.text("user", user);
    v.text("style", if root { cfg.str("style_root", "bold red") } else { cfg.str("style_user", "bold yellow") });
    finish(ctx, "username", cfg, "[$user]($style) in ", "", "", &mut v)
}

fn host_name() -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its length; gethostname NUL-terminates on success.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..end]).into_owned())
}

fn hostname(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let ssh = is_ssh(ctx);
    if cfg.bool("ssh_only", true) && !ssh {
        return None;
    }
    let mut host = host_name()?;
    let trim_at = cfg.str("trim_at", ".");
    if !trim_at.is_empty()
        && let Some(i) = host.find(&trim_at)
    {
        host.truncate(i);
    }
    let mut v = Vars::new();
    v.text("hostname", host);
    v.text("ssh_symbol", if ssh { cfg.str("ssh_symbol", "🌐 ") } else { String::new() });
    finish(ctx, "hostname", cfg, "[$ssh_symbol$hostname]($style) in ", "bold dimmed green", "", &mut v)
}

fn os(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let (kind, name) = if cfg!(target_os = "macos") {
        ("Macos".to_string(), "macOS".to_string())
    } else {
        let rel = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let field = |k: &str| {
            rel.lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .map(|v| v.trim_matches('"').to_string())
        };
        let id = field("ID").unwrap_or_else(|| "linux".into());
        let mut kind = id.clone();
        if let Some(f) = kind.get_mut(..1) {
            f.make_ascii_uppercase();
        }
        (kind, field("NAME").unwrap_or(id))
    };
    let defaults = [
        ("Macos", "🍎 "), ("Linux", "🐧 "), ("Ubuntu", "🎯 "), ("Debian", "🌀 "), ("Arch", "🎗️ "),
        ("Fedora", "🎩 "), ("Alpine", "🏔️ "), ("Nixos", "❄️ "), ("Unknown", "❓ "),
    ];
    let symbols = cfg.table("symbols");
    let symbol = symbols
        .opt_str(&kind)
        .map(String::from)
        .or_else(|| defaults.iter().find(|(k, _)| *k == kind).map(|(_, s)| s.to_string()))
        .unwrap_or_else(|| "🐧 ".into());
    let mut v = Vars::new();
    v.text("symbol", symbol).text("name", name).text("type", kind).text("version", "").text("codename", "").text("edition", "");
    finish(ctx, "os", cfg, "[$symbol]($style)", "bold white", "", &mut v)
}

fn jobs(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let n = ctx.jobs as i64;
    if n < cfg.int("threshold", 1).max(1) {
        return None;
    }
    let mut v = Vars::new();
    let symbol = if n >= cfg.int("symbol_threshold", 1) { cfg.str("symbol", "✦") } else { String::new() };
    v.text("symbol", symbol);
    v.text("number", if n >= cfg.int("number_threshold", 2) { n.to_string() } else { String::new() });
    finish(ctx, "jobs", cfg, "[$symbol$number]($style) ", "bold blue", "", &mut v)
}

fn status(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let success_symbol = cfg.str("success_symbol", "");
    let code = ctx.status;
    if code == 0 && success_symbol.is_empty() {
        return None;
    }
    let signal = (code > 128 && cfg.bool("recognize_signal_code", true)).then_some(code - 128);
    let symbol = if code == 0 {
        success_symbol
    } else if !cfg.bool("map_symbol", false) {
        cfg.str("symbol", "❌")
    } else {
        match (code, signal) {
            (126, _) => cfg.str("not_executable_symbol", "🚫"),
            (127, _) => cfg.str("not_found_symbol", "🔍"),
            (_, Some(2)) => cfg.str("sigint_symbol", "🧱"),
            (_, Some(_)) => cfg.str("signal_symbol", "⚡"),
            _ => cfg.str("symbol", "❌"),
        }
    };
    let signal_name = match signal {
        Some(1) => "HUP",
        Some(2) => "INT",
        Some(3) => "QUIT",
        Some(4) => "ILL",
        Some(6) => "ABRT",
        Some(8) => "FPE",
        Some(9) => "KILL",
        Some(11) => "SEGV",
        Some(13) => "PIPE",
        Some(14) => "ALRM",
        Some(15) => "TERM",
        _ => "",
    };
    let mut v = Vars::new();
    v.text("symbol", symbol).text("status", code.to_string()).text("int", code.to_string());
    v.text("hex_status", format!("0x{code:X}"));
    v.text("common_meaning", match code {
        1 => "ERROR",
        2 => "USAGE",
        126 => "NOPERM",
        127 => "NOTFOUND",
        _ => "",
    });
    v.text("signal_number", signal.map(|n| n.to_string()).unwrap_or_default()).text("signal_name", signal_name);
    finish(ctx, "status", cfg, "[$symbol$status]($style) ", "bold red", "❌", &mut v)
}

fn sudo(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let bin = util::which(ctx, "sudo")?;
    if !util::run(ctx, &bin, &["-n", "true"])?.ok {
        return None;
    }
    finish(ctx, "sudo", cfg, "[as $symbol]($style)", "bold blue", "🧙 ", &mut Vars::new())
}

/// `(percent, state)` where state is `charging`, `discharging`, `full` or `unknown`.
fn battery_state(ctx: &Context) -> Option<(f64, &'static str)> {
    if cfg!(target_os = "macos") {
        let out = util::run(ctx, &PathBuf::from("/usr/bin/pmset"), &["-g", "batt"])?;
        let line = out.stdout.lines().find(|l| l.contains('%'))?;
        let pct = line.split('%').next()?.rsplit(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
        let state = if line.contains("discharging") {
            "discharging"
        } else if line.contains("charging") {
            "charging"
        } else if line.contains("charged") {
            "full"
        } else {
            "unknown"
        };
        Some((pct, state))
    } else {
        let dir = std::fs::read_dir("/sys/class/power_supply").ok()?.flatten().map(|e| e.path()).find(|p| {
            p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("BAT"))
        })?;
        let pct = std::fs::read_to_string(dir.join("capacity")).ok()?.trim().parse().ok()?;
        let st = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
        let state = match st.trim() {
            "Charging" => "charging",
            "Discharging" => "discharging",
            "Full" => "full",
            _ => "unknown",
        };
        Some((pct, state))
    }
}

fn battery(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let (pct, state) = battery_state(ctx)?;
    // `display` is a list of `{ threshold, style, charging_symbol, discharging_symbol }`.
    let mut displays: Vec<(i64, String, Option<String>, Option<String>)> = Vec::new();
    if let Some(arr) = cfg.get("display").and_then(|i| i.as_array()) {
        for d in arr.iter().filter_map(|v| v.as_inline_table()) {
            displays.push((
                d.get("threshold").and_then(|v| v.as_integer()).unwrap_or(10),
                d.get("style").and_then(|v| v.as_str()).unwrap_or("red bold").to_string(),
                d.get("charging_symbol").and_then(|v| v.as_str()).map(String::from),
                d.get("discharging_symbol").and_then(|v| v.as_str()).map(String::from),
            ));
        }
    } else if let Some(arr) = cfg.get("display").and_then(|i| i.as_array_of_tables()) {
        for d in arr.iter() {
            displays.push((
                d.get("threshold").and_then(|v| v.as_integer()).unwrap_or(10),
                d.get("style").and_then(|v| v.as_str()).unwrap_or("red bold").to_string(),
                d.get("charging_symbol").and_then(|v| v.as_str()).map(String::from),
                d.get("discharging_symbol").and_then(|v| v.as_str()).map(String::from),
            ));
        }
    } else {
        displays.push((10, "red bold".into(), None, None));
    }
    displays.sort_by_key(|d| d.0);
    let (_, style, chg, dis) = displays.into_iter().find(|d| pct <= d.0 as f64)?;
    let symbol = match state {
        "charging" => chg.unwrap_or_else(|| cfg.str("charging_symbol", "\u{f0084} ")),
        "discharging" => dis.unwrap_or_else(|| cfg.str("discharging_symbol", "\u{f0083} ")),
        "full" => cfg.str("full_symbol", "\u{f0079} "),
        _ => cfg.str("unknown_symbol", "\u{f007d} "),
    };
    let mut v = Vars::new();
    v.text("symbol", symbol).text("percentage", format!("{pct:.0}%")).text("style", style);
    finish(ctx, "battery", cfg, "[$symbol$percentage]($style) ", "", "", &mut v)
}

/// `(used, total)` bytes of RAM.
fn memory_bytes(ctx: &Context) -> Option<(u64, u64)> {
    if cfg!(target_os = "macos") {
        let total: u64 = util::run(ctx, &PathBuf::from("/usr/sbin/sysctl"), &["-n", "hw.memsize"])?.stdout.trim().parse().ok()?;
        let vm = util::run(ctx, &PathBuf::from("/usr/bin/vm_stat"), &[])?.stdout;
        let page: u64 = vm.split("page size of ").nth(1)?.split_whitespace().next()?.parse().ok()?;
        let pages = |label: &str| -> u64 {
            vm.lines()
                .find(|l| l.starts_with(label))
                .and_then(|l| l.rsplit(':').next())
                .and_then(|n| n.trim().trim_end_matches('.').parse().ok())
                .unwrap_or(0)
        };
        let used = (pages("Pages active") + pages("Pages wired down") + pages("Pages occupied by compressor")) * page;
        Some((used, total))
    } else {
        let info = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb = |k: &str| -> Option<u64> {
            info.lines().find(|l| l.starts_with(k))?.split_whitespace().nth(1)?.parse().ok()
        };
        let total = kb("MemTotal:")? * 1024;
        Some((total - kb("MemAvailable:")? * 1024, total))
    }
}

fn gib(b: u64) -> String {
    let g = b as f64 / (1u64 << 30) as f64;
    if g >= 10.0 { format!("{g:.0}GiB") } else { format!("{g:.1}GiB") }
}

fn memory(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let (used, total) = memory_bytes(ctx)?;
    let pct = (used as f64 / total.max(1) as f64 * 100.0).round() as i64;
    if pct < cfg.int("threshold", 75) {
        return None;
    }
    let mut v = Vars::new();
    v.text("ram", format!("{}/{} ({pct}%)", gib(used), gib(total))).text("ram_pct", format!("{pct}%"));
    v.text("swap", "").text("swap_pct", "");
    finish(ctx, "memory_usage", cfg, "via $symbol[${ram}( | ${swap})]($style) ", "white bold dimmed", "🐏 ", &mut v)
}

fn docker(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let files = ["docker-compose.yml", "docker-compose.yaml", "Dockerfile", "compose.yml", "compose.yaml"];
    if cfg.bool("only_with_files", true) && !detected(ctx, cfg, &files, &[], &[]) {
        return None;
    }
    let context = ctx.var("DOCKER_MACHINE_NAME").or_else(|| ctx.var("DOCKER_HOST")).or_else(|| ctx.var("DOCKER_CONTEXT")).or_else(|| {
        let dir = ctx.var("DOCKER_CONFIG").map(PathBuf::from).or_else(|| ctx.home_dir().map(|h| h.join(".docker")))?;
        let text = std::fs::read_to_string(dir.join("config.json")).ok()?;
        let after = text.split("\"currentContext\"").nth(1)?;
        Some(after.split('"').nth(1)?.to_string())
    })?;
    if context == "default" || context.starts_with("unix://") {
        return None;
    }
    let mut v = Vars::new();
    v.text("context", context);
    finish(ctx, "docker_context", cfg, "via [$symbol$context]($style) ", "blue bold", "🐳 ", &mut v)
}

fn kubernetes(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let file = match ctx.var("KUBECONFIG") {
        Some(k) => PathBuf::from(k.split(':').next()?),
        None => ctx.home_dir()?.join(".kube/config"),
    };
    let text = std::fs::read_to_string(file).ok()?;
    let context = text.lines().find_map(|l| l.strip_prefix("current-context:"))?.trim().trim_matches('"').to_string();
    if context.is_empty() {
        return None;
    }
    let mut v = Vars::new();
    v.text("context", context).text("namespace", "").text("cluster", "").text("user", "");
    finish(ctx, "kubernetes", cfg, "[$symbol$context( \\($namespace\\))]($style) in ", "cyan bold", "☸ ", &mut v)
}

fn aws(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let profile = ["AWS_VAULT", "AWSU_PROFILE", "AWS_SSO_PROFILE", "AWS_PROFILE", "AWS_DEFAULT_PROFILE"].iter().find_map(|k| ctx.var(k));
    let region = ctx.var("AWS_REGION").or_else(|| ctx.var("AWS_DEFAULT_REGION"));
    if profile.is_none() && region.is_none() {
        return None;
    }
    let mut v = Vars::new();
    v.text("profile", profile.unwrap_or_default()).text("region", region.unwrap_or_default()).text("duration", "");
    finish(ctx, "aws", cfg, "on [$symbol($profile )(\\($region\\) )(\\[$duration\\] )]($style)", "bold yellow", "☁️  ", &mut v)
}

/// `[custom.<name>]`: shown when a detect rule matches or `when` succeeds; prints the
/// output of `command`.
pub fn custom(ctx: &Context, name: &str, cfg: Cfg) -> Option<Vec<Seg>> {
    let label = format!("custom.{name}");
    if let Some(os) = cfg.opt_str("os")
        && os != std::env::consts::OS
    {
        return None;
    }
    if cfg.bool("require_repo", false) && super::git::repo(ctx).is_none() {
        return None;
    }
    let has_detect = ["detect_files", "detect_extensions", "detect_folders"].iter().any(|k| cfg.get(k).is_some());
    let sh = util::which(ctx, "sh")?;
    let shown = (has_detect && detected(ctx, cfg, &[], &[], &[]))
        || match cfg.get("when") {
            Some(w) if w.as_bool() == Some(true) => true,
            Some(w) => w.as_str().is_some_and(|cmd| util::run(ctx, &sh, &["-c", cmd]).is_some_and(|o| o.ok)),
            None => false,
        };
    if !shown {
        return None;
    }
    let output = match cfg.opt_str("command") {
        Some(cmd) => util::run(ctx, &sh, &["-c", cmd]).map(|o| o.stdout.trim().to_string()).unwrap_or_default(),
        None => String::new(),
    };
    let mut v = Vars::new();
    v.text("output", output);
    finish(ctx, &label, cfg, "[$symbol($output )]($style)", "bold green", "", &mut v)
}

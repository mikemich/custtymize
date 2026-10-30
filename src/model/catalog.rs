//! Static knowledge about the Starship modules custtymize can style.

/// How a module's symbol is expressed in the generated config.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolMode {
    /// The module has a `symbol` key; we write it and the body uses `$symbol`.
    Native,
    /// No `symbol` key: the glyph is inlined as literal text where the body says `$symbol`.
    Inline,
    /// Not editable here (the module picks its own symbol, e.g. `os`, `battery`).
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptKind {
    Bool,
    Int,
    Str,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptDef {
    pub key: &'static str,
    pub kind: OptKind,
    /// Starship's own default, shown dimmed when the option is unset.
    pub default: &'static str,
    pub help: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleDef {
    pub name: &'static str,
    /// The variables that make up the visible text, without padding or styling.
    pub body: &'static str,
    pub symbol_mode: SymbolMode,
    pub default_symbol: &'static str,
    /// Starship ships this module with `disabled = true`.
    pub off_by_default: bool,
    /// Shown in the module list on first run.
    pub starter: bool,
    pub right_by_default: bool,
    pub options: &'static [OptDef],
    pub blurb: &'static str,
}

const fn opt(key: &'static str, kind: OptKind, default: &'static str, help: &'static str) -> OptDef {
    OptDef { key, kind, default, help }
}

macro_rules! module {
    ($name:literal, $body:literal, $mode:ident, $sym:literal, off=$off:literal, starter=$st:literal, right=$r:literal, $blurb:literal, [$($o:expr),* $(,)?]) => {
        ModuleDef {
            name: $name,
            body: $body,
            symbol_mode: SymbolMode::$mode,
            default_symbol: $sym,
            off_by_default: $off,
            starter: $st,
            right_by_default: $r,
            options: &[$($o),*],
            blurb: $blurb,
        }
    };
}

use OptKind::*;

pub static CATALOG: &[ModuleDef] = &[
    module!("os", "$symbol", None, "", off=true, starter=true, right=false,
        "Operating system glyph", []),
    module!("username", "$symbol$user", Inline, "", off=false, starter=true, right=false,
        "Current user (hidden locally unless show_always)", [
            opt("show_always", Bool, "false", "show even when not root / ssh"),
        ]),
    module!("hostname", "$ssh_symbol$hostname", None, "", off=false, starter=true, right=false,
        "Machine name (ssh sessions by default)", [
            opt("ssh_only", Bool, "true", "only show over ssh"),
            opt("trim_at", Str, ".", "cut hostname at this string"),
        ]),
    module!("directory", "$path$read_only", None, "", off=false, starter=true, right=false,
        "Current working directory", [
            opt("truncation_length", Int, "3", "parent folders to keep"),
            opt("truncation_symbol", Str, "", "prefix for truncated paths"),
            opt("truncate_to_repo", Bool, "true", "truncate to the git root"),
            opt("read_only", Str, "🔒", "read-only marker"),
            opt("home_symbol", Str, "~", "symbol for $HOME"),
        ]),
    module!("git_branch", "$symbol $branch", Native, "\u{e0a0}", off=false, starter=true, right=false,
        "Active git branch", [
            opt("truncation_length", Int, "9223372036854775807", "max branch name graphemes"),
            opt("truncation_symbol", Str, "…", "suffix for truncated names"),
            opt("only_attached", Bool, "false", "hide when HEAD is detached"),
        ]),
    module!("git_status", "$all_status$ahead_behind", None, "", off=false, starter=true, right=false,
        "Working tree state", [
            opt("ahead", Str, "⇡", "ahead of upstream"),
            opt("behind", Str, "⇣", "behind upstream"),
            opt("modified", Str, "!", "modified files"),
            opt("untracked", Str, "?", "untracked files"),
            opt("staged", Str, "+", "staged changes"),
            opt("stashed", Str, "$", "stash present"),
        ]),
    module!("nodejs", "$symbol($version)", Native, "\u{e718} ", off=false, starter=true, right=false,
        "Node.js version", [
            opt("version_format", Str, "v${raw}", "version formatting"),
        ]),
    module!("rust", "$symbol($version)", Native, "\u{e7a8} ", off=false, starter=true, right=false,
        "Rust toolchain version", [
            opt("version_format", Str, "v${raw}", "version formatting"),
        ]),
    module!("python", "$symbol$pyenv_prefix($version)( \\($virtualenv\\))", Native, "\u{e235} ", off=false, starter=true, right=false,
        "Python version and virtualenv", [
            opt("version_format", Str, "v${raw}", "version formatting"),
            opt("pyenv_version_name", Bool, "false", "use pyenv's name"),
        ]),
    module!("golang", "$symbol($version)", Native, "\u{e627} ", off=false, starter=true, right=false,
        "Go version", [
            opt("version_format", Str, "v${raw}", "version formatting"),
        ]),
    module!("docker_context", "$symbol$context", Native, "\u{f308} ", off=false, starter=true, right=false,
        "Active docker context", [
            opt("only_with_files", Bool, "true", "only in docker projects"),
        ]),
    module!("cmd_duration", "$symbol$duration", Inline, "\u{f252} ", off=false, starter=true, right=true,
        "Duration of the last command", [
            opt("min_time", Int, "2000", "ms before it shows up"),
            opt("show_milliseconds", Bool, "false", "include milliseconds"),
        ]),
    module!("time", "$symbol$time", Inline, "\u{f017} ", off=true, starter=true, right=true,
        "Wall clock", [
            opt("time_format", Str, "%R", "strftime format"),
            opt("use_12hr", Bool, "false", "12 hour clock"),
        ]),
    module!("battery", "$symbol$percentage", None, "", off=false, starter=true, right=true,
        "Battery level (below threshold)", [
            opt("full_symbol", Str, "\u{f0079} ", "symbol when full"),
            opt("charging_symbol", Str, "\u{f0084} ", "symbol when charging"),
            opt("discharging_symbol", Str, "\u{f0083} ", "symbol when discharging"),
        ]),
    module!("character", "", None, "❯", off=false, starter=true, right=false,
        "The prompt character itself (always last)", []),
    // Extras, not shown until added with `:add`.
    module!("package", "$symbol$version", Native, "\u{f487} ", off=false, starter=false, right=false,
        "Package version (Cargo.toml, package.json, …)", []),
    module!("java", "$symbol($version)", Native, "\u{e256} ", off=false, starter=false, right=false,
        "Java version", []),
    module!("ruby", "$symbol($version)", Native, "\u{e791} ", off=false, starter=false, right=false,
        "Ruby version", []),
    module!("lua", "$symbol($version)", Native, "\u{e620} ", off=false, starter=false, right=false,
        "Lua version", []),
    module!("zig", "$symbol($version)", Native, "\u{e6a9} ", off=false, starter=false, right=false,
        "Zig version", []),
    module!("kubernetes", "$symbol$context( \\($namespace\\))", Native, "\u{2638} ", off=true, starter=false, right=true,
        "Kubernetes context", []),
    module!("aws", "$symbol($profile)( \\($region\\))", Native, "\u{e33d} ", off=false, starter=false, right=true,
        "AWS profile and region", []),
    module!("memory_usage", "$symbol${ram}", Native, "\u{f035b} ", off=true, starter=false, right=true,
        "System memory usage", [
            opt("threshold", Int, "75", "percent before it shows"),
        ]),
    module!("jobs", "$symbol$number", Native, "✦", off=false, starter=false, right=false,
        "Background jobs", []),
    module!("status", "$symbol$status", Native, "✘ ", off=true, starter=false, right=true,
        "Exit status of the last command", []),
    module!("sudo", "$symbol", Native, "\u{f023} ", off=true, starter=false, right=false,
        "Cached sudo credentials", []),
];

impl ModuleDef {
    /// The key holding this module's style. `None` means the style is inlined in `format`.
    pub fn style_key(&self) -> Option<&'static str> {
        match self.name {
            "username" => Some("style_user"),
            "battery" | "character" => None,
            _ => Some("style"),
        }
    }
}

/// Symbols for terminals without a Nerd Font: `(unicode, ascii)`. Spacing matches the
/// Nerd Font defaults above (`git_branch` has none; its body adds the space).
pub fn plain_symbols(name: &str) -> (&'static str, &'static str) {
    match name {
        "git_branch" => ("⎇", "git"),
        "nodejs" => ("⬢ ", "node "),
        "rust" => ("🦀 ", "rs "),
        "python" => ("🐍 ", "py "),
        "golang" => ("🐹 ", "go "),
        "docker_context" => ("🐳 ", "docker "),
        "cmd_duration" => ("⏱ ", "took "),
        "time" => ("◷ ", ""),
        "package" => ("📦 ", "pkg "),
        "java" => ("☕ ", "java "),
        "ruby" => ("💎 ", "rb "),
        "lua" => ("🌙 ", "lua "),
        "zig" => ("↯ ", "zig "),
        "kubernetes" => ("☸ ", "k8s "),
        "aws" => ("☁ ", "aws "),
        "memory_usage" => ("▤ ", "mem "),
        "jobs" => ("✦", "*"),
        "status" => ("✘ ", "x "),
        "sudo" => ("⚡ ", "sudo "),
        _ => ("", ""),
    }
}

pub fn lookup(name: &str) -> Option<&'static ModuleDef> {
    CATALOG.iter().find(|m| m.name == name)
}

/// Definition used for segments added with `:add <name>` that we know nothing about.
pub static GENERIC: ModuleDef = ModuleDef {
    name: "generic",
    body: "$symbol($version)",
    symbol_mode: SymbolMode::None,
    default_symbol: "",
    off_by_default: false,
    starter: false,
    right_by_default: false,
    options: &[],
    blurb: "Module outside the catalog (generic $symbol($version) body)",
};

/// A curated set of Nerd Font glyphs for the glyph picker.
pub static GLYPHS: &[(&str, &str)] = &[
    ("\u{e0a0}", "branch"), ("\u{f418}", "branch-alt"), ("\u{e725}", "git-branch"), ("\u{f1d3}", "git"),
    ("\u{e7a8}", "rust"), ("\u{e718}", "node"), ("\u{e235}", "python"), ("\u{e627}", "go"),
    ("\u{f308}", "docker"), ("\u{e256}", "java"), ("\u{e791}", "ruby"), ("\u{e620}", "lua"),
    ("\u{f017}", "clock"), ("\u{f252}", "hourglass"), ("\u{f0e7}", "bolt"), ("\u{f023}", "lock"),
    ("\u{f007}", "user"), ("\u{f108}", "desktop"), ("\u{f179}", "apple"), ("\u{f17c}", "linux"),
    ("\u{f07b}", "folder"), ("\u{f07c}", "folder-open"), ("\u{f015}", "home"), ("\u{f121}", "code"),
    ("\u{f120}", "terminal"), ("\u{f135}", "rocket"), ("\u{f0ac}", "globe"), ("\u{f0c2}", "cloud"),
    ("\u{f2db}", "chip"), ("\u{f013}", "gear"), ("\u{f188}", "bug"), ("\u{f06d}", "fire"),
    ("❯", "chevron"), ("➜", "arrow"), ("λ", "lambda"), ("⚡", "zap"),
    ("▶", "play"), ("»", "guillemet"), ("$", "dollar"), ("#", "hash"),
    ("◆", "diamond"), ("●", "dot"), ("✦", "star"), ("☢", "radioactive"),
    ("⌘", "command"), ("⎇", "alt-branch"), ("⏻", "power"), ("☠", "skull"),
];

//! Language version modules and `package`. Versions come from `<tool> --version`,
//! cached between prompts.

use super::super::format::Vars;
use super::super::style::Seg;
use super::super::{Cfg, Context, util};
use super::{detected, finish, listing};

pub struct Spec {
    pub name: &'static str,
    symbol: &'static str,
    style: &'static str,
    files: &'static [&'static str],
    exts: &'static [&'static str],
    folders: &'static [&'static str],
    /// Binaries tried in order, and their version arguments.
    bins: &'static [&'static str],
    args: &'static [&'static str],
    /// Files that pin a version (they invalidate the cache when they change).
    pins: &'static [&'static str],
    env_pins: &'static [&'static str],
}

const FORMAT: &str = "via [$symbol($version )]($style)";

pub static SPECS: &[Spec] = &[
    Spec {
        name: "nodejs",
        symbol: "\u{e718} ",
        style: "bold green",
        files: &["package.json", ".node-version", ".nvmrc"],
        exts: &["js", "mjs", "cjs", "ts", "mts", "cts"],
        folders: &["node_modules"],
        bins: &["node"],
        args: &["--version"],
        pins: &[".nvmrc", ".node-version", ".tool-versions", "mise.toml"],
        env_pins: &["NODE_VERSION"],
    },
    Spec {
        name: "python",
        symbol: "🐍 ",
        style: "yellow bold",
        files: &["requirements.txt", ".python-version", "pyproject.toml", "Pipfile", "tox.ini", "setup.py", "__init__.py"],
        exts: &["py", "ipynb"],
        folders: &[],
        bins: &["python", "python3", "python2"],
        args: &["--version"],
        pins: &[".python-version", ".tool-versions", "mise.toml"],
        env_pins: &["PYENV_VERSION", "VIRTUAL_ENV"],
    },
    Spec {
        name: "rust",
        symbol: "🦀 ",
        style: "bold red",
        files: &["Cargo.toml"],
        exts: &["rs"],
        folders: &[],
        bins: &["rustc"],
        args: &["--version"],
        pins: &["rust-toolchain.toml", "rust-toolchain"],
        env_pins: &["RUSTUP_TOOLCHAIN"],
    },
    Spec {
        name: "golang",
        symbol: "🐹 ",
        style: "bold cyan",
        files: &["go.mod", "go.sum", "go.work", "glide.yaml", "Gopkg.yml", "Gopkg.lock", ".go-version"],
        exts: &["go"],
        folders: &["Godeps"],
        bins: &["go"],
        args: &["version"],
        pins: &[".go-version", ".tool-versions", "mise.toml"],
        env_pins: &["GOTOOLCHAIN"],
    },
    Spec {
        name: "java",
        symbol: "☕ ",
        style: "red dimmed",
        files: &["pom.xml", "build.gradle.kts", "build.sbt", ".java-version", "deps.edn", "project.clj", "build.boot", ".sdkmanrc"],
        exts: &["java", "class", "jar", "gradle", "clj", "cljc"],
        folders: &[],
        bins: &["java"],
        args: &["-version"],
        pins: &[".java-version", ".sdkmanrc", ".tool-versions"],
        env_pins: &["JAVA_HOME"],
    },
    Spec {
        name: "ruby",
        symbol: "💎 ",
        style: "bold red",
        files: &["Gemfile", ".ruby-version"],
        exts: &["rb"],
        folders: &[],
        bins: &["ruby"],
        args: &["-v"],
        pins: &[".ruby-version", ".tool-versions"],
        env_pins: &["RUBY_VERSION", "RBENV_VERSION"],
    },
    Spec {
        name: "lua",
        symbol: "🌙 ",
        style: "bold blue",
        files: &[".lua-version"],
        exts: &["lua"],
        folders: &["lua"],
        bins: &["lua"],
        args: &["-v"],
        pins: &[".lua-version"],
        env_pins: &[],
    },
    Spec {
        name: "php",
        symbol: "🐘 ",
        style: "147 bold",
        files: &["composer.json", ".php-version"],
        exts: &["php"],
        folders: &[],
        bins: &["php"],
        args: &["-nr", "echo PHP_VERSION;"],
        pins: &[".php-version", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "swift",
        symbol: "🐦 ",
        style: "bold 202",
        files: &["Package.swift"],
        exts: &["swift"],
        folders: &[],
        bins: &["swift"],
        args: &["--version"],
        pins: &[".swift-version"],
        env_pins: &["TOOLCHAINS"],
    },
    Spec {
        name: "deno",
        symbol: "🦕 ",
        style: "green bold",
        files: &["deno.json", "deno.jsonc", "deno.lock", "mod.ts", "mod.js", "deps.ts", "deps.js"],
        exts: &[],
        folders: &[],
        bins: &["deno"],
        args: &["-V"],
        pins: &[".dvmrc", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "bun",
        symbol: "🥟 ",
        style: "bold red",
        files: &["bun.lock", "bun.lockb", "bunfig.toml"],
        exts: &[],
        folders: &[],
        bins: &["bun"],
        args: &["--version"],
        pins: &[".bun-version", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "kotlin",
        symbol: "🅺 ",
        style: "bold blue",
        files: &[],
        exts: &["kt", "kts"],
        folders: &[],
        bins: &["kotlin"],
        args: &["-version"],
        pins: &[".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "dart",
        symbol: "🎯 ",
        style: "bold blue",
        files: &["pubspec.yaml", "pubspec.yml", "pubspec.lock"],
        exts: &["dart"],
        folders: &[".dart_tool"],
        bins: &["dart"],
        args: &["--version"],
        pins: &[".fvmrc", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "perl",
        symbol: "🐪 ",
        style: "149 bold",
        files: &["Makefile.PL", "Build.PL", "cpanfile", "cpanfile.snapshot", "META.json", "META.yml", ".perl-version"],
        exts: &["pl", "pm", "pod"],
        folders: &[],
        bins: &["perl"],
        args: &["-e", "printf q#%vd#,$^V;"],
        pins: &[".perl-version", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "terraform",
        symbol: "💠 ",
        style: "bold 105",
        files: &[],
        exts: &["tf", "tfplan", "tfstate"],
        folders: &[".terraform"],
        bins: &["terraform"],
        args: &["version"],
        pins: &[".terraform-version", ".tool-versions"],
        env_pins: &[],
    },
    Spec {
        name: "zig",
        symbol: "↯ ",
        style: "bold yellow",
        files: &[],
        exts: &["zig"],
        folders: &[],
        bins: &["zig"],
        args: &["version"],
        pins: &[".tool-versions"],
        env_pins: &[],
    },
];

/// Cached in place of a version when a binary exists but couldn't report one.
const NO_VERSION: &str = "-";

fn is_shim(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("shims") || s.contains("/.cargo/bin/") || s.contains("mise") || s.contains("asdf")
}

/// The tool's raw version (`26.10.0`), from the cache when possible.
fn version(ctx: &Context, spec: &Spec, cfg: Cfg) -> Option<String> {
    let bins = if spec.name == "python" { cfg.strs("python_binary", spec.bins) } else { spec.bins.iter().map(|s| s.to_string()).collect() };
    let env: String = spec.env_pins.iter().map(|k| ctx.var(k).unwrap_or_default()).collect::<Vec<_>>().join(",");
    let pin = util::nearest_pin(&ctx.cwd, spec.pins);
    // Try each binary in turn: a pyenv shim for `python` can exist yet fail ("command
    // not found in this version"), in which case `python3` is the answer.
    for bin in bins.iter().filter_map(|b| util::which(ctx, b)) {
        let key = format!("{}|{}|{}|{pin}|{env}", bin.display(), util::mtime(&bin), spec.args.join(" "));
        match ctx.versions.get(&key, is_shim(&bin)).as_deref() {
            // Remembered failure: a slow shim that can't answer here.
            Some(NO_VERSION) => continue,
            Some(v) => return Some(v.to_string()),
            None => {}
        }
        // `java -version` and older pythons print to stderr.
        let raw = util::run(ctx, &bin, spec.args)
            .filter(|o| o.ok)
            .and_then(|o| util::find_version(&o.stdout).or_else(|| util::find_version(&o.stderr)));
        ctx.versions.put(key, raw.clone().unwrap_or_else(|| NO_VERSION.to_string()));
        if raw.is_some() {
            return raw;
        }
    }
    None
}

pub fn formatted_version(ctx: &Context, name: &str, cfg: Cfg, raw: &str) -> String {
    let mut v = Vars::new();
    for (k, val) in util::version_vars(raw) {
        v.text(k, val);
    }
    let segs = super::sub_format(ctx, name, &cfg.str("version_format", "v${raw}"), &v);
    super::super::style::segs_text(&segs)
}

pub fn render(ctx: &Context, name: &str, cfg: Cfg) -> Option<Vec<Seg>> {
    let spec = SPECS.iter().find(|s| s.name == name)?;
    let venv = if name == "python" { ctx.var("VIRTUAL_ENV") } else { None };
    if !detected(ctx, cfg, spec.files, spec.exts, spec.folders) && venv.is_none() {
        return None;
    }
    let mut v = Vars::new();
    let raw = version(ctx, spec, cfg);
    v.text("version", raw.map(|r| formatted_version(ctx, name, cfg, &r)).unwrap_or_default());
    let mut format = FORMAT.to_string();
    if name == "python" {
        let venv_name = venv
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        v.text("virtualenv", venv_name).text("pyenv_prefix", "");
        format = "via [${symbol}${pyenv_prefix}(${version} )(\\($virtualenv\\) )]($style)".into();
    }
    finish(ctx, name, cfg, &format, spec.style, spec.symbol, &mut v)
}

/// The project's own version from package.json, Cargo.toml or pyproject.toml.
fn package_version(ctx: &Context, cfg: Cfg) -> Option<String> {
    let l = listing(ctx);
    let read = |f: &str| std::fs::read_to_string(ctx.cwd.join(f)).ok();
    if l.files.contains("package.json")
        && let Some(text) = read("package.json")
        && let Ok(doc) = text.parse::<serde_like::Json>()
    {
        if doc.private && !cfg.bool("display_private", false) {
            return None;
        }
        if let Some(v) = doc.version {
            return Some(v);
        }
    }
    let toml_version = |f: &str, paths: &[&[&str]]| -> Option<String> {
        let doc = read(f)?.parse::<toml_edit::DocumentMut>().ok()?;
        paths.iter().find_map(|path| {
            let mut item = doc.as_item();
            for p in *path {
                item = item.get(p)?;
            }
            item.as_str().map(String::from)
        })
    };
    if l.files.contains("Cargo.toml")
        && let Some(v) = toml_version("Cargo.toml", &[&["package", "version"], &["workspace", "package", "version"]])
    {
        return Some(v);
    }
    if l.files.contains("pyproject.toml") {
        return toml_version("pyproject.toml", &[&["project", "version"], &["tool", "poetry", "version"]]);
    }
    None
}

pub fn package(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let raw = package_version(ctx, cfg)?;
    let mut v = Vars::new();
    v.text("version", formatted_version(ctx, "package", cfg, raw.trim_start_matches('v')));
    finish(ctx, "package", cfg, "is [$symbol$version]($style) ", "208 bold", "📦 ", &mut v)
}

/// Just enough JSON to read `version` and `private` from the top level of package.json.
mod serde_like {
    pub struct Json {
        pub version: Option<String>,
        pub private: bool,
    }

    impl std::str::FromStr for Json {
        type Err = ();
        fn from_str(s: &str) -> Result<Json, ()> {
            let mut depth = 0i32;
            let mut out = Json { version: None, private: false };
            let b = s.as_bytes();
            let mut i = 0;
            let mut last_key: Option<String> = None;
            let mut expecting_value = false;
            while i < b.len() {
                match b[i] {
                    b'{' | b'[' => {
                        depth += 1;
                        expecting_value = false;
                    }
                    b'}' | b']' => depth -= 1,
                    b':' => expecting_value = true,
                    b',' => expecting_value = false,
                    b'"' => {
                        let start = i + 1;
                        let mut j = start;
                        while j < b.len() && b[j] != b'"' {
                            if b[j] == b'\\' {
                                j += 1;
                            }
                            j += 1;
                        }
                        let text = s.get(start..j.min(b.len())).unwrap_or("").to_string();
                        if depth == 1 {
                            if expecting_value {
                                if last_key.as_deref() == Some("version") {
                                    out.version = Some(text);
                                }
                                expecting_value = false;
                            } else {
                                last_key = Some(text);
                            }
                        }
                        i = j;
                    }
                    b't' if depth == 1 && expecting_value && s[i..].starts_with("true") => {
                        if last_key.as_deref() == Some("private") {
                            out.private = true;
                        }
                        expecting_value = false;
                    }
                    _ => {}
                }
                i += 1;
            }
            Ok(out)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::Json;

        #[test]
        fn reads_top_level_only() {
            let j: Json = r#"{"name":"x","dependencies":{"version":"9"},"version":"0.13.37","private":true}"#.parse().expect("json");
            assert_eq!(j.version.as_deref(), Some("0.13.37"));
            assert!(j.private);
        }
    }
}

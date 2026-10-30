<h1 align="center">▓▒░ custtymize ░▒▓</h1>

<p align="center">
  <b>Forge your <a href="https://starship.rs">Starship</a> prompt in a hacker console.</b><br>
  Live preview rendered by real starship · colour picker · presets &amp; shapes · before/after review · one-key backups
</p>

<p align="center">
  <a href="https://mikemich.github.io/custtymize/">Website</a> ·
  <a href="#install">Install</a> ·
  <a href="#keys">Keys</a> ·
  <a href="#commands-">Commands</a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.88%2B-ffb86c?style=flat-square&logo=rust&logoColor=white&labelColor=282a36">
  <img alt="ratatui" src="https://img.shields.io/badge/built%20with-ratatui-bd93f9?style=flat-square&labelColor=282a36">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-50fa7b?style=flat-square&labelColor=282a36">
</p>

<p align="center">
  <img src="docs/assets/main.svg" alt="custtymize: live prompt preview, module list, inspector and log" width="100%">
</p>

Pick modules, colours, shapes and presets, and watch the **real** `starship prompt`
output update live, in both success and error states. Nothing is written to
`~/.config/starship.toml` until you have seen the before, the after and the diff.

| | |
| --- | --- |
| <img src="docs/assets/picker.svg" alt="colour picker"> | <img src="docs/assets/review.svg" alt="install review with before/after and diff"> |
| **Colour picker** with live preview | **Review before install**: before, after, diff |

**Presets × shapes**, all rendered by starship:

<img src="docs/assets/prompt-dracula.svg" alt="dracula powerline" height="34"><br>
<img src="docs/assets/prompt-tokyo-night.svg" alt="tokyo-night rounded" height="34"><br>
<img src="docs/assets/prompt-catppuccin-mocha.svg" alt="catppuccin-mocha slanted" height="34"><br>
<img src="docs/assets/prompt-gruvbox.svg" alt="gruvbox powerline" height="34"><br>
<img src="docs/assets/prompt-nord.svg" alt="nord rounded" height="34">

## Install

Requires Rust (1.88+) and `starship` on your `PATH`.

```bash
cargo install --git https://github.com/mikemich/custtymize
```

or from a clone:

```bash
cargo install --path .
```

Then run `custtymize`. A Nerd Font (e.g. JetBrains Mono Nerd Font) is needed for the
powerline / rounded / slanted glyphs.

### CLI

| flag | what it does |
| --- | --- |
| `-c, --config <PATH>` | config to edit (default `$STARSHIP_CONFIG`, then `~/.config/starship.toml`) |
| `--dry-run` | print the generated TOML to stdout and exit, writing nothing |
| `--preset <NAME>` | apply a palette preset first (works with `--dry-run`) |
| `--shape <SHAPE>` | apply a segment shape first (works with `--dry-run`) |
| `--print-preview` | render the prompt once with real starship, print the ANSI and exit |
| `--no-boot` | skip the boot sequence |
| `--ansi256` / `--truecolor` | force the UI colour depth (auto-detected otherwise) |

```bash
custtymize --dry-run --preset tokyo-night --shape powerline > /tmp/prompt.toml
```

## Keys

| key | action |
| --- | --- |
| `j` `k` / `↑` `↓` | move selection |
| `tab` | switch between module list and inspector |
| `space` | toggle module on/off |
| `J` `K` | move module down/up in `format` (`character` stays pinned last) |
| `R` | send module to the right prompt (`right_format`) |
| `a` / `D` | add / remove a module |
| `⏎` | edit the field: text inline, colours open the picker |
| `x` / `⌫` | reset field to default (format → generated, option → starship default) |
| `p` | Nerd Font glyph picker (on the symbol field; `tab` from symbol edit too) |
| `g` | global options popup (layout, shape, cap, prompt char, colours, palette) |
| `S` `L` `C` `P` | cycle shape / layout / left cap / palette preset |
| `s` | cycle preview scenario: dirty repo, clean repo, deep path, ssh, home |
| `u` / `ctrl-r` | undo / redo |
| `ctrl-s` | install: opens the review screen (before/after prompts + diff); `y` writes |
| `B` | browse backups: preview each one, restore it or load it into the editor |
| `t` | full generated TOML, with `+` on lines that differ from disk |
| `y` | copy the generated TOML to the clipboard (`pbcopy`) |
| `?` | help |
| `q` / `ctrl-c` | quit (asks if there are unsaved changes) |

**Colour picker:** `hjkl` to move (the preview updates live), `#` to type a hex value,
`⏎` to apply, `esc` to revert. Rows: Dracula swatches, the active palette, every other
preset, and a grey ramp.

## Commands (`:`)

`tab` completes commands and arguments; `↑` `↓` walk the history.

| command | |
| --- | --- |
| `:w [path]` | validate → review before/after → back up → install (or save a copy to `path`) |
| `:backups` | browse, preview and restore previous setups |
| `:w!` | save even if the file changed on disk since it was loaded |
| `:q` / `:q!` / `:wq` / `:x` | quit / quit discarding / save & quit |
| `:preset <name>` | `dracula` `tokyo-night` `catppuccin-mocha` `gruvbox` `phosphor` `amber` `synthwave` `nord` |
| `:undo` / `:redo` | |
| `:export <path>` | validate and write the TOML elsewhere |
| `:shape <s>` | `powerline` `rounded` `slanted` `flat` `plain` |
| `:layout <l>` | `one` `two` `right` |
| `:cap <c>` | `gradient` (░▒▓) `rounded` `none` |
| `:char <glyph>` | prompt character |
| `:set <module>.<key> <value>` | e.g. `:set git_branch.bg #ff79c6`, `:set directory.truncation_length 2` |
| `:add <module>` / `:rm [module]` | add any starship module (catalog extras: `package` `java` `kubernetes` `aws` `memory_usage` `jobs` `status` `sudo` …) |
| `:scenario <s>` | preview context |
| `:toml` / `:yank` | view / copy the generated TOML |
| `:random` | random golden-angle colour scheme |
| `:reload[!]` / `:reset` | reload from disk / back to the last saved state |
| `:newline` | toggle `add_newline` |
| `:colors true\|256` | UI colour depth |
| `:clear` | clear the log |

## Safety

* **Preview before install.** Saving to your live config (`ctrl-s`, `:w`, `:wq`) never
  writes straight away. It opens a review screen with your **current** prompt and the
  **new** one, both rendered by real starship in success and error states, plus a
  line diff of the TOML. `y` installs; anything else cancels and nothing is written.
* **Backups.** Every install first copies the existing file to
  `starship.toml.bak-YYYYmmdd-HHMMSS`. `B` (or `:backups`) lists every
  `starship.toml.bak*` next to your config, including hand-made ones like
  `.bak-tokyo`, with a live preview of each. `⏎` restores one through the same review
  screen and backs up the current config first, so a restore can itself be undone.
  `l` loads a backup's modules and colours into the editor without touching the file.
* **Validation.** The generated TOML must parse, and `starship prompt` must run cleanly
  against it. Starship exits 0 even for a broken config, so its stderr is checked too:
  config-schema errors and unparsable module formats block the save, and the reason is
  shown in the log pane.
* **Concurrent edits.** If the file changed on disk after custtymize loaded it, `:w`
  refuses; use `:w!` to overwrite or `:reload!` to discard your edits.
* **Round-trip.** Editing uses `toml_edit`, so comments, key order and anything
  custtymize doesn't manage survive. A no-op save is byte-identical: only modules you
  touched are rewritten. Literal text in your `format` (e.g. `[@](#727169)`) and modules
  outside the catalog (`${custom.foo}`, `$kotlin`) are kept verbatim as movable list
  entries. Hand-written module formats are kept (tagged `custom` in the inspector) until
  you pick a shape, which regenerates them.
* Symlinked configs (dotfiles repos) are written through the link, not over it.
* Writes are atomic (temp file + rename).

## How the preview works

The preview thread writes the working config to `~/Library/Caches/custtymize/` and runs
`starship prompt` four times in parallel: `--status=0` and `--status=1`, each for the
left and right prompts. It sets `STARSHIP_CONFIG`, `PWD` and `--terminal-width`, and
debounces changes by 100 ms. The output is converted with `ansi-to-tui`.

It runs inside a throwaway sandbox (`~/Library/Caches/custtymize/playground/`). The
sandbox has a dirty git repo on `feature/neon-dreams` with modified, staged and
untracked files, plus `Cargo.toml`, `package.json`, `go.mod` and `requirements.txt`. It
also sets a fake `VIRTUAL_ENV` and, for the ssh scenario, `SSH_CONNECTION`.
`cmd_duration` gets a fake `--cmd-duration=4242`. Starship's own log goes to the cache
dir, not `~/.cache/starship`.

## Architecture

```
src/
  main.rs               CLI, terminal setup, event loop (~30 fps for the animations)
  error.rs              thiserror Error
  app.rs                App state machine: modes, input, commands, undo/redo, save
  preview.rs            background starship runner, debounce, validation
  sandbox.rs            fake repos/scenarios for the preview
  model/
    mod.rs              Segment, Globals, Model (+ Shape/Layout/Cap enums)
    color.rs            Rgb, hex/style parsing, xterm-256 fallback, Dracula constants
    catalog.rs          per-module knowledge: body variables, symbol mode, options
    palette.rs          presets
  config/
    generate.rs         Model → TOML (format strings, separator chaining, minimal diff)
    load.rs             TOML → Model (best effort, lossless for what it can't model)
    save.rs             backup + atomic write
  ui/                   theme, boot, preview, modules, inspector, log, status, popups, matrix
```

### The segment model

```rust
pub struct Segment {
    pub name: String,                     // starship module: "git_branch", "custom.vpn"
    pub raw: bool,                        // unknown module: emitted verbatim, table untouched
    pub literal: Option<String>,          // literal format text, e.g. "[@](#727169)"
    pub enabled: bool,
    pub side: Side,                       // Left | Right (right_format)
    pub symbol: Option<String>,           // None = catalog default
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub format_override: Option<String>,  // hand-written module format
    pub options: BTreeMap<String, OptValue>, // module-specific keys that are set
}
```

The generator splits the work between two places:

* Each module's own `format` holds the body wrapped in a conditional group, e.g.
  `([ $symbol $branch ]($style))`, so a module with nothing to show also hides its
  padding.
* The top-level `format` carries the separators for chained shapes. For powerline,
  every boundary is `[](fg:<prev bg> bg:<next bg>)`, or a thin `` when both
  neighbours share a background. Rounded and slanted segments carry their own caps, so
  they need no chaining.

## Tests

```bash
cargo test
```

The tests cover format-string building, separator colour chaining (normal, thin and
right-side), caps, layouts, raw-segment chain breaks, style strings, escaping, comment
and unknown-key preservation, byte-identical no-op round trips, literal-text
preservation, stderr validation rules, backups and palettes.

## Notes

* **Apple Terminal.** macOS 26's Terminal.app supports 24-bit colour but doesn't set
  `COLORTERM`, so custtymize detects it from `TERM_PROGRAM_VERSION`. If the colours look
  wrong, start with `--ansi256` (or run `:colors 256`).
* Press `M`.

## License

[MIT](LICENSE)

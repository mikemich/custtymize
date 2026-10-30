//! Shell integration scripts printed by `custtymize init <shell>`.

use super::Shell;

/// Single-quotes a path for sh-like shells.
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Separates the left and right prompt in `prompt --both` output.
pub const SEPARATOR: char = '\u{1e}';

pub fn script(shell: Shell, exe: &str) -> Option<String> {
    let bin = quote(exe);
    Some(match shell {
        // One process per prompt: precmd renders both sides into variables, and the
        // prompts reference those variables. With prompt_subst, zsh expands `${var}`
        // once and does not re-evaluate what it contains, so a hostile branch name
        // can't run code.
        Shell::Zsh => format!(
            r#"# custtymize prompt (zsh)
zmodload zsh/datetime 2>/dev/null
zmodload zsh/parameter 2>/dev/null
typeset -g __custty_bin={bin}
export CUSTTYMIZE_SHELL=zsh
__custty_preexec() {{ __custty_start=$EPOCHREALTIME }}
__custty_render() {{
  local out
  out="$("$__custty_bin" prompt --shell=zsh --both --status="$__custty_status" --cmd-duration="$__custty_dur" --jobs="$__custty_jobs" --terminal-width="$COLUMNS" --keymap="${{KEYMAP:-}}")"
  __custty_left=${{out%%$'\x1e'*}}
  __custty_right=${{out#*$'\x1e'}}
}}
__custty_precmd() {{
  __custty_status=$?
  __custty_dur=
  if [[ -n ${{__custty_start-}} && -n ${{EPOCHREALTIME-}} ]]; then
    __custty_dur=$(( (EPOCHREALTIME - __custty_start) * 1000 ))
    __custty_dur=${{__custty_dur%%.*}}
  fi
  unset __custty_start
  __custty_jobs=${{#jobstates}}
  KEYMAP= __custty_render
}}
# Vi mode: redraw so `character` can show `vimcmd_symbol`.
__custty_keymap_select() {{
  __custty_render
  zle reset-prompt
}}
if [[ ${{+functions[zle-keymap-select]}} == 0 ]]; then
  zle -N zle-keymap-select __custty_keymap_select
fi
autoload -Uz add-zsh-hook
add-zsh-hook precmd __custty_precmd
add-zsh-hook preexec __custty_preexec
setopt prompt_subst
PROMPT='${{__custty_left}}'
RPROMPT='${{__custty_right}}'
"#
        ),
        // bash has no right prompt; PS1 holds a variable reference for the same reason
        // as zsh, and colour codes are wrapped in \001/\002 instead of \[ \].
        Shell::Bash => format!(
            r#"# custtymize prompt (bash)
__custty_bin={bin}
export CUSTTYMIZE_SHELL=bash
__custty_ready=
__custty_preexec() {{
  [[ -n $__custty_ready ]] || return 0
  __custty_ready=
  __custty_start=${{EPOCHREALTIME//[.,]/}}
}}
__custty_precmd() {{
  local st=$? dur= now
  if [[ -n ${{__custty_start-}} && -n ${{EPOCHREALTIME-}} ]]; then
    now=${{EPOCHREALTIME//[.,]/}}
    dur=$(( (now - __custty_start) / 1000 ))
  fi
  unset __custty_start
  local jl=($(jobs -p))
  __custty_ps1="$("$__custty_bin" prompt --shell=bash --status="$st" --cmd-duration="$dur" --jobs="${{#jl[@]}}" --terminal-width="$COLUMNS")"
  __custty_ready=1
}}
trap '__custty_preexec' DEBUG
if [[ ";${{PROMPT_COMMAND[*]:-}};" != *";__custty_precmd;"* ]]; then
  PROMPT_COMMAND="__custty_precmd${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
fi
PS1='${{__custty_ps1}}'
"#
        ),
        Shell::Fish => format!(
            r#"# custtymize prompt (fish)
set -g __custty_bin {bin}
set -gx CUSTTYMIZE_SHELL fish
function fish_prompt
    set -g __custty_status $status
    set -g __custty_dur $CMD_DURATION
    set -g __custty_jobs (count (jobs -p))
    $__custty_bin prompt --shell=fish --status=$__custty_status --cmd-duration=$__custty_dur --jobs=$__custty_jobs --terminal-width=$COLUMNS --keymap=$fish_bind_mode
end
function fish_right_prompt
    $__custty_bin prompt --shell=fish --right --status=$__custty_status --cmd-duration=$__custty_dur --jobs=$__custty_jobs --terminal-width=$COLUMNS
end
function fish_mode_prompt
end
"#
        ),
        Shell::None => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_quote_the_binary() {
        let z = script(Shell::Zsh, "/Users/o'brien/.cargo/bin/custtymize").expect("zsh");
        assert!(z.contains(r"typeset -g __custty_bin='/Users/o'\''brien/.cargo/bin/custtymize'"), "{z}");
        assert!(z.contains("PROMPT='${__custty_left}'"));
        assert!(script(Shell::Bash, "/x").expect("bash").contains("PS1='${__custty_ps1}'"));
        assert!(script(Shell::Fish, "/x").expect("fish").contains("function fish_prompt"));
    }
}

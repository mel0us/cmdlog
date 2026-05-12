# hook/cmdlog.zsh — Source from ~/.zshrc
# Records interactive commands via cmdlog binary.
# Safe to re-source: functions are redefined, precmd_functions is deduped.
#
# Usage: source /path/to/hook/cmdlog.zsh

# Only activate in interactive shells
[[ -o interactive ]] || return 0

typeset -g __CMDLOG_BIN="$HOME/.local/bin/cmdlog"

# Record function — always redefined on re-source (picks up new binary path).
# Always returns the captured user exit code so downstream precmd_functions
# observe the original $? rather than the binary's exit.
__cmdlog_record() {
    local __cmdlog_ec=$?
    # Get the last history entry (fc is a builtin)
    local cmd
    cmd=$(fc -ln -1) || return "$__cmdlog_ec"
    cmd="${cmd#"${cmd%%[! ]*}"}"
    cmd="${cmd%"${cmd##*[! ]}"}"
    [[ -n "$cmd" ]] || return "$__cmdlog_ec"

    # Delegate all filtering to the binary
    "$__CMDLOG_BIN" record zsh "$PWD" "$__cmdlog_ec" "$cmd"
    return "$__cmdlog_ec"
}

# Install as precmd hook (idempotent — safe on re-source).
# Remove any prior entry, then prepend so we run first and capture the user's $?.
(( ${+precmd_functions} )) || typeset -ga precmd_functions
precmd_functions=("${(@)precmd_functions:#__cmdlog_record}")
precmd_functions=(__cmdlog_record "${precmd_functions[@]}")

# Wrapper function — always redefined on re-source (picks up new binary path)
# Captures TUI selection and injects into edit buffer.
# Only bare `cmdlog` (no args) triggers the TUI capture; everything else
# (install, doctor, compact, ...) passes straight through to the binary.
cmdlog() {
    if [[ $# -eq 0 ]]; then
        local cmd method
        method=$("$__CMDLOG_BIN" config inject.zsh)
        cmd=$("$__CMDLOG_BIN")
        if [[ -n "$cmd" ]]; then
            case "$method" in
                tiocsti)
                    "$__CMDLOG_BIN" inject zsh "$cmd" || true
                    ;;
                history)
                    print -s -- "$cmd"
                    ;;
                *)
                    # "print-z" (default): push onto edit buffer
                    print -z "$cmd"
                    ;;
            esac
        fi
    else
        "$__CMDLOG_BIN" "$@"
    fi
}

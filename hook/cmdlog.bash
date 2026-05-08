# hook/cmdlog.bash — Source from ~/.bashrc
# Records interactive commands via cmdlog binary.
# Safe to re-source: functions are redefined, PROMPT_COMMAND is deduped.
#
# Usage: source /path/to/hook/cmdlog.bash

# Only activate in interactive shells
[[ $- == *i* ]] || return 0

# Derive paths from this script's location (relocatable)
__CMDLOG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
__CMDLOG_BIN="${__CMDLOG_DIR}/cmdlog"

# Record function — always redefined on re-source (picks up new binary path)
__cmdlog_record() {
    local __cmdlog_ec=$?
    local hist
    hist=$(builtin history 1) || return 0

    # Extract command text (strip leading history number)
    local histnum="${hist%%[^0-9 ]*}"
    histnum="${histnum// /}"
    local cmd="${hist#*"${histnum}"}"
    cmd="${cmd#"${cmd%%[![:space:]]*}"}"
    [[ -n "$cmd" ]] || return 0

    # Delegate all filtering to the binary
    "$__CMDLOG_BIN" record bash "$PWD" "$__cmdlog_ec" "$cmd"
}

# Install into PROMPT_COMMAND (idempotent — safe on re-source).
# Use array form on bash 5.1+ for robustness against overrides.
if [[ ${BASH_VERSINFO[0]:-0} -ge 5 && ${BASH_VERSINFO[1]:-0} -ge 1 ]] || [[ ${BASH_VERSINFO[0]:-0} -ge 6 ]]; then
    if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == *"-a"* ]]; then
        # Already an array — prepend if not present (must run first to capture $?)
        __cmdlog_found=0
        for __cmdlog_pc in "${PROMPT_COMMAND[@]}"; do
            [[ "$__cmdlog_pc" == *"__cmdlog_record"* ]] && __cmdlog_found=1 && break
        done
        if [[ $__cmdlog_found -eq 0 ]]; then
            PROMPT_COMMAND=("__cmdlog_record" "${PROMPT_COMMAND[@]}")
        fi
        unset __cmdlog_pc __cmdlog_found
    elif [[ -n "${PROMPT_COMMAND-}" ]]; then
        # String — convert to array only if we're not already in it
        if [[ "$PROMPT_COMMAND" != *"__cmdlog_record"* ]]; then
            PROMPT_COMMAND=("__cmdlog_record" "${PROMPT_COMMAND}")
        fi
    else
        PROMPT_COMMAND=("__cmdlog_record")
    fi
else
    # String form (bash < 5.1)
    if [[ -z "${PROMPT_COMMAND-}" ]]; then
        PROMPT_COMMAND="__cmdlog_record"
    elif [[ "$PROMPT_COMMAND" != *"__cmdlog_record"* ]]; then
        PROMPT_COMMAND="__cmdlog_record;${PROMPT_COMMAND}"
    fi
fi

# Wrapper function — always redefined on re-source (picks up new binary path)
# Captures TUI selection and injects into readline for editing.
cmdlog() {
    if [[ "${1-}" == "list" ]]; then
        local cmd method
        method=$("$__CMDLOG_BIN" config inject.bash)
        cmd=$("$__CMDLOG_BIN" "$@")
        if [[ -n "$cmd" ]]; then
            case "$method" in
                tiocsti)
                    "$__CMDLOG_BIN" inject bash "$cmd" || true
                    ;;
                history)
                    builtin history -s -- "$cmd"
                    ;;
                *)
                    # "readline" (default): inject into readline buffer
                    # Uses DSR query/response as a one-shot trigger:
                    # printf sends ESC[5n, terminal responds ESC[0n,
                    # readline fires the bound callback, which unbinds itself.
                    __cmdlog_pending="$cmd"
                    __cmdlog_readline_inject() {
                        READLINE_LINE="$__cmdlog_pending"
                        READLINE_POINT=${#READLINE_LINE}
                        unset __cmdlog_pending
                        bind -r '"\e[0n"'
                    }
                    bind -x '"\e[0n": __cmdlog_readline_inject'
                    printf '\e[5n' > /dev/tty
                    ;;
            esac
        fi
    else
        "$__CMDLOG_BIN" "$@"
    fi
}

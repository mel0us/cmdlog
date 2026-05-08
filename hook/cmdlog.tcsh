# hook/cmdlog.tcsh — Source from ~/.tcshrc (csh/tcsh)
# Records interactive commands via cmdlog binary.
# Safe to re-source: aliases are redefined, precmd is idempotent.
#
# Usage: source /path/to/hook/cmdlog.tcsh

# Only activate in interactive shells.
# $?prompt is the standard tcsh idiom.
if (! $?prompt) then
    # Non-interactive — do nothing (cannot use return in tcsh)
else

set __cmdlog_bin = ~/.local/bin/cmdlog

# tcsh aliases cannot use if/then/else/endif with semicolons — tcsh fails to
# parse endif when the condition is false, producing an "if?" prompt.  Use only
# the short form: if (expr) single-command.

# Recording alias.
alias __cmdlog_do_record 'set __cmdlog_ec = $status ; set __cmdlog_h = "`history -h 1`" ; if ("$__cmdlog_h" != "") $__cmdlog_bin record tcsh $cwd $__cmdlog_ec "$__cmdlog_h"'

# Chain into precmd if not already present. Skip if already chained.
set __cmdlog_cur_precmd = "`alias precmd`"
if ("$__cmdlog_cur_precmd" !~ *__cmdlog_do_record*) then
    if ("$__cmdlog_cur_precmd" != "") then
        alias precmd "__cmdlog_do_record ; $__cmdlog_cur_precmd"
    else
        alias precmd __cmdlog_do_record
    endif
endif
unset __cmdlog_cur_precmd

# Wrapper: cmdlog list captures stdout and injects into shell.
# Uses short-form if only — each step guarded by $__cmdlog_eval flag.
alias cmdlog 'set __cmdlog_a1 = \!:1 ; set __cmdlog_sel = "" ; set __cmdlog_eval = 0 ; set __cmdlog_method = "" ; set __cmdlog_tmpf = /tmp/.cmdlog_hist.$$ ; if ("$__cmdlog_a1" == "list") set __cmdlog_method = `$__cmdlog_bin config inject.tcsh` ; if ("$__cmdlog_a1" == "list") set __cmdlog_sel = `$__cmdlog_bin \!*` ; if ("$__cmdlog_sel" != "") set __cmdlog_eval = 1 ; if ($__cmdlog_eval) if ("$__cmdlog_method" != "tiocsti") echo "$__cmdlog_sel" > "$__cmdlog_tmpf" ; if ($__cmdlog_eval) if ("$__cmdlog_method" != "tiocsti") history -L "$__cmdlog_tmpf" ; if ($__cmdlog_eval) rm -f "$__cmdlog_tmpf" ; if ($__cmdlog_eval) $__cmdlog_bin inject tcsh "$__cmdlog_sel" ; if ("$__cmdlog_a1" != "list") $__cmdlog_bin \!*'

endif

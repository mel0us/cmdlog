//! Core command logic: record filtering, builtin detection, CLI parsing.
//!
//! Extracted from main.rs for testability.

use std::fs;
use std::io::{BufWriter, Write};
use std::os::unix::fs::OpenOptionsExt;

use crate::log;
use crate::time;

// ---------------------------------------------------------------------------
// Builtin detection (union of bash + zsh + tcsh builtins)
// ---------------------------------------------------------------------------

pub fn is_builtin(cmd: &str) -> bool {
    matches!(
        cmd,
        "alias" | "alloc" | "bg" | "bind" | "bindkey" | "break" | "breaksw"
        | "builtin" | "builtins" | "caller" | "cd" | "chdir" | "command"
        | "compgen" | "complete" | "compopt" | "continue" | "declare"
        | "dirs" | "disown" | "echo" | "echotc" | "enable" | "eval"
        | "exec" | "exit" | "export" | "false" | "fc" | "fg" | "filetest"
        | "foreach" | "getopts" | "glob" | "goto" | "hash" | "hashstat"
        | "help" | "history" | "hup" | "jobs" | "kill" | "let" | "limit"
        | "local" | "log" | "login" | "logout" | "ls-F" | "mapfile"
        | "nice" | "nohup" | "notify" | "onintr" | "popd" | "printf"
        | "printenv" | "pushd" | "pwd" | "read" | "readarray" | "rehash"
        | "repeat" | "return" | "sched" | "set" | "setenv" | "settc"
        | "setty" | "shift" | "shopt" | "source" | "stop" | "suspend"
        | "switch" | "telltc" | "test" | "time" | "times" | "trap"
        | "true" | "type" | "typeset" | "ulimit" | "umask" | "unalias"
        | "uncomplete" | "unhash" | "unlimit" | "unset" | "unsetenv"
        | "wait" | "where" | "which"
    )
}

/// Check if a command contains shell operators (`;`, `&`, `|`, etc.).
/// Used by both record-time and display-time filters to override waive/builtin skipping.
pub fn has_special_chars(cmd: &str) -> bool {
    cmd.bytes().any(|b| b";&|(){}$`!<>".contains(&b))
}

// ---------------------------------------------------------------------------
// Record subcommand
// ---------------------------------------------------------------------------

/// Record a command to the log, applying builtin filter.
pub fn record(shell: &str, pwd: &str, exit_code: &str, raw_cmd: &str) {
    let cmd = raw_cmd.trim();
    if cmd.is_empty() {
        return;
    }

    let first_word = cmd.split_whitespace().next().unwrap_or("");
    let has_special = has_special_chars(cmd);

    // Skip builtins unless the command contains shell operators
    if !has_special && is_builtin(first_word) {
        return;
    }

    // Write log entry (explicit flush — don't rely on BufWriter::drop)
    let ts = time::iso_timestamp();
    let log = log::log_path();
    let debug = std::env::var_os("CMDLOG_DEBUG").is_some();
    match fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&log) {
        Ok(file) => {
            let mut w = BufWriter::new(file);
            if let Err(e) = writeln!(w, "{}\t{}\t{}\t{}\t{}", ts, shell, pwd, exit_code, cmd) {
                if debug { eprintln!("[cmdlog] write error: {}", e); }
            } else if let Err(e) = w.flush() {
                if debug { eprintln!("[cmdlog] flush error: {}", e); }
            }
        }
        Err(e) => {
            if debug { eprintln!("[cmdlog] open error: {} ({})", log.display(), e); }
        }
    }
}

// ---------------------------------------------------------------------------
// Waive filter (shared by TUI, linear list, and compact)
// ---------------------------------------------------------------------------

/// Check if a command should be hidden by the waive list or min-length rule.
/// Returns true if the command should be waived (hidden/removed).
pub fn should_waive(cmd: &str, waive_commands: &[String], min_cmd_len: usize) -> bool {
    if has_special_chars(cmd) {
        return false;
    }
    let mut words = cmd.split_whitespace();
    let first_word = words.next().unwrap_or("");
    let is_single_word = words.next().is_none();
    if waive_commands.iter().any(|w| w == first_word) {
        return true;
    }
    if min_cmd_len > 0 && is_single_word && first_word.len() <= min_cmd_len {
        return true;
    }
    false
}


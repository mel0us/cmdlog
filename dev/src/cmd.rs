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

// ---------------------------------------------------------------------------
// List subcommand options
// ---------------------------------------------------------------------------

pub struct ListOpts {
    pub last_n: Option<usize>,
    pub show_all: bool,
    pub search: Option<String>,
    pub fuzzy: Option<String>,
    pub date: Option<String>,
    pub shell_type: Option<String>,
    pub path_prefix: Option<String>,
    pub today: bool,
    pub here: bool,
    pub no_color: bool,
    pub no_tui: bool,
    pub help: bool,
}

impl Default for ListOpts {
    fn default() -> Self {
        Self {
            last_n: Some(20),
            show_all: false,
            search: None,
            fuzzy: None,
            date: None,
            shell_type: None,
            path_prefix: None,
            today: false,
            here: false,
            no_color: false,
            no_tui: false,
            help: false,
        }
    }
}

pub fn parse_list_args(args: &[String]) -> ListOpts {
    let mut opts = ListOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-n" | "--last" => {
                i += 1;
                if i < args.len() {
                    opts.last_n = args[i].parse().ok();
                }
            }
            "-a" | "--all" => opts.show_all = true,
            "-s" | "--search" => {
                i += 1;
                if i < args.len() {
                    opts.search = Some(args[i].clone());
                }
            }
            "-f" | "--fuzzy" => {
                i += 1;
                if i < args.len() {
                    opts.fuzzy = Some(args[i].clone());
                }
            }
            "-d" | "--date" => {
                i += 1;
                if i < args.len() {
                    opts.date = Some(args[i].clone());
                }
            }
            "-t" | "--shell-type" => {
                i += 1;
                if i < args.len() {
                    opts.shell_type = Some(args[i].clone());
                }
            }
            "-p" | "--path" => {
                i += 1;
                if i < args.len() {
                    opts.path_prefix = Some(args[i].clone());
                }
            }
            "--today" => opts.today = true,
            "--here" => opts.here = true,
            "--no-color" => opts.no_color = true,
            "--no-tui" => opts.no_tui = true,
            "-h" | "--help" => {
                opts.help = true;
            }
            _ => {}
        }
        i += 1;
    }
    opts
}

//! cmdlog — Cross-shell command logger and query tool.
//!
//! Usage:
//!   cmdlog record <shell> <pwd> <exit_code> <cmd>    Record a command (called by shell hooks)
//!   cmdlog list [options]                Query/filter the log
//!
//! The binary handles builtin detection, timestamping, and log querying —
//! replacing per-shell scripts and Python hist.

use std::env;
use std::io::{self, BufWriter, Write};
use std::process;

use cmdlog::{cmd, log, time, tui};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

fn home_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
}

fn cmdlog_dir() -> std::path::PathBuf {
    home_dir().join(".local/share/cmdlog")
}

// ---------------------------------------------------------------------------
// record subcommand
// ---------------------------------------------------------------------------

fn cmd_record(shell: &str, pwd: &str, exit_code: &str, raw_cmd: &str) {
    cmd::record(shell, pwd, exit_code, raw_cmd);
}

// ---------------------------------------------------------------------------
// install / uninstall subcommands
// ---------------------------------------------------------------------------

fn cmd_install(shell: &str, force: bool) {
    let home = home_dir();

    if cmdlog::hook::hook_source(shell).is_none() {
        eprintln!("Unknown shell: {}. Expected: bash, zsh, tcsh", shell);
        process::exit(1);
    }

    // tcsh can't use eval (backtick collapses newlines) — extract the embedded
    // hook to disk so the rc block can `source` it.
    if shell == "tcsh" {
        if let Err(e) = cmdlog::hook::write_tcsh_hook(&home) {
            eprintln!("{}", e);
            process::exit(1);
        }
    }

    let candidates = cmdlog::hook::rc_candidates(shell, &home);
    let rc_path = match cmdlog::hook::find_rc_file(&candidates) {
        Some(p) => p,
        None => {
            let tried: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
            eprintln!("No rc file found for {}. Tried: {}", shell, tried.join(", "));
            process::exit(1);
        }
    };

    let reinstalled = if force {
        match cmdlog::hook::uninstall_hook(&rc_path) {
            Ok(()) => true,
            Err(_) => false, // no existing hook to remove
        }
    } else {
        false
    };

    match cmdlog::hook::install_hook(&rc_path, shell, &home) {
        Ok(()) => {
            let verb = if reinstalled { "Reinstalled" } else { "Installed" };
            println!("{} cmdlog hook in {}", verb, rc_path.display());
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn cmd_hook(shell: &str) {
    match cmdlog::hook::hook_source(shell) {
        Some(src) => {
            // Embedded files already end with a newline.
            let stdout = io::stdout();
            let mut out = BufWriter::new(stdout.lock());
            let _ = out.write_all(src.as_bytes());
        }
        None => {
            eprintln!("Unknown shell: {}. Expected: bash, zsh, tcsh", shell);
            process::exit(1);
        }
    }
}

fn cmd_uninstall(shell: &str) {
    let home = home_dir();

    if cmdlog::hook::hook_source(shell).is_none() {
        eprintln!("Unknown shell: {}. Expected: bash, zsh, tcsh", shell);
        process::exit(1);
    }

    let candidates = cmdlog::hook::rc_candidates(shell, &home);
    for candidate in &candidates {
        if !candidate.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(candidate) {
            if content.contains(cmdlog::hook::GUARD_BEGIN) {
                match cmdlog::hook::uninstall_hook(candidate) {
                    Ok(()) => {
                        println!("Removed cmdlog hook from {}", candidate.display());
                        return;
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        process::exit(1);
                    }
                }
            }
        }
    }

    let tried: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
    eprintln!("cmdlog hook not found in any rc file for {}.", shell);
    eprintln!("Tried: {}", tried.join(", "));
    process::exit(1);
}

// ---------------------------------------------------------------------------
// list subcommand
// ---------------------------------------------------------------------------

fn print_list_help() {
    println!(
        "Usage: cmdlog list [options]

Options:
  -n, --last N        Show last N entries (default: 20)
  -a, --all           Show all entries
  -s, --search PAT    Filter command text (substring, case-insensitive)
  -f, --fuzzy PAT     Filter command text (fuzzy, fzf-style)
  -d, --date PREFIX   Filter by date prefix (e.g. 2026-04-06)
  -t, --shell-type S  Filter by shell (bash, zsh, tcsh)
  -p, --path PREFIX   Filter by working directory prefix
  --today             Today's entries only
  --here              Current directory only
  --no-color          Disable colored output
  --no-tui            Disable interactive mode
  -h, --help          Show this help

Log file: ~/.cmdlog.tsv (override with CMDLOG_FILE env var)"
    );
}

fn cmd_list(args: &[String]) {
    let opts = cmd::parse_list_args(args);

    if opts.help {
        print_list_help();
        process::exit(0);
    }

    let dir = cmdlog_dir();
    let current_shell = detect_shell().unwrap_or_default();

    // Load and validate config in one pass — create from defaults if missing
    let conf = config_path();
    let content = match std::fs::read_to_string(&conf) {
        Ok(c) => c,
        Err(_) => {
            tui::config::init_config(&conf, &dir.join("default.conf"));
            std::fs::read_to_string(&conf).unwrap_or_default()
        }
    };
    let (mut config_state, issues) = tui::config::validate_and_load(&content, &current_shell);
    if !issues.is_empty() {
        use tui::config::IssueKind;
        for issue in &issues {
            match issue.kind {
                IssueKind::UnknownKey => {
                    eprintln!(
                        "[cmdlog] unknown key [{}] {}. Valid keys: {}",
                        issue.section, issue.key,
                        issue.valid.join(", "),
                    );
                }
                IssueKind::UnknownSection => {
                    eprintln!(
                        "[cmdlog] unknown section [{}]. Valid sections: {}",
                        issue.key,
                        issue.valid.join(", "),
                    );
                }
                _ => {
                    eprintln!(
                        "[cmdlog] invalid [{}] {} = \"{}\". Valid: {}",
                        issue.section, issue.key, issue.value,
                        issue.valid.join(", "),
                    );
                    if !issue.hint.is_empty() {
                        eprintln!("  {}", issue.hint);
                    }
                }
            }
        }
        eprintln!("[cmdlog] run 'cmdlog doctor' to fix.");
        process::exit(1);
    }

    let current_dir = env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // TUI mode: stderr must be a TTY (TUI renders there), no --no-color, no --no-tui
    let stderr_tty = is_tty(2);
    if stderr_tty && !opts.no_color && !opts.no_tui {
        let entries = log::load_entries();
        if entries.is_empty() {
            eprintln!("No matching entries.");
            return;
        }

        config_state.session.current_dir = current_dir.clone();

        if let Some(cmd) = tui::run(entries, &dir, &conf, config_state, &current_shell, &current_dir) {
            println!("{}", cmd);
        }
        return;
    }

    run_linear_list(&opts, &config_state, &current_dir);
}

/// Apply CLI filters, limit, and display entries in linear mode.
fn run_linear_list(opts: &cmd::ListOpts, config_state: &tui::state::AppState, current_dir: &str) {
    let use_color = !opts.no_color && is_tty(1);
    let today_str = if opts.today {
        Some(time::today_prefix())
    } else {
        None
    };
    let cwd = if opts.here {
        Some(current_dir.to_string())
    } else {
        None
    };
    let search_lower = opts.search.as_ref().map(|s| s.to_lowercase());
    // Pre-segment fuzzy needle once (NeedleBuf is reused across all entries).
    let fuzzy_query = opts.fuzzy.as_deref();
    let fuzzy_buf = fuzzy_query.map(tui::filter::NeedleBuf::new);

    let waive_commands = &config_state.session.waive_commands;
    let waive_min_cmd_len = config_state.session.waive_min_cmd_len;

    // Read and filter
    let all = log::load_entries();
    let mut entries: Vec<log::LogEntry> = all
        .into_iter()
        .filter(|entry| {
            if let Some(ref t) = today_str {
                if !entry.date.starts_with(t.as_str()) {
                    return false;
                }
            }
            if let Some(ref d) = opts.date {
                if !entry.date.starts_with(d.as_str()) {
                    return false;
                }
            }
            if let Some(ref st) = opts.shell_type {
                if entry.shell != *st {
                    return false;
                }
            }
            if let Some(ref pp) = opts.path_prefix {
                if !entry.pwd.starts_with(pp.as_str()) {
                    return false;
                }
            }
            if let Some(ref c) = cwd {
                if entry.pwd != *c {
                    return false;
                }
            }
            if let Some(ref sl) = search_lower {
                if !entry.cmd.to_lowercase().contains(sl.as_str()) {
                    return false;
                }
            }
            if let (Some(q), Some(buf)) = (fuzzy_query, fuzzy_buf.as_ref()) {
                if tui::filter::fuzzy_score(&entry.cmd, q, buf).is_none() {
                    return false;
                }
            }
            if cmd::should_waive(&entry.cmd, waive_commands, waive_min_cmd_len) {
                return false;
            }
            true
        })
        .collect();

    // Limit
    if !opts.show_all {
        let n = opts.last_n.unwrap_or(20);
        if entries.len() > n {
            entries = entries.split_off(entries.len() - n);
        }
    }

    if entries.is_empty() {
        eprintln!("No matching entries.");
        return;
    }

    // Display
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for entry in &entries {
        let dir_display = if entry.pwd == current_dir { tui::ui::PWD_DISPLAY } else { &entry.pwd };
        if use_color {
            let ec_color = if entry.exit_code != "0" { "31" } else { "90" };
            let _ = writeln!(
                out,
                "\x1b[90m{}\x1b[0m  \x1b[36m{:5}\x1b[0m  \x1b[{}m{}\x1b[0m  \x1b[33m{}\x1b[0m  {}",
                entry.date, entry.shell, ec_color, entry.exit_code, dir_display, entry.cmd
            );
        } else {
            let _ = writeln!(out, "{}  {:5}  {}  {}  {}", entry.date, entry.shell, entry.exit_code, dir_display, entry.cmd);
        }
    }
}

fn is_tty(fd: i32) -> bool {
    #[cfg(unix)]
    {
        extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        unsafe { isatty(fd) != 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = fd;
        false
    }
}

// ---------------------------------------------------------------------------
// inject subcommand — push text into terminal input via TIOCSTI
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[allow(non_camel_case_types)]
type libc_ioctl_t = u64;

#[cfg(unix)]
const TIOCSTI: libc_ioctl_t = 0x5412;

#[cfg(unix)]
extern "C" {
    fn ioctl(fd: i32, request: libc_ioctl_t, ...) -> i32;
}

#[cfg(unix)]
fn try_tiocsti(text: &str) -> bool {
    // Suppress terminal echo while pushing TIOCSTI bytes so they don't
    // appear twice (once from echo, once when the shell reads them).
    // Uses an opaque buffer for termios to avoid struct layout assumptions.
    extern "C" {
        fn tcgetattr(fd: i32, buf: *mut u8) -> i32;
        fn tcsetattr(fd: i32, action: i32, buf: *const u8) -> i32;
    }
    // c_lflag is the 4th u32 field in Linux termios (after c_iflag, c_oflag,
    // c_cflag). tcflag_t is u32 on all Linux architectures, so offset 12
    // is stable regardless of struct tail layout (NCCS, speed fields).
    const LFLAG_OFFSET: usize = 12;
    const ECHO: u32 = 0o10;
    const TCSANOW: i32 = 0;

    let mut saved = [0u8; 128]; // oversized for any Linux termios variant
    let have_termios = unsafe { tcgetattr(0, saved.as_mut_ptr()) == 0 };
    if have_termios {
        let mut noecho = saved;
        let lflag = u32::from_ne_bytes(noecho[LFLAG_OFFSET..LFLAG_OFFSET + 4].try_into().unwrap());
        noecho[LFLAG_OFFSET..LFLAG_OFFSET + 4].copy_from_slice(&(lflag & !ECHO).to_ne_bytes());
        unsafe { tcsetattr(0, TCSANOW, noecho.as_ptr()); }
    }

    let mut ok = true;
    for byte in text.bytes() {
        if unsafe { ioctl(0, TIOCSTI, &byte as *const u8) } < 0 {
            ok = false;
            break;
        }
    }

    if have_termios {
        unsafe { tcsetattr(0, TCSANOW, saved.as_ptr()); }
    }
    ok
}

/// Match a process name to a known shell, stripping login-shell `-` prefix.
fn match_shell(name: &str) -> Option<String> {
    let name = name.strip_prefix('-').unwrap_or(name);
    match name {
        "bash" | "zsh" | "tcsh" | "csh" => Some(name.to_string()),
        _ => None,
    }
}

#[cfg(unix)]
extern "C" { fn getppid() -> i32; }

/// Detect the calling shell by reading the parent process name.
/// Uses /proc/<ppid>/comm on Linux, proc_pidpath() on macOS.
#[cfg(target_os = "linux")]
fn detect_parent_shell() -> Option<String> {
    let ppid = unsafe { getppid() };
    let comm = std::fs::read_to_string(format!("/proc/{}/comm", ppid)).ok()?;
    match_shell(comm.trim())
}

#[cfg(target_os = "macos")]
fn detect_parent_shell() -> Option<String> {
    extern "C" {
        fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
    }
    let ppid = unsafe { getppid() };
    let mut buf = [0u8; 1024];
    let ret = unsafe { proc_pidpath(ppid, buf.as_mut_ptr(), buf.len() as u32) };
    if ret <= 0 { return None; }
    let path = std::str::from_utf8(&buf[..ret as usize]).ok()?;
    match_shell(path.rsplit('/').next().unwrap_or(""))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detect_parent_shell() -> Option<String> {
    None
}

/// Detect shell: try parent process name, fall back to $SHELL basename.
fn detect_shell() -> Option<String> {
    detect_parent_shell()
        .or_else(|| env::var("SHELL").ok()
            .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
            .filter(|n| !n.is_empty()))
}

fn config_path() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
    )
    .join(".cmdlog.conf")
}

#[cfg(unix)]
fn cmd_inject(shell: &str, text: &str) {
    let config_path = config_path();
    let method = tui::config::load_inject_method(&config_path, shell);

    if method != "tiocsti" || !try_tiocsti(text) {
        process::exit(1);
    }
}

fn cmd_config(key: &str) {
    let config_path = config_path();

    if let Some(shell) = key.strip_prefix("inject.") {
        println!("{}", tui::config::load_inject_method(&config_path, shell));
    } else {
        eprintln!("Unknown config key: {}", key);
        process::exit(1);
    }
}

fn check_log_file_private() {
    use std::os::unix::fs::PermissionsExt;
    let path = log::log_path();
    let Ok(meta) = std::fs::metadata(&path) else { return };
    let mode = meta.permissions().mode() & 0o777;
    if mode == 0o600 {
        return;
    }
    log::enforce_private(&path);
    eprintln!(
        "[cmdlog] tightened {} mode {:04o} -> 0600 (owner-only).",
        path.display(),
        mode,
    );
}

/// Check and repair config for a shell. Exit 0 if healthy, 1 if hard issues found.
fn cmd_doctor(shell: &str) {
    let config_path = config_path();
    let dir = cmdlog_dir();
    let default_conf = dir.join("default.conf");

    let issues = tui::config::doctor_config(&config_path, &default_conf, shell);

    check_log_file_private();

    if issues.is_empty() {
        eprintln!("[cmdlog] config ok.");
        return;
    }

    use tui::config::IssueKind;
    for issue in &issues {
        match issue.kind {
            IssueKind::FileCreated => {
                eprintln!("[cmdlog] created ~/.cmdlog.conf from defaults.");
            }
            IssueKind::FileRegenerated => {
                eprintln!("[cmdlog] config was unparseable: {}", issue.value);
                eprintln!("  Backed up to .cmdlog.conf.bak, regenerated from defaults.");
            }
            IssueKind::TypeFixed => {
                eprintln!(
                    "[cmdlog] wrong type [{}] {} = {}. {} — replaced with {}",
                    issue.section, issue.key, issue.value, issue.hint, issue.fixed_to,
                );
            }
            IssueKind::SectionFilled => {
                eprintln!(
                    "[cmdlog] missing section [{}] — added with defaults",
                    issue.section,
                );
            }
            IssueKind::KeyFilled => {
                eprintln!(
                    "[cmdlog] missing [{}] {} — added default {}",
                    issue.section, issue.key, issue.fixed_to,
                );
            }
            IssueKind::EnumFixed => {
                eprintln!(
                    "[cmdlog] invalid [{}] {} = \"{}\". Switched to \"{}\". Valid: {}",
                    issue.section, issue.key, issue.value, issue.fixed_to,
                    issue.valid.join(", "),
                );
            }
            IssueKind::UnknownKey => {
                eprintln!(
                    "[cmdlog] unknown key [{}] {}. Valid keys: {}",
                    issue.section, issue.key,
                    issue.valid.join(", "),
                );
            }
            IssueKind::UnknownSection => {
                eprintln!(
                    "[cmdlog] unknown section [{}]. Valid sections: {}",
                    issue.key,
                    issue.valid.join(", "),
                );
            }
            IssueKind::InvalidValue => {
                eprintln!(
                    "[cmdlog] invalid [{}] {} = \"{}\". Valid: {}",
                    issue.section, issue.key, issue.value,
                    issue.valid.join(", "),
                );
                if !issue.hint.is_empty() {
                    eprintln!("  {}", issue.hint);
                }
            }
        }
    }

    if issues.iter().any(|i| i.kind.is_hard()) {
        eprintln!("[cmdlog] edit ~/.cmdlog.conf to fix. Run 'cmdlog doctor' again.");
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// compact subcommand — remove waived entries from the log
// ---------------------------------------------------------------------------

/// Mode for the compact subcommand.
enum CompactMode { Summary, DryRun, Force }

fn cmd_compact(mode: CompactMode) {
    let dir = cmdlog_dir();
    let config_path = config_path();
    let default_conf = dir.join("default.conf");
    tui::config::init_config(&config_path, &default_conf);
    let config_state = tui::config::load_config(&config_path);

    let waive_commands = config_state.session.waive_commands;
    let min_cmd_len = config_state.session.waive_min_cmd_len;

    if waive_commands.is_empty() && min_cmd_len == 0 {
        eprintln!("Nothing to compact: [waive] commands is empty and min_cmd_len is 0.");
        return;
    }

    let should_remove = |entry: &log::LogEntry| -> bool {
        cmd::should_waive(&entry.cmd, &waive_commands, min_cmd_len)
    };

    let use_color = is_tty(1);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut on_remove = |line: &str, reason: log::RemoveReason| {
        let (label, color) = match reason {
            log::RemoveReason::Malformed => ("malformed", "31"),
            log::RemoveReason::Waived => ("waived", "33"),
        };
        let display = line;
        if use_color {
            // Show tab characters with reverse-video highlight
            let display = display.replace('\t', "\x1b[7m\\t\x1b[27m");
            let _ = writeln!(out, "  \x1b[{}m[{}]\x1b[0m {}", color, label, display);
        } else {
            let display = display.replace('\t', "\\t");
            let _ = writeln!(out, "  [{}] {}", label, display);
        }
    };

    match mode {
        CompactMode::DryRun => {
            match log::compact_dry_run(should_remove, &mut on_remove) {
                Ok((total, removed)) => {
                    let _ = writeln!(out, "{} of {} entries would be removed.", removed, total);
                }
                Err(e) => {
                    eprintln!("[cmdlog] {}", e);
                    process::exit(1);
                }
            }
        }
        CompactMode::Summary => {
            match log::compact_dry_run(should_remove, |_, _| {}) {
                Ok((total, removed)) => {
                    if removed == 0 {
                        let _ = writeln!(out, "No waived entries found ({} entries).", total);
                    } else {
                        let _ = writeln!(out, "{} of {} entries can be removed. Use -n to list, -f to remove.", removed, total);
                    }
                }
                Err(e) => {
                    eprintln!("[cmdlog] {}", e);
                    process::exit(1);
                }
            }
        }
        CompactMode::Force => {
            match log::compact_log(should_remove, &mut on_remove) {
                Ok((total, removed)) => {
                    if removed == 0 {
                        let _ = writeln!(out, "No waived entries found. Log unchanged ({} entries).", total);
                    } else {
                        let _ = writeln!(out, "Removed {} of {} entries. {} remaining.", removed, total, total - removed);
                    }
                }
                Err(e) => {
                    eprintln!("[cmdlog] {}", e);
                    process::exit(1);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn print_usage() {
    println!(
        "cmdlog — Cross-shell command logger

Usage:
  cmdlog list [options]               Query the log (TUI or linear)
  cmdlog install [-f] <shell>         Install hook in rc file
         aliases: add, i
         -f, --force: remove and reinstall
  cmdlog uninstall <shell>            Remove hook from rc file
         aliases: unlink, remove, rm, r, un
  cmdlog compact [-n | -f]             Remove waived/malformed entries from the log
                                        (no flag: summary, -n: list each, -f: remove)
  cmdlog config <key>                 Query a config value (e.g. inject.bash)
  cmdlog doctor [shell]               Check and repair config
  cmdlog hook <shell>                 Print shell hook source (for eval)
  cmdlog help                         Show this help

Internal (called by shell hooks):
  cmdlog record <shell> <pwd> <exit_code> <cmd>   Record a command to the log
  cmdlog inject <shell> <text>        Push text into terminal via TIOCSTI

Shells: bash, zsh, tcsh
Run 'cmdlog list --help' for query options."
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "record" => {
            if args.len() < 6 {
                eprintln!("Usage: cmdlog record <shell> <pwd> <exit_code> <cmd>");
                process::exit(1);
            }
            // If more than 6 args, join the rest as the command
            // (handles cases where the shell didn't quote properly)
            let cmd = if args.len() == 6 {
                args[5].clone()
            } else {
                args[5..].join(" ")
            };
            cmd_record(&args[2], &args[3], &args[4], &cmd);
        }
        "list" => {
            cmd_list(&args[2..]);
        }
        "install" | "add" | "i" | "in" | "ins" | "inst" | "insta" | "instal"
        | "isnt" | "isnta" | "isntal" | "isntall" => {
            let rest = &args[2..];
            let force = rest.iter().any(|a| a == "-f" || a == "--force");
            let shell_args: Vec<&str> = rest.iter()
                .filter(|a| !a.starts_with('-'))
                .map(|s| s.as_str())
                .collect();
            if shell_args.is_empty() {
                eprintln!("Usage: cmdlog install [-f] <bash|zsh|tcsh>");
                process::exit(1);
            }
            cmd_install(shell_args[0], force);
        }
        "uninstall" | "unlink" | "remove" | "rm" | "r" | "un" => {
            if args.len() < 3 {
                eprintln!("Usage: cmdlog uninstall <bash|zsh|tcsh>");
                process::exit(1);
            }
            cmd_uninstall(&args[2]);
        }
        #[cfg(unix)]
        "inject" => {
            if args.len() < 4 {
                eprintln!("Usage: cmdlog inject <shell> <text>");
                process::exit(1);
            }
            cmd_inject(&args[2], &args[3]);
        }
        "config" => {
            if args.len() < 3 {
                eprintln!("Usage: cmdlog config <key>");
                process::exit(1);
            }
            cmd_config(&args[2]);
        }
        "doctor" => {
            let shell = if args.len() >= 3 {
                args[2].clone()
            } else {
                detect_shell().unwrap_or_else(|| {
                        eprintln!("Usage: cmdlog doctor [bash|zsh|tcsh]");
                        eprintln!("Could not detect shell. Specify it explicitly.");
                        process::exit(1);
                    })
            };
            cmd_doctor(&shell);
        }
        "hook" => {
            // Accept both positional ("cmdlog hook bash") and flag form
            // ("cmdlog hook --bash") for ergonomic parity with fzf-style usage.
            if args.len() < 3 {
                eprintln!("Usage: cmdlog hook <bash|zsh|tcsh>");
                process::exit(1);
            }
            let shell = args[2].strip_prefix("--").unwrap_or(&args[2]);
            cmd_hook(shell);
        }
        "compact" => {
            let mut mode = CompactMode::Summary;
            for arg in &args[2..] {
                match arg.as_str() {
                    "--dry-run" | "-n" => mode = CompactMode::DryRun,
                    "--force" | "-f" => mode = CompactMode::Force,
                    other => {
                        eprintln!("Unknown option: {}", other);
                        eprintln!("Usage: cmdlog compact [-n | -f]");
                        process::exit(1);
                    }
                }
            }
            cmd_compact(mode);
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        "--version" | "-v" | "-V" => {
            println!("cmdlog {}", env!("CARGO_PKG_VERSION"));
        }
        other => {
            eprintln!("Unknown command: {}", other);
            print_usage();
            process::exit(1);
        }
    }
}

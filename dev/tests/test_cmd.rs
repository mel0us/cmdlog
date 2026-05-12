use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use cmdlog::cmd::{is_builtin, record};

/// Tests that call `record()` use `std::env::set_var("CMDLOG_FILE", ...)` which
/// mutates the process-global environment. Running them in parallel causes races.
/// This mutex serializes access to the env var.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cmdlog_test_cmd_{}_{}", std::process::id(), suffix
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

fn log_lines(dir: &PathBuf) -> Vec<String> {
    let log = dir.join(".cmdlog.tsv");
    if !log.exists() {
        return vec![];
    }
    fs::read_to_string(&log)
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// is_builtin
// ---------------------------------------------------------------------------

#[test]
fn builtin_known_builtins() {
    assert!(is_builtin("cd"));
    assert!(is_builtin("echo"));
    assert!(is_builtin("export"));
    assert!(is_builtin("set"));
    assert!(is_builtin("local"));
    assert!(is_builtin("alias"));
    assert!(is_builtin("source"));
    assert!(is_builtin("return"));
    assert!(is_builtin("typeset"));
    assert!(is_builtin("pwd"));
    assert!(is_builtin("exit"));
    assert!(is_builtin("eval"));
}

#[test]
fn builtin_tcsh_specific() {
    assert!(is_builtin("bindkey"));
    assert!(is_builtin("breaksw"));
    assert!(is_builtin("filetest"));
    assert!(is_builtin("foreach"));
    assert!(is_builtin("goto"));
    assert!(is_builtin("onintr"));
    assert!(is_builtin("setenv"));
    assert!(is_builtin("unsetenv"));
}

#[test]
fn builtin_not_builtin() {
    assert!(!is_builtin("git"));
    assert!(!is_builtin("python3"));
    assert!(!is_builtin("make"));
    assert!(!is_builtin("ssh"));
    assert!(!is_builtin("cargo"));
    assert!(!is_builtin("docker"));
    assert!(!is_builtin(""));
}

// ---------------------------------------------------------------------------
// record
// ---------------------------------------------------------------------------

#[test]
fn record_logs_external_command() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_external");

    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "git status");
    let lines = log_lines(&dir);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("\tbash\t/home/user\t0\tgit status"));

    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_skips_empty_command() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_empty");

    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "");
    record("bash", "/home/user", "0", "   ");
    assert!(log_lines(&dir).is_empty());

    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_skips_builtin() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_builtin");

    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "cd /tmp");
    record("bash", "/home/user", "0", "echo hello");
    record("bash", "/home/user", "0", "export FOO=bar");
    assert!(log_lines(&dir).is_empty());

    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_logs_formerly_waived() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_waived");
    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "ls -la");
    record("bash", "/home/user", "0", "grep -rn TODO .");
    record("bash", "/home/user", "0", "cat file.txt");
    assert_eq!(log_lines(&dir).len(), 3);
    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_pipe_override() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_pipe");
    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "echo hello | tr a b");
    record("bash", "/home/user", "0", "cat f | grep x");
    record("bash", "/home/user", "0", "ls -la | head");
    assert_eq!(log_lines(&dir).len(), 3);
    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_no_longer_deduplicates() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_no_dedup");
    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));

    record("bash", "/home/user", "0", "git status");
    record("bash", "/home/user", "0", "git status");
    record("bash", "/home/user", "0", "git status");

    std::env::remove_var("CMDLOG_FILE");
    // All 3 are written — dedup is now at load time
    assert_eq!(log_lines(&dir).len(), 3);

    cleanup(&dir);
}

#[test]
fn record_tsv_format() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_tsv");

    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "git push origin main");
    let lines = log_lines(&dir);
    assert_eq!(lines.len(), 1);

    let parts: Vec<&str> = lines[0].splitn(5, '\t').collect();
    assert_eq!(parts.len(), 5);
    // parts[0] = timestamp
    assert!(parts[0].contains('T'));
    assert_eq!(parts[1], "bash");
    assert_eq!(parts[2], "/home/user");
    assert_eq!(parts[3], "0");
    assert_eq!(parts[4], "git push origin main");

    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}

#[test]
fn record_multiple_shells() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rec_shells");

    std::env::set_var("CMDLOG_FILE", dir.join(".cmdlog.tsv"));
    record("bash", "/home/user", "0", "git status");
    record("zsh", "/home/user", "0", "python3 foo.py");
    record("tcsh", "/home/user", "0", "ssh server");

    let lines = log_lines(&dir);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\tbash\t"));
    assert!(lines[1].contains("\tzsh\t"));
    assert!(lines[2].contains("\ttcsh\t"));

    std::env::remove_var("CMDLOG_FILE");
    cleanup(&dir);
}


use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use cmdlog::log::{compact_dry_run, compact_log, load_entries, log_path, rewrite_excluding, LogEntry};
use std::collections::HashSet;

/// Serializes tests that mutate CMDLOG_FILE env var to prevent races.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cmdlog_test_log_{}_{}", std::process::id(), suffix
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn log_path_default_is_home_raw_log() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::remove_var("CMDLOG_FILE");
    let home = std::env::var("HOME").unwrap();
    assert_eq!(log_path(), PathBuf::from(format!("{}/.cmdlog.tsv", home)));
}

#[test]
fn log_path_respects_cmdlog_file_env() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::set_var("CMDLOG_FILE", "/tmp/my_custom.log");
    let result = log_path();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(result, PathBuf::from("/tmp/my_custom.log"));
}

#[test]
fn log_path_ignores_empty_cmdlog_file() {
    let _lock = ENV_LOCK.lock().unwrap();
    std::env::set_var("CMDLOG_FILE", "");
    let home = std::env::var("HOME").unwrap();
    let result = log_path();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(result, PathBuf::from(format!("{}/.cmdlog.tsv", home)));
}

#[test]
fn load_entries_valid_tsv() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("valid");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:05:00\tzsh\t/tmp\t0\tmake -j8\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].date, "2026-04-06T10:00:00");
    assert_eq!(entries[0].shell, "bash");
    assert_eq!(entries[0].pwd, "/home/user");
    assert_eq!(entries[0].exit_code, "0");
    assert_eq!(entries[0].cmd, "git status");
    assert_eq!(entries[1].shell, "zsh");
    assert_eq!(entries[1].cmd, "make -j8");

    cleanup(&dir);
}

#[test]
fn load_entries_skips_malformed_lines() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("malformed");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         bad line without tabs\n\
         only\ttwo\ttabs\n\
         2026-04-06T10:00:30\tbash\t/home/user\told four field line\n\
         2026-04-06T11:00:00\tzsh\t/tmp\t0\tpython3 foo.py\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].cmd, "git status");
    assert_eq!(entries[1].cmd, "python3 foo.py");

    cleanup(&dir);
}

#[test]
fn load_entries_empty_file() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("empty");
    let log = dir.join(".cmdlog.tsv");
    fs::write(&log, "").unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert!(entries.is_empty());

    cleanup(&dir);
}

#[test]
fn load_entries_missing_file() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("missing");
    // Don't create .cmdlog.tsv
    std::env::set_var("CMDLOG_FILE", dir.join("nonexistent.log"));
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert!(entries.is_empty());
    cleanup(&dir);
}

#[test]
fn load_entries_tabs_in_command() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("tabs");
    let log = dir.join(".cmdlog.tsv");
    // splitn(5, '\t') means the 5th field captures everything after the 4th tab,
    // including embedded tabs in the command.
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tawk '{print $1}'\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].cmd, "awk '{print $1}'");

    cleanup(&dir);
}

#[test]
fn load_entries_preserves_order() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("order");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/a\t0\tcmd1\n\
         2026-04-05T09:00:00\tzsh\t/b\t0\tcmd2\n\
         2026-04-07T11:00:00\ttcsh\t/c\t0\tcmd3\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].cmd, "cmd1");
    assert_eq!(entries[1].cmd, "cmd2");
    assert_eq!(entries[2].cmd, "cmd3");

    cleanup(&dir);
}

#[test]
fn load_entries_dedup_consecutive_cmds() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("dedup_consec");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:00:01\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:00:02\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:01:00\tzsh\t/tmp\t0\tmake -j8\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].cmd, "git status");
    assert_eq!(entries[1].cmd, "make -j8");

    cleanup(&dir);
}

#[test]
fn load_entries_dedup_keeps_non_consecutive() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("dedup_noncon");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:01:00\tbash\t/home/user\t0\tmake -j8\n\
         2026-04-06T10:02:00\tbash\t/home/user\t0\tgit status\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");

    assert_eq!(entries.len(), 3);

    cleanup(&dir);
}

#[test]
fn load_entries_dedup_by_cmd_field_only() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("dedup_cmd_only");
    let log = dir.join(".cmdlog.tsv");
    // Same cmd but different shell/pwd — still deduped (cmd-only match)
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:00:01\tzsh\t/tmp\t0\tgit status\n\
         2026-04-06T10:01:00\tbash\t/home/user\t0\tmake\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].cmd, "git status");
    assert_eq!(entries[1].cmd, "make");

    cleanup(&dir);
}

#[test]
fn load_entries_trailing_newline() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("trailing");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home\t0\tgit push\n\n\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 1);

    cleanup(&dir);
}

#[test]
fn load_entries_includes_exit_code() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("exit_code");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:01:00\tbash\t/home/user\t1\tgit push\n\
         2026-04-06T10:02:00\tzsh\t/tmp\t127\tnonexistent_cmd\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let entries = load_entries();
    std::env::remove_var("CMDLOG_FILE");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].exit_code, "0");
    assert_eq!(entries[1].exit_code, "1");
    assert_eq!(entries[2].exit_code, "127");

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// rewrite_excluding
// ---------------------------------------------------------------------------

#[test]
fn rewrite_excluding_single_entry() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rw_single");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:05:00\tzsh\t/tmp\t0\tmake -j8\n\
         2026-04-06T10:10:00\tbash\t/home/user\t0\tgit push\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let deleted: HashSet<String> = ["2026-04-06T10:05:00".to_string()].into();
    let result = rewrite_excluding(&deleted);
    std::env::remove_var("CMDLOG_FILE");

    assert!(result.is_ok());
    let content = fs::read_to_string(&log).unwrap();
    assert_eq!(content.lines().count(), 2);
    assert!(!content.contains("make -j8"));
    assert!(content.contains("git status"));
    assert!(content.contains("git push"));

    cleanup(&dir);
}

#[test]
fn rewrite_excluding_multiple_entries() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rw_multi");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:05:00\tzsh\t/tmp\t0\tgit status\n\
         2026-04-06T10:10:00\tbash\t/home/user\t0\tgit push\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let deleted: HashSet<String> = [
        "2026-04-06T10:00:00".to_string(),
        "2026-04-06T10:05:00".to_string(),
    ].into();
    let result = rewrite_excluding(&deleted);
    std::env::remove_var("CMDLOG_FILE");

    assert!(result.is_ok());
    let content = fs::read_to_string(&log).unwrap();
    assert_eq!(content.lines().count(), 1);
    assert!(content.contains("git push"));

    cleanup(&dir);
}

#[test]
fn rewrite_excluding_no_match_is_noop() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rw_noop");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let deleted: HashSet<String> = ["2099-01-01T00:00:00".to_string()].into();
    let result = rewrite_excluding(&deleted);
    std::env::remove_var("CMDLOG_FILE");

    assert!(result.is_ok());
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("git status"));

    cleanup(&dir);
}

#[test]
fn rewrite_excluding_preserves_trailing_newline() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("rw_trailing");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:05:00\tzsh\t/tmp\t0\tmake -j8\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let deleted: HashSet<String> = ["2026-04-06T10:00:00".to_string()].into();
    let result = rewrite_excluding(&deleted);
    std::env::remove_var("CMDLOG_FILE");

    assert!(result.is_ok());
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.ends_with('\n'));

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// compact_log / compact_dry_run
// ---------------------------------------------------------------------------

#[test]
fn compact_log_removes_waived_entries() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("compact_waive");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
         2026-04-06T10:01:00\tbash\t/home/user\t0\tls -la\n\
         2026-04-06T10:02:00\tbash\t/home/user\t0\tls | grep foo\n\
         2026-04-06T10:03:00\tbash\t/home/user\t0\tmake\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let result = compact_log(|e| {
        let has_special = cmdlog::cmd::has_special_chars(&e.cmd);
        if has_special { return false; }
        let first = e.cmd.split_whitespace().next().unwrap_or("");
        first == "ls"
    }, |_, _: cmdlog::log::RemoveReason| {});
    std::env::remove_var("CMDLOG_FILE");

    let (total, removed) = result.unwrap();
    assert_eq!(total, 4);
    assert_eq!(removed, 1); // "ls -la" removed, "ls | grep foo" kept (special)
    let content = fs::read_to_string(&log).unwrap();
    assert_eq!(content.lines().count(), 3);
    assert!(!content.contains("ls -la"));
    assert!(content.contains("ls | grep foo"));

    cleanup(&dir);
}

#[test]
fn compact_dry_run_does_not_modify_file() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("compact_dry");
    let log = dir.join(".cmdlog.tsv");
    let original = "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n\
                     2026-04-06T10:01:00\tbash\t/home/user\t0\tls -la\n";
    fs::write(&log, original).unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let result = compact_dry_run(|e| {
        e.cmd.split_whitespace().next().unwrap_or("") == "ls"
    }, |_, _: cmdlog::log::RemoveReason| {});
    std::env::remove_var("CMDLOG_FILE");

    let (total, removed) = result.unwrap();
    assert_eq!(total, 2);
    assert_eq!(removed, 1);
    // File unchanged
    let content = fs::read_to_string(&log).unwrap();
    assert!(content.contains("ls -la"));

    cleanup(&dir);
}

#[test]
fn compact_log_noop_when_nothing_matches() {
    let _lock = ENV_LOCK.lock().unwrap();
    let dir = tmp_dir("compact_noop");
    let log = dir.join(".cmdlog.tsv");
    fs::write(
        &log,
        "2026-04-06T10:00:00\tbash\t/home/user\t0\tgit status\n",
    )
    .unwrap();

    std::env::set_var("CMDLOG_FILE", &log);
    let result = compact_log(|_| false, |_, _: cmdlog::log::RemoveReason| {});
    std::env::remove_var("CMDLOG_FILE");

    let (total, removed) = result.unwrap();
    assert_eq!(total, 1);
    assert_eq!(removed, 0);

    cleanup(&dir);
}

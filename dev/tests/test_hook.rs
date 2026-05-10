use std::fs;
use std::path::PathBuf;

use cmdlog::hook::{find_rc_file, hook_source, install_hook, uninstall_hook};

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cmdlog_test_hook_{}_{}", std::process::id(), suffix
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// find_rc_file
// ---------------------------------------------------------------------------

#[test]
fn find_rc_file_returns_first_existing() {
    let dir = tmp_dir("rc_first");
    let rc1 = dir.join("rc_a");
    let rc2 = dir.join("rc_b");
    fs::write(&rc2, "# bashrc\n").unwrap();
    let result = find_rc_file(&[rc1.clone(), rc2.clone()]);
    assert_eq!(result, Some(rc2));
    cleanup(&dir);
}

#[test]
fn find_rc_file_prefers_first() {
    let dir = tmp_dir("rc_prefer");
    let rc1 = dir.join("rc_a");
    let rc2 = dir.join("rc_b");
    fs::write(&rc1, "# custom\n").unwrap();
    fs::write(&rc2, "# bashrc\n").unwrap();
    let result = find_rc_file(&[rc1.clone(), rc2.clone()]);
    assert_eq!(result, Some(rc1));
    cleanup(&dir);
}

#[test]
fn find_rc_file_none_exist() {
    let dir = tmp_dir("rc_none");
    let rc1 = dir.join("rc_a");
    let rc2 = dir.join("rc_b");
    let result = find_rc_file(&[rc1, rc2]);
    assert_eq!(result, None);
    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// install_hook
// ---------------------------------------------------------------------------

#[test]
fn install_hook_appends_guarded_block() {
    let dir = tmp_dir("install_basic");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "# existing content\n").unwrap();
    let hook_path = dir.join("hook/cmdlog.bash");
    fs::create_dir_all(dir.join("hook")).unwrap();
    fs::write(&hook_path, "# hook\n").unwrap();

    let result = install_hook(&rc, &hook_path);
    assert!(result.is_ok());

    let content = fs::read_to_string(&rc).unwrap();
    assert!(content.contains("# existing content"));
    assert!(content.contains("# >>> cmdlog >>>"));
    assert!(content.contains("source "));
    assert!(content.contains("hook/cmdlog.bash"));
    assert!(content.contains("# <<< cmdlog <<<"));

    cleanup(&dir);
}

#[test]
fn install_hook_fails_if_guard_present() {
    let dir = tmp_dir("install_guard");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "# stuff\n# >>> cmdlog >>>\nsource foo\n# <<< cmdlog <<<\n").unwrap();
    let hook_path = dir.join("hook/cmdlog.bash");
    fs::create_dir_all(dir.join("hook")).unwrap();
    fs::write(&hook_path, "# hook\n").unwrap();

    let result = install_hook(&rc, &hook_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already present"));

    cleanup(&dir);
}

#[test]
fn install_hook_fails_if_manual_source_present() {
    let dir = tmp_dir("install_manual");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "source /some/path/hook/cmdlog.bash\n").unwrap();
    let hook_path = dir.join("hook/cmdlog.bash");
    fs::create_dir_all(dir.join("hook")).unwrap();
    fs::write(&hook_path, "# hook\n").unwrap();

    let result = install_hook(&rc, &hook_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already present"));

    cleanup(&dir);
}

#[test]
fn install_hook_tcsh_uses_csh_syntax() {
    let dir = tmp_dir("install_tcsh");
    let rc = dir.join(".tcshrc");
    fs::write(&rc, "# tcshrc\n").unwrap();
    let hook_path = dir.join("hook/cmdlog.tcsh");
    fs::create_dir_all(dir.join("hook")).unwrap();
    fs::write(&hook_path, "# hook\n").unwrap();

    let result = install_hook(&rc, &hook_path);
    assert!(result.is_ok());

    let content = fs::read_to_string(&rc).unwrap();
    assert!(content.contains("\nsource "));
    assert!(content.contains("alias cl 'cmdlog list'"));
    assert!(content.contains("setenv CMDLOG_TZ"));
    assert!(!content.contains("export CMDLOG_TZ"));

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// uninstall_hook
// ---------------------------------------------------------------------------

#[test]
fn uninstall_hook_removes_guarded_block() {
    let dir = tmp_dir("uninstall_basic");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "# before\n# >>> cmdlog >>>\nsource /path/hook/cmdlog.bash\n# <<< cmdlog <<<\n# after\n").unwrap();

    let result = uninstall_hook(&rc);
    assert!(result.is_ok());

    let content = fs::read_to_string(&rc).unwrap();
    assert!(content.contains("# before"));
    assert!(content.contains("# after"));
    assert!(!content.contains("# >>> cmdlog >>>"));
    assert!(!content.contains("# <<< cmdlog <<<"));
    assert!(!content.contains("source"));

    cleanup(&dir);
}

#[test]
fn uninstall_hook_fails_if_no_guard() {
    let dir = tmp_dir("uninstall_noguard");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "# just normal content\n").unwrap();

    let result = uninstall_hook(&rc);
    assert!(result.is_err());

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// hook_source — embedded hook contents
// ---------------------------------------------------------------------------

#[test]
fn hook_source_bash_returns_bash_hook() {
    let src = hook_source("bash").expect("bash hook missing");
    assert!(src.contains("__cmdlog_record"), "bash hook should define __cmdlog_record");
    assert!(src.contains("PROMPT_COMMAND"), "bash hook should integrate with PROMPT_COMMAND");
}

#[test]
fn hook_source_zsh_returns_zsh_hook() {
    let src = hook_source("zsh").expect("zsh hook missing");
    assert!(src.contains("__cmdlog_record"), "zsh hook should define __cmdlog_record");
    assert!(src.contains("precmd_functions"), "zsh hook should integrate with precmd_functions");
}

#[test]
fn hook_source_tcsh_returns_tcsh_hook() {
    let src = hook_source("tcsh").expect("tcsh hook missing");
    assert!(src.contains("__cmdlog_do_record"), "tcsh hook should define __cmdlog_do_record alias");
    assert!(src.contains("precmd"), "tcsh hook should chain into precmd");
}

#[test]
fn hook_source_unknown_shell_returns_none() {
    assert!(hook_source("fish").is_none());
    assert!(hook_source("").is_none());
    assert!(hook_source("BASH").is_none(), "case-sensitive");
}

#[test]
fn hook_source_matches_disk_files() {
    // Embedded content must match the on-disk hook files byte-for-byte —
    // single source of truth for both `source ~/.local/share/cmdlog/hook/*`
    // and `eval "$(cmdlog hook *)"` install styles.
    let manifest = env!("CARGO_MANIFEST_DIR");
    for shell in &["bash", "zsh", "tcsh"] {
        let path = format!("{}/../hook/cmdlog.{}", manifest, shell);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {}", path, e));
        let embedded = hook_source(shell).unwrap();
        assert_eq!(embedded, on_disk, "{} hook drift between embedded and on-disk", shell);
    }
}

#[test]
fn uninstall_hook_preserves_surrounding_content() {
    let dir = tmp_dir("uninstall_preserve");
    let rc = dir.join(".bashrc");
    fs::write(&rc, "line1\nline2\n# >>> cmdlog >>>\nsource foo\n# <<< cmdlog <<<\nline3\nline4\n").unwrap();

    let result = uninstall_hook(&rc);
    assert!(result.is_ok());

    let content = fs::read_to_string(&rc).unwrap();
    assert_eq!(content, "line1\nline2\nline3\nline4\n");

    cleanup(&dir);
}

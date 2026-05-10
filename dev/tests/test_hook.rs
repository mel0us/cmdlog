use std::fs;
use std::path::PathBuf;

use cmdlog::hook::{find_rc_file, install_hook, uninstall_hook};

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

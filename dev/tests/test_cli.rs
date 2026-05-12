//! Integration tests for the cmdlog binary CLI interface.

use std::path::PathBuf;
use std::process::Command;

fn binary_path() -> PathBuf {
    // Use the debug binary built by cargo test
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove 'deps'
    path.push("cmdlog");
    path
}

// ---------------------------------------------------------------------------
// Help
// ---------------------------------------------------------------------------

#[test]
fn cli_help_exits_zero() {
    let output = Command::new(binary_path())
        .arg("help")
        .output()
        .expect("failed to run cmdlog help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cmdlog"));
}

#[test]
fn cli_help_flag() {
    let output = Command::new(binary_path())
        .arg("--help")
        .output()
        .expect("failed to run cmdlog --help");
    assert!(output.status.success());
}

#[test]
fn cli_no_args_requires_tty() {
    // Bare `cmdlog` launches the TUI, which requires a TTY on stderr.
    // Without one (as in this test harness) it must exit non-zero with
    // a clear message rather than blowing up trying to draw on a pipe.
    let output = Command::new(binary_path())
        .output()
        .expect("failed to run cmdlog");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("TTY"), "missing TTY hint: {}", stderr);
}

#[test]
fn cli_unknown_command_exits_nonzero() {
    let output = Command::new(binary_path())
        .arg("foobar")
        .output()
        .expect("failed to run cmdlog foobar");
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// Record (via binary)
// ---------------------------------------------------------------------------

#[test]
fn cli_record_missing_args() {
    let output = Command::new(binary_path())
        .args(["record", "bash", "/tmp"])
        .output()
        .expect("failed to run cmdlog record");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: cmdlog record <shell> <pwd> <exit_code> <cmd>"));
}

// ---------------------------------------------------------------------------
// Install / Uninstall
// ---------------------------------------------------------------------------

#[test]
fn cli_install_missing_shell_arg() {
    let output = Command::new(binary_path())
        .arg("install")
        .output()
        .expect("failed to run cmdlog install");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: cmdlog install"));
}

#[test]
fn cli_uninstall_missing_shell_arg() {
    let output = Command::new(binary_path())
        .arg("uninstall")
        .output()
        .expect("failed to run cmdlog uninstall");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: cmdlog uninstall"));
}

#[test]
fn cli_install_unknown_shell() {
    let output = Command::new(binary_path())
        .args(["install", "fish"])
        .output()
        .expect("failed to run cmdlog install fish");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown shell"));
}

// ---------------------------------------------------------------------------
// Hook (print embedded shell hook source)
// ---------------------------------------------------------------------------

#[test]
fn cli_hook_bash_prints_bash_hook() {
    let output = Command::new(binary_path())
        .args(["hook", "bash"])
        .output()
        .expect("failed to run cmdlog hook bash");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("__cmdlog_record"));
    assert!(stdout.contains("PROMPT_COMMAND"));
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_hook_zsh_prints_zsh_hook() {
    let output = Command::new(binary_path())
        .args(["hook", "zsh"])
        .output()
        .expect("failed to run cmdlog hook zsh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("precmd_functions"));
}

#[test]
fn cli_hook_tcsh_prints_tcsh_hook() {
    let output = Command::new(binary_path())
        .args(["hook", "tcsh"])
        .output()
        .expect("failed to run cmdlog hook tcsh");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("__cmdlog_do_record"));
}

#[test]
fn cli_hook_flag_form_accepted() {
    // `cmdlog hook --bash` should equal `cmdlog hook bash`.
    let positional = Command::new(binary_path())
        .args(["hook", "bash"])
        .output()
        .unwrap();
    let flag = Command::new(binary_path())
        .args(["hook", "--bash"])
        .output()
        .unwrap();
    assert!(flag.status.success());
    assert_eq!(positional.stdout, flag.stdout);
}

#[test]
fn cli_hook_unknown_shell_exits_nonzero() {
    let output = Command::new(binary_path())
        .args(["hook", "fish"])
        .output()
        .expect("failed to run cmdlog hook fish");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown shell"));
}

#[test]
fn cli_hook_missing_shell_arg() {
    let output = Command::new(binary_path())
        .arg("hook")
        .output()
        .expect("failed to run cmdlog hook");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: cmdlog hook"));
}

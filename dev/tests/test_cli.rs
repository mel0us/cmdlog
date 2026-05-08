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
fn cli_no_args_exits_nonzero() {
    let output = Command::new(binary_path())
        .output()
        .expect("failed to run cmdlog");
    assert!(!output.status.success());
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
// List
// ---------------------------------------------------------------------------

#[test]
fn cli_list_help() {
    let output = Command::new(binary_path())
        .args(["list", "--help"])
        .output()
        .expect("failed to run cmdlog list --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--no-tui"));
    assert!(stdout.contains("--no-color"));
    assert!(stdout.contains("--search"));
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

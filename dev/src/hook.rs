//! Shell hook install/uninstall logic.
//!
//! Manages guarded source blocks in user rc files.

use std::fs;
use std::path::{Path, PathBuf};

pub const GUARD_BEGIN: &str = "# >>> cmdlog >>>";
const GUARD_END: &str = "# <<< cmdlog <<<";

/// Find the first existing file from a list of candidate paths.
pub fn find_rc_file(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Return the rc file candidates for a given shell, rooted at `home`.
pub fn rc_candidates(shell: &str, home: &Path) -> Vec<PathBuf> {
    match shell {
        "bash" => vec![home.join(".bashrc")],
        "zsh"  => vec![home.join(".zshrc")],
        "tcsh" => vec![home.join(".tcshrc"), home.join(".cshrc")],
        _ => vec![],
    }
}

/// Hook source files embedded at build time. Single source of truth: the
/// on-disk `hook/cmdlog.<shell>` files used by `source`-style installs are
/// the same bytes the `cmdlog hook <shell>` subcommand emits.
const HOOK_BASH: &str = include_str!("../../hook/cmdlog.bash");
const HOOK_ZSH:  &str = include_str!("../../hook/cmdlog.zsh");
const HOOK_TCSH: &str = include_str!("../../hook/cmdlog.tcsh");

/// Return the embedded hook source for a shell.
pub fn hook_source(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" => Some(HOOK_BASH),
        "zsh"  => Some(HOOK_ZSH),
        "tcsh" => Some(HOOK_TCSH),
        _ => None,
    }
}

/// Canonical on-disk path for the tcsh hook (extracted from embedded source
/// at install time). bash/zsh don't need a file since `eval "$(cmdlog hook
/// <shell>)"` reads embedded bytes directly.
pub fn tcsh_hook_path(home: &Path) -> PathBuf {
    home.join(".local/share/cmdlog/hook/cmdlog.tcsh")
}

/// Write the embedded tcsh hook source to its canonical install path.
pub fn write_tcsh_hook(home: &Path) -> Result<PathBuf, String> {
    let path = tcsh_hook_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {}", parent.display(), e))?;
    }
    let src = hook_source("tcsh").expect("tcsh hook always embedded");
    fs::write(&path, src)
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))?;
    Ok(path)
}

/// Build the rc-file integration line for a shell. bash/zsh use eval over
/// `cmdlog hook`, tcsh sources the extracted file (backtick command
/// substitution collapses newlines, defeating eval).
fn integration_line(shell: &str, home: &Path) -> String {
    let bin = home.join(".local/bin/cmdlog");
    match shell {
        "bash" | "zsh" => format!("eval \"$({} hook {})\"", bin.display(), shell),
        "tcsh" => format!("source {}", tcsh_hook_path(home).display()),
        _ => String::new(),
    }
}

/// Install a hook into an rc file. Appends a guarded block that wires the
/// embedded hook into the shell — eval-based for bash/zsh, source-based for
/// tcsh (see `integration_line`).
pub fn install_hook(rc_path: &Path, shell: &str, home: &Path) -> Result<(), String> {
    let content = fs::read_to_string(rc_path)
        .map_err(|e| format!("Cannot read {}: {}", rc_path.display(), e))?;

    if content.contains(GUARD_BEGIN) {
        return Err(format!(
            "cmdlog hook already present in {}",
            rc_path.display()
        ));
    }

    // Reject any pre-existing manual install (eval form, source form, or
    // legacy path under hook/).
    let markers = [
        format!("cmdlog hook {}", shell),
        format!("cmdlog.{}", shell),
    ];
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if markers.iter().any(|m| trimmed.contains(m.as_str())) {
            return Err(format!(
                "cmdlog hook already present in {}",
                rc_path.display()
            ));
        }
    }

    let is_csh = shell == "tcsh";
    let (alias_line, tz_line) = if is_csh {
        ("alias cl 'cmdlog list'", "setenv CMDLOG_TZ +8")
    } else {
        ("alias cl='cmdlog list'", "export CMDLOG_TZ=+8")
    };

    let block = format!(
        "\n{}\n{}\n{}\n{}\n{}\n",
        GUARD_BEGIN,
        integration_line(shell, home),
        alias_line,
        tz_line,
        GUARD_END,
    );

    fs::write(rc_path, content + &block)
        .map_err(|e| format!("Cannot write {}: {}", rc_path.display(), e))?;

    Ok(())
}

/// Remove the guarded cmdlog block from an rc file.
pub fn uninstall_hook(rc_path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(rc_path)
        .map_err(|e| format!("Cannot read {}: {}", rc_path.display(), e))?;

    if !content.contains(GUARD_BEGIN) {
        return Err(format!(
            "cmdlog hook not found in {}.",
            rc_path.display()
        ));
    }

    let mut result = String::new();
    let mut inside_guard = false;
    for line in content.lines() {
        if line.trim() == GUARD_BEGIN {
            inside_guard = true;
            continue;
        }
        if line.trim() == GUARD_END {
            inside_guard = false;
            continue;
        }
        if !inside_guard {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing blank line left by the guard block's leading newline
    while result.ends_with("\n\n") {
        result.pop();
    }

    fs::write(rc_path, &result)
        .map_err(|e| format!("Cannot write {}: {}", rc_path.display(), e))?;

    Ok(())
}

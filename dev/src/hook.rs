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
        "bash" => vec![
            home.join(".bashrc_custom"),
            home.join(".bashrc"),
        ],
        "zsh" => vec![
            home.join(".zshrc_custom"),
            home.join(".zshrc"),
        ],
        "tcsh" => vec![
            home.join(".tcshrc_custom"),
            home.join(".tcshrc"),
            home.join(".cshrc_custom"),
            home.join(".cshrc"),
        ],
        _ => vec![],
    }
}

/// Return the hook filename for a given shell.
pub fn hook_filename(shell: &str) -> &'static str {
    match shell {
        "bash" => "cmdlog.bash",
        "zsh" => "cmdlog.zsh",
        "tcsh" => "cmdlog.tcsh",
        _ => "",
    }
}

/// Install a hook into an rc file. Appends a guarded source block.
/// `cmdlog_dir` is only needed for tcsh (to set `__cmdlog_dir`).
pub fn install_hook(
    rc_path: &Path,
    hook_path: &Path,
    cmdlog_dir: Option<&Path>,
) -> Result<(), String> {
    let content = fs::read_to_string(rc_path)
        .map_err(|e| format!("Cannot read {}: {}", rc_path.display(), e))?;

    // Check for existing guarded block
    if content.contains(GUARD_BEGIN) {
        return Err(format!(
            "cmdlog hook already present in {}.",
            rc_path.display()
        ));
    }

    // Check for manual source line containing hook filename
    let hook_name = hook_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let hook_marker = format!("hook/{}", hook_name);
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains(&hook_marker) && !trimmed.starts_with('#') {
            return Err(format!(
                "cmdlog hook already present in {}.",
                rc_path.display()
            ));
        }
    }

    // Build the source line
    let hook_abs = hook_path.to_string_lossy();
    let source_line = if let Some(dir) = cmdlog_dir {
        // tcsh/csh: set __cmdlog_dir before source
        format!(
            "set __cmdlog_dir = {}\nsource {}",
            dir.to_string_lossy(),
            hook_abs
        )
    } else {
        format!("source {}", hook_abs)
    };

    // Alias and env var: tcsh/csh vs bash/zsh syntax
    let (alias_line, tz_line) = if cmdlog_dir.is_some() {
        ("alias cl 'cmdlog list'", "setenv CMDLOG_TZ +8")
    } else {
        ("alias cl='cmdlog list'", "export CMDLOG_TZ=+8")
    };

    // Append guarded block
    let block = format!(
        "\n{}\n{}\n{}\n{}\n{}\n",
        GUARD_BEGIN, source_line, alias_line, tz_line, GUARD_END
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

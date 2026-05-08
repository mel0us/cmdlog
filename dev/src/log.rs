use std::fs;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub fn enforce_private(path: &Path) {
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

pub fn write_private(path: &Path, content: &[u8]) -> io::Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(content)?;
    // mode(0o600) only applies on create; tighten in case a stale tmp file was reused.
    enforce_private(path);
    Ok(())
}

/// Reason a line was removed during compact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveReason {
    Malformed,
    Waived,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub date: String,
    pub shell: String,
    pub pwd: String,
    pub exit_code: String,
    pub cmd: String,
}

pub fn log_path() -> PathBuf {
    if let Ok(p) = std::env::var("CMDLOG_FILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".cmdlog.tsv")
}

/// Rewrite the log file, excluding entries whose (date, cmd) is in the deleted set.
/// Uses atomic temp-file + verify + rename.
pub fn rewrite_excluding(deleted: &std::collections::HashSet<String>) -> Result<(), String> {
    let path = log_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read log: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let original_count = lines.len();

    let mut removed = 0;
    let kept: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let parts: Vec<&str> = line.splitn(5, '\t').collect();
            if parts.len() != 5 {
                return true; // keep malformed lines
            }
            if deleted.contains(parts[0]) {
                removed += 1;
                false
            } else {
                true
            }
        })
        .collect();

    if removed == 0 {
        return Ok(());
    }

    let mut new_content = kept.join("\n");
    if !new_content.is_empty() {
        new_content.push('\n');
    }

    // PID-scoped temp file (safe if multiple TUI instances exit concurrently)
    let dir = path.parent().unwrap_or(std::path::Path::new("/tmp"));
    let tmp_path = dir.join(format!(".cmdlog.tsv.tmp.{}", std::process::id()));
    write_private(&tmp_path, new_content.as_bytes())
        .map_err(|e| format!("failed to write temp file: {}", e))?;

    // Verify: re-read and check line count
    let verify_content = fs::read_to_string(&tmp_path)
        .map_err(|e| format!("failed to re-read temp file: {}", e))?;
    let verify_count = if verify_content.is_empty() { 0 } else { verify_content.lines().count() };
    let expected_count = original_count - removed;
    if verify_count != expected_count {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "verification failed: expected {} lines, got {}. Original log preserved.",
            expected_count, verify_count
        ));
    }

    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("failed to rename temp file: {}", e))?;

    Ok(())
}

/// Rewrite the log file, removing lines that match the waive predicate or are malformed.
/// Calls `on_remove(line, reason)` for each removed line. Returns (original_count, removed_count).
pub fn compact_log(
    should_remove: impl Fn(&LogEntry) -> bool,
    mut on_remove: impl FnMut(&str, RemoveReason),
) -> Result<(usize, usize), String> {
    let path = log_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read log: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();
    let original_count = lines.len();

    let mut removed = 0;
    let kept: Vec<&str> = lines
        .into_iter()
        .filter(|line| {
            let parts: Vec<&str> = line.splitn(5, '\t').collect();
            if parts.len() != 5 {
                on_remove(line, RemoveReason::Malformed);
                removed += 1;
                return false;
            }
            let entry = LogEntry {
                date: parts[0].to_string(),
                shell: parts[1].to_string(),
                pwd: parts[2].to_string(),
                exit_code: parts[3].to_string(),
                cmd: parts[4].to_string(),
            };
            if should_remove(&entry) {
                on_remove(line, RemoveReason::Waived);
                removed += 1;
                false
            } else {
                true
            }
        })
        .collect();

    if removed == 0 {
        return Ok((original_count, 0));
    }

    let mut new_content = kept.join("\n");
    if !new_content.is_empty() {
        new_content.push('\n');
    }

    let dir = path.parent().unwrap_or(std::path::Path::new("/tmp"));
    let tmp_path = dir.join(format!(".cmdlog.tsv.tmp.{}", std::process::id()));
    write_private(&tmp_path, new_content.as_bytes())
        .map_err(|e| format!("failed to write temp file: {}", e))?;

    let verify_content = fs::read_to_string(&tmp_path)
        .map_err(|e| format!("failed to re-read temp file: {}", e))?;
    let verify_count = if verify_content.is_empty() { 0 } else { verify_content.lines().count() };
    let expected_count = original_count - removed;
    if verify_count != expected_count {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "verification failed: expected {} lines, got {}. Original log preserved.",
            expected_count, verify_count
        ));
    }

    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("failed to rename temp file: {}", e))?;

    Ok((original_count, removed))
}

/// Count lines that match the waive predicate or are malformed, without modifying the log.
/// Calls `on_remove(line, reason)` for each line that would be removed.
pub fn compact_dry_run(
    should_remove: impl Fn(&LogEntry) -> bool,
    mut on_remove: impl FnMut(&str, RemoveReason),
) -> Result<(usize, usize), String> {
    let path = log_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read log: {}", e))?;

    let mut total = 0;
    let mut removed = 0;
    for line in content.lines() {
        total += 1;
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() != 5 {
            on_remove(line, RemoveReason::Malformed);
            removed += 1;
            continue;
        }
        let entry = LogEntry {
            date: parts[0].to_string(),
            shell: parts[1].to_string(),
            pwd: parts[2].to_string(),
            exit_code: parts[3].to_string(),
            cmd: parts[4].to_string(),
        };
        if should_remove(&entry) {
            on_remove(line, RemoveReason::Waived);
            removed += 1;
        }
    }
    Ok((total, removed))
}

pub fn load_entries() -> Vec<LogEntry> {
    let path = log_path();
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = io::BufReader::new(file);
    let mut entries = Vec::new();
    let mut last_cmd = String::new();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() != 5 {
            continue;
        }
        if parts[4] == last_cmd {
            continue;
        }
        last_cmd = parts[4].to_string();
        entries.push(LogEntry {
            date: parts[0].to_string(),
            shell: parts[1].to_string(),
            pwd: parts[2].to_string(),
            exit_code: parts[3].to_string(),
            cmd: last_cmd.clone(),
        });
    }
    entries
}

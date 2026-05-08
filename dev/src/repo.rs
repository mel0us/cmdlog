use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolved repo metadata for a directory.
#[derive(Clone, Debug)]
pub struct RepoInfo {
    pub name: String,
    pub root: String,
}

/// Abstraction for resolving a directory to its git repo info.
pub trait RepoResolver {
    fn resolve(&mut self, dir: &str) -> Option<RepoInfo>;

    fn repo_name(&mut self, dir: &str) -> String {
        self.resolve(dir).map(|i| i.name).unwrap_or_default()
    }

    fn resolve_name_and_relpath(&mut self, dir: &str) -> (String, String) {
        match self.resolve(dir) {
            Some(info) => {
                let rel = dir.strip_prefix(&info.root)
                    .unwrap_or(dir)
                    .trim_start_matches('/')
                    .to_string();
                (info.name, rel)
            }
            None => (String::new(), String::new()),
        }
    }
}

/// Cached mapping from directory to (repo_short_name, repo_root).
/// All paths are absolute.
pub struct RepoCache {
    cache_path: PathBuf,
    /// dir -> (short_name, repo_root)
    entries: HashMap<String, (String, String)>,
}

impl RepoCache {
    pub fn load(cmdlog_dir: &Path) -> Self {
        let cache_path = cmdlog_dir.join(".repo_cache");
        let mut entries = HashMap::new();
        if let Ok(content) = fs::read_to_string(&cache_path) {
            for line in content.lines() {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.len() == 3 {
                    entries.insert(
                        parts[0].to_string(),
                        (parts[1].to_string(), parts[2].to_string()),
                    );
                }
            }
        }
        RepoCache { cache_path, entries }
    }

    pub fn save(&self) {
        let mut content = String::new();
        for (dir, (name, root)) in &self.entries {
            if name.is_empty() && root.is_empty() {
                continue; // skip negative cache entries
            }
            content.push_str(dir);
            content.push('\t');
            content.push_str(name);
            content.push('\t');
            content.push_str(root);
            content.push('\n');
        }
        let _ = fs::write(&self.cache_path, content);
    }

}

impl RepoResolver for RepoCache {
    /// Resolve a directory to repo info.
    /// Returns None if the directory is not inside a git repo.
    fn resolve(&mut self, dir: &str) -> Option<RepoInfo> {
        if let Some(cached) = self.entries.get(dir) {
            if cached.0.is_empty() && cached.1.is_empty() {
                return None; // cached negative result
            }
            if dir.starts_with(&cached.1) {
                return Some(RepoInfo {
                    name: cached.0.clone(),
                    root: cached.1.clone(),
                });
            }
            self.entries.remove(dir);
        }

        match find_git_repo(dir) {
            Some(result) => {
                self.entries.insert(dir.to_string(), result.clone());
                Some(RepoInfo {
                    name: result.0,
                    root: result.1,
                })
            }
            None => {
                // Cache negative result to avoid repeated filesystem walks
                self.entries.insert(dir.to_string(), (String::new(), String::new()));
                None
            }
        }
    }
}

/// Walk up from `dir` to find `.git/`, resolve repo name, return
/// (short_name, repo_root).
fn find_git_repo(dir: &str) -> Option<(String, String)> {
    let mut current = PathBuf::from(dir);
    loop {
        let git_dir = current.join(".git");
        if git_dir.is_dir() {
            let config_path = git_dir.join("config");
            let repo_root = current.to_string_lossy().to_string();
            let short_name = resolve_repo_name(&config_path, &current);
            return Some((short_name, repo_root));
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Resolve a repo name with fallback priority:
/// 1. Remote "origin" URL → owner/repo
/// 2. Any other remote URL → owner/repo
/// 3. No remote → $USER/dirname
fn resolve_repo_name(config_path: &Path, repo_root: &Path) -> String {
    // Read file once, scan for origin then any remote
    if let Ok(content) = fs::read_to_string(config_path) {
        if let Some(name) = parse_remote_from_content(&content, Some("origin"))
            .or_else(|| parse_remote_from_content(&content, None))
        {
            return name;
        }
    }
    // Fallback: user/dirname
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "local".to_string());
    let dirname = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    format!("{}/{}", user, dirname)
}

/// Parse a remote URL from git config content (avoids re-reading the file).
fn parse_remote_from_content(content: &str, name: Option<&str>) -> Option<String> {
    let target_header = name.map(|n| format!("[remote \"{}\"]", n));
    let mut in_target = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[remote \"") && trimmed.ends_with("\"]") {
            in_target = match &target_header {
                Some(h) => trimmed == h,
                None => true,
            };
            continue;
        }
        if trimmed.starts_with('[') {
            in_target = false;
            continue;
        }
        if in_target && trimmed.starts_with("url") {
            if let Some(url) = trimmed.split('=').nth(1) {
                let short = extract_short_name(url.trim());
                if !short.is_empty() {
                    return Some(short);
                }
            }
        }
    }
    None
}


/// Extract `owner/repo` from a git remote URL.
/// Handles https://host/owner/repo.git, git@host:owner/repo.git,
/// ssh://host/owner/repo.git, git://host/owner/repo.git, etc.
pub fn extract_short_name(url: &str) -> String {
    let mut path = url.trim();

    // Strip common schemes
    for scheme in &["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = path.strip_prefix(scheme) {
            path = rest;
            break;
        }
    }

    // Handle git@host:owner/repo (SSH shorthand)
    if let Some(rest) = path.strip_prefix("git@") {
        path = rest;
        // Replace first ':' with '/' (git@host:owner/repo -> host/owner/repo)
        if let Some(colon_pos) = path.find(':') {
            let mut s = path.to_string();
            s.replace_range(colon_pos..colon_pos + 1, "/");
            // Strip .git suffix, take last two segments
            let s = s.trim_end_matches(".git").to_string();
            let segments: Vec<&str> = s.split('/').filter(|s| !s.is_empty()).collect();
            return if segments.len() >= 2 {
                format!("{}/{}", segments[segments.len() - 2], segments[segments.len() - 1])
            } else {
                s.to_string()
            };
        }
    }

    // Strip .git suffix
    let path = path.trim_end_matches(".git");

    // Take last two path segments (strips host portion)
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() >= 2 {
        format!("{}/{}", segments[segments.len() - 2], segments[segments.len() - 1])
    } else if segments.len() == 1 {
        segments[0].to_string()
    } else {
        String::new()
    }
}

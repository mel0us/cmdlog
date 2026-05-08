use std::collections::HashMap;

use crate::cmd;
use crate::log::LogEntry;
use crate::repo::RepoResolver;
use crate::tui::state::{GroupDimension, FilterToggle, OrderDimension};

/// All parameters the pipeline needs, decoupled from AppState.
pub struct FilterSpec<'r> {
    pub filter_shell: bool,
    pub filter_dir: bool,
    pub filter_repo: bool,
    pub filter_today: bool,
    pub exit_success: bool,
    pub exit_failure: bool,
    pub operator_piped: bool,
    pub operator_chained: bool,
    pub search_regex: Option<&'r regex::Regex>,
    pub deleted: &'r std::collections::HashSet<String>,
    pub waive_commands: &'r [String],
    pub waive_min_cmd_len: usize,
    pub dedup: bool,
    pub order: &'r [super::state::OrderBadge],
    pub group: &'r [(GroupDimension, bool)],
}

impl<'r> From<&'r super::state::AppState> for FilterSpec<'r> {
    fn from(s: &'r super::state::AppState) -> Self {
        FilterSpec {
            filter_shell: s.filter.is_filter_enabled(FilterToggle::ThisShell),
            filter_dir: s.filter.is_filter_enabled(FilterToggle::ThisDir),
            filter_repo: s.filter.is_filter_enabled(FilterToggle::ThisRepo),
            filter_today: s.filter.is_filter_enabled(FilterToggle::Today),
            exit_success: s.filter.is_exit_filter_success(),
            exit_failure: s.filter.is_exit_filter_failure(),
            operator_piped: s.filter.is_operator_filter_piped(),
            operator_chained: s.filter.is_operator_filter_chained(),
            search_regex: s.search.search_regex.as_ref(),
            deleted: s.delete_log.deleted_set(),
            waive_commands: &s.session.waive_commands,
            waive_min_cmd_len: s.session.waive_min_cmd_len,
            dedup: s.filter.dedup,
            order: &s.filter.order,
            group: &s.filter.group,
        }
    }
}

/// A processed entry with precomputed metadata for display and sorting.
#[derive(Clone)]
pub struct DisplayEntry {
    pub entry: LogEntry,
    pub repo_name: String,
    pub relpath: String,
    pub frequency: usize,
    pub group_score: u8,
}

/// Build frequency map: cmd -> count
pub fn build_frequency_map(entries: &[LogEntry]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for e in entries {
        *map.entry(e.cmd.clone()).or_insert(0) += 1;
    }
    map
}

/// Compute relative age string from a timestamp like "2026-04-06T10:30:15".
pub fn age_string(date: &str) -> String {
    // Both the entry timestamp and "now" are in local time (from localtime_r),
    // so we can diff them directly using the same manual epoch calculation.
    let entry_secs = parse_local_timestamp(date);
    let now_secs = crate::time::current_local_seconds();

    if now_secs <= entry_secs {
        return "just now".to_string();
    }
    let diff = now_secs - entry_secs;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 86400 * 30 {
        format!("{}d ago", diff / 86400)
    } else if diff < 86400 * 365 {
        format!("{}mo ago", diff / (86400 * 30))
    } else {
        format!("{}y ago", diff / (86400 * 365))
    }
}

/// Parse a "YYYY-MM-DDTHH:MM:SS" local-time timestamp into seconds since epoch.
pub fn parse_local_timestamp(date: &str) -> u64 {
    let parts: Vec<&str> = date.split('T').collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_parts: Vec<u32> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return 0;
    }
    crate::time::days_since_epoch(date_parts[0], date_parts[1], date_parts[2]) * 86400
        + time_parts[0] as u64 * 3600
        + time_parts[1] as u64 * 60
        + time_parts[2] as u64
}


/// Session-level context that doesn't change during the TUI session.
pub struct PipelineContext<'a> {
    pub current_shell: &'a str,
    pub current_dir: &'a str,
    pub current_repo: &'a str,
}

/// Apply filters, search, sort, and rank to produce the display list.
pub fn apply_pipeline<R: RepoResolver>(
    all_entries: &[LogEntry],
    spec: &FilterSpec<'_>,
    repo_cache: &mut R,
    freq_map: &HashMap<String, usize>,
    ctx: &PipelineContext<'_>,
) -> Vec<DisplayEntry> {
    let today = if spec.filter_today {
        Some(crate::time::today_prefix())
    } else {
        None
    };

    // Step 1: Filter
    let filtered: Vec<&LogEntry> = all_entries
        .iter()
        .filter(|e| {
            // Soft-deleted entries (date is unique per log line)
            if !spec.deleted.is_empty() && spec.deleted.contains(&e.date) {
                return false;
            }
            if spec.filter_shell && e.shell != ctx.current_shell {
                return false;
            }
            if spec.filter_dir && e.pwd != ctx.current_dir {
                return false;
            }
            if spec.filter_repo {
                let entry_repo = repo_cache.repo_name(&e.pwd);
                if entry_repo != ctx.current_repo {
                    return false;
                }
            }
            if let Some(ref t) = today {
                if !e.date.starts_with(t.as_str()) {
                    return false;
                }
            }
            if spec.operator_piped && !e.cmd.contains('|') {
                return false;
            }
            if spec.operator_chained && !e.cmd.bytes().any(|b| b";&|".contains(&b)) {
                return false;
            }
            if spec.exit_success && e.exit_code != "0" {
                return false;
            }
            if spec.exit_failure && e.exit_code == "0" {
                return false;
            }
            // Waive + min-length filters (skip if command has shell operators)
            if cmd::should_waive(&e.cmd, spec.waive_commands, spec.waive_min_cmd_len) {
                return false;
            }
            if let Some(re) = spec.search_regex {
                if !re.is_match(&e.cmd) {
                    return false;
                }
            }
            true
        })
        .collect();

    // Step 2: Group + Dedup + Order
    let current_relpath = repo_cache.resolve_name_and_relpath(ctx.current_dir).1;
    let enabled_groups: Vec<&GroupDimension> = spec.group.iter()
        .filter(|(_, en)| *en)
        .map(|(dim, _)| dim)
        .collect();

    let mut display: Vec<DisplayEntry> = filtered
        .into_iter()
        .map(|e| {
            let (repo_name, relpath) = repo_cache.resolve_name_and_relpath(&e.pwd);
            let frequency = if spec.dedup {
                *freq_map.get(&e.cmd).unwrap_or(&1)
            } else {
                1
            };
            let de = DisplayEntry {
                entry: e.clone(),
                repo_name,
                relpath,
                frequency,
                group_score: 0,
            };
            let score = group_score(&de, &enabled_groups, ctx.current_dir, ctx.current_repo, &current_relpath);
            DisplayEntry { group_score: score, ..de }
        })
        .collect();

    if spec.dedup {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for e in display.into_iter().rev() {
            if seen.insert((e.group_score, e.entry.cmd.clone())) {
                deduped.push(e);
            }
        }
        deduped.reverse();
        display = deduped;
    }

    display.sort_by(|a, b| {
        // Compare cached group scores (left-to-right priority baked in)
        match b.group_score.cmp(&a.group_score) {
            std::cmp::Ordering::Equal => {
                // Within same group, apply order dimensions (left-most = primary)
                for badge in spec.order {
                    let cmp = match badge.dim {
                        OrderDimension::Recency => {
                            let c = a.entry.date.cmp(&b.entry.date);
                            if badge.ascending { c.reverse() } else { c }
                        }
                        OrderDimension::Frequency => {
                            let c = a.frequency.cmp(&b.frequency);
                            if badge.ascending { c.reverse() } else { c }
                        }
                    };
                    if cmp != std::cmp::Ordering::Equal {
                        return cmp;
                    }
                }
                std::cmp::Ordering::Equal
            }
            other => other,
        }
    });

    display
}

/// Compute a group score as a bitmask: bit (N-1-i) is set if dimension i matches.
/// Higher score = higher priority group. Allocation-free, O(1) comparison.
fn group_score(
    entry: &DisplayEntry,
    enabled_groups: &[&GroupDimension],
    current_dir: &str,
    current_repo: &str,
    current_relpath: &str,
) -> u8 {
    let len = enabled_groups.len();
    enabled_groups.iter().enumerate().fold(0u8, |acc, (i, dim)| {
        if matches_group(entry, dim, current_dir, current_repo, current_relpath) {
            acc | (1 << (len - 1 - i))
        } else {
            acc
        }
    })
}

fn matches_group(
    entry: &DisplayEntry,
    dim: &GroupDimension,
    current_dir: &str,
    current_repo: &str,
    current_relpath: &str,
) -> bool {
    match dim {
        GroupDimension::Dir => entry.entry.pwd == current_dir,
        GroupDimension::Repo => !entry.repo_name.is_empty() && entry.repo_name == current_repo,
        GroupDimension::RelPath => {
            !entry.relpath.is_empty() && entry.relpath == current_relpath
        }
    }
}

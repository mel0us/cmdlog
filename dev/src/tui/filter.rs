use std::cell::RefCell;
use std::collections::HashMap;

use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::cmd;
use crate::log::LogEntry;
use crate::repo::RepoResolver;
use crate::tui::state::{GroupDimension, FilterToggle, OrderDimension};

// Thread-local fuzzy matcher + reusable haystack buffer. The TUI is single-
// threaded and tests run with --test-threads=1, so a single Matcher per
// thread is safe. The Matcher caches DP tables across calls — reuse matters
// for per-keystroke latency. The needle is segmented once per pipeline run
// in `apply_pipeline` (see `NeedleBuf`) rather than once per entry, so it
// doesn't live here.
thread_local! {
    static MATCHER: RefCell<Matcher> = RefCell::new(Matcher::new(Config::DEFAULT));
    static HAYSTACK_BUF: RefCell<Vec<char>> = const { RefCell::new(Vec::new()) };
}

/// Owns the codepoint buffer backing a pre-segmented needle. Allocated once
/// per pipeline run, borrowed by `Utf32Str` for every per-entry score call.
pub struct NeedleBuf {
    chars: Vec<char>,
    is_empty: bool,
}

impl NeedleBuf {
    pub fn new(needle: &str) -> Self {
        let mut chars = Vec::new();
        // Pre-segment once. `Utf32Str::new` fills `chars` only for non-ASCII
        // needles; ASCII path is zero-allocation. We discard the returned
        // Utf32Str here because its lifetime is tied to `chars` and we want
        // to hand out fresh borrows in `as_utf32`.
        let _ = Utf32Str::new(needle, &mut chars);
        NeedleBuf { chars, is_empty: needle.is_empty() }
    }

    fn as_utf32<'a>(&'a self, needle: &'a str) -> Utf32Str<'a> {
        // ASCII needles bypass the chars buffer entirely.
        if needle.is_ascii() {
            Utf32Str::Ascii(needle.as_bytes())
        } else {
            Utf32Str::Unicode(&self.chars)
        }
    }
}

/// Score a haystack against a pre-segmented needle. Returns None when the
/// needle is not a subsequence of the haystack. Empty needle returns
/// `Some(0)` uniformly so the "no query" case degenerates to a tied sort.
pub fn fuzzy_score(haystack: &str, needle: &str, buf: &NeedleBuf) -> Option<u16> {
    if buf.is_empty {
        return Some(0);
    }
    MATCHER.with(|m| {
        HAYSTACK_BUF.with(|hb| {
            let mut hb = hb.borrow_mut();
            let h = Utf32Str::new(haystack, &mut hb);
            let n = buf.as_utf32(needle);
            m.borrow_mut().fuzzy_match(h, n)
        })
    })
}

/// Score + record the codepoint positions in `haystack` that the needle
/// matched. `out` is appended to (nucleo doesn't clear it). Callers are
/// expected to `out.clear()` between uses. Only called for the visible
/// viewport during rendering — `fuzzy_indices` is more expensive than
/// `fuzzy_match` because it tracks the DP path, so don't call it from the
/// filter loop where we score every candidate.
pub fn fuzzy_indices(
    haystack: &str,
    needle: &str,
    buf: &NeedleBuf,
    out: &mut Vec<u32>,
) -> Option<u16> {
    if buf.is_empty {
        return Some(0);
    }
    MATCHER.with(|m| {
        HAYSTACK_BUF.with(|hb| {
            let mut hb = hb.borrow_mut();
            let h = Utf32Str::new(haystack, &mut hb);
            let n = buf.as_utf32(needle);
            m.borrow_mut().fuzzy_indices(h, n, out)
        })
    })
}

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
    /// Fuzzy needle. `None` (or empty string semantically) means "no search
    /// active" — every entry passes and score sort is skipped.
    pub search_query: Option<&'r str>,
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
            search_query: if s.search.search_input.is_empty() {
                None
            } else {
                Some(s.search.search_input.as_str())
            },
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
    /// Fuzzy-match score against the active search query (0 when no query
    /// is active). Higher = better match; nucleo scores favor word starts,
    /// camelCase boundaries, and consecutive char runs.
    pub search_score: u16,
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

    // Step 1: Filter + score. Fuzzy match runs last so we only pay scoring
    // cost for entries that survive the cheap predicate filters. The needle
    // is segmented once here (NeedleBuf), then borrowed by every per-entry
    // call inside the loop — avoids O(needle_len) re-segmentation per entry.
    let needle = spec.search_query.unwrap_or("");
    let needle_buf = NeedleBuf::new(needle);
    let filtered: Vec<(&LogEntry, u16)> = all_entries
        .iter()
        .filter_map(|e| {
            if !spec.deleted.is_empty() && spec.deleted.contains(&e.date) {
                return None;
            }
            if spec.filter_shell && e.shell != ctx.current_shell {
                return None;
            }
            if spec.filter_dir && e.pwd != ctx.current_dir {
                return None;
            }
            if spec.filter_repo {
                let entry_repo = repo_cache.repo_name(&e.pwd);
                if entry_repo != ctx.current_repo {
                    return None;
                }
            }
            if let Some(ref t) = today {
                if !e.date.starts_with(t.as_str()) {
                    return None;
                }
            }
            if spec.operator_piped && !e.cmd.contains('|') {
                return None;
            }
            if spec.operator_chained && !e.cmd.bytes().any(|b| b";&|".contains(&b)) {
                return None;
            }
            if spec.exit_success && e.exit_code != "0" {
                return None;
            }
            if spec.exit_failure && e.exit_code == "0" {
                return None;
            }
            if cmd::should_waive(&e.cmd, spec.waive_commands, spec.waive_min_cmd_len) {
                return None;
            }
            // Fuzzy match — empty needle scores 0 for everything (no filter).
            let score = fuzzy_score(&e.cmd, needle, &needle_buf)?;
            Some((e, score))
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
        .map(|(e, search_score)| {
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
                search_score,
            };
            let score = group_score(&de, &enabled_groups, ctx.current_dir, ctx.current_repo, &current_relpath);
            DisplayEntry { group_score: score, ..de }
        })
        .collect();

    if spec.dedup {
        // Collapse identical commands across all groups. Keep the occurrence
        // with the highest group_score so the survivor reflects the most
        // contextually relevant run; tie-break to the most recent.
        let mut best: HashMap<String, usize> = HashMap::new();
        for (idx, e) in display.iter().enumerate() {
            match best.get(&e.entry.cmd) {
                Some(&prev) if display[prev].group_score > e.group_score => {}
                _ => {
                    best.insert(e.entry.cmd.clone(), idx);
                }
            }
        }
        let keep: std::collections::HashSet<usize> = best.into_values().collect();
        display = display
            .into_iter()
            .enumerate()
            .filter_map(|(i, e)| if keep.contains(&i) { Some(e) } else { None })
            .collect();
    }

    display.sort_by(|a, b| {
        // Fuzzy score is always the primary sort key. With no active query,
        // every entry scores 0 — the comparison resolves to Equal and falls
        // through to group_score and order badges, preserving the original
        // pre-search ordering. With a query, strongest match wins.
        match b.search_score.cmp(&a.search_score) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
        match b.group_score.cmp(&a.group_score) {
            std::cmp::Ordering::Equal => {
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

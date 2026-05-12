use cmdlog::log::LogEntry;
use cmdlog::repo::RepoResolver;
use cmdlog::tui::filter::*;
use cmdlog::tui::state::*;

struct MockRepoResolver {
    data: std::collections::HashMap<String, (String, String)>,
}

impl MockRepoResolver {
    fn new() -> Self {
        MockRepoResolver { data: std::collections::HashMap::new() }
    }
}

impl RepoResolver for MockRepoResolver {
    fn resolve(&mut self, dir: &str) -> Option<cmdlog::repo::RepoInfo> {
        self.data.get(dir).map(|(name, root)| cmdlog::repo::RepoInfo {
            name: name.clone(),
            root: root.clone(),
        })
    }
}

fn ctx<'a>(shell: &'a str, dir: &'a str, repo: &'a str) -> PipelineContext<'a> {
    PipelineContext { current_shell: shell, current_dir: dir, current_repo: repo }
}

// ---------------------------------------------------------------------------
// Helper: create LogEntry
// ---------------------------------------------------------------------------

fn entry(date: &str, shell: &str, pwd: &str, cmd: &str) -> LogEntry {
    LogEntry {
        date: date.to_string(),
        shell: shell.to_string(),
        pwd: pwd.to_string(),
        exit_code: "0".to_string(),
        cmd: cmd.to_string(),
    }
}

fn sample_entries() -> Vec<LogEntry> {
    vec![
        entry("2026-04-05T09:00:00", "bash", "/home/user/proj-a", "git status"),
        entry("2026-04-05T10:00:00", "zsh", "/home/user/proj-b", "make -j8"),
        entry("2026-04-06T08:00:00", "bash", "/home/user/proj-a", "git pull"),
        entry("2026-04-06T09:00:00", "bash", "/home/user/proj-a", "pytest tests/"),
        entry("2026-04-06T10:00:00", "tcsh", "/tmp", "nvcc --version"),
        entry("2026-04-06T11:00:00", "bash", "/home/user/proj-a", "echo hello | tr a b"),
        entry("2026-04-06T12:00:00", "bash", "/home/user/proj-a", "git status"),
    ]
}

// ---------------------------------------------------------------------------
// build_frequency_map
// ---------------------------------------------------------------------------

#[test]
fn frequency_map_basic() {
    let entries = sample_entries();
    let map = build_frequency_map(&entries);
    assert_eq!(*map.get("git status").unwrap(), 2);
    assert_eq!(*map.get("make -j8").unwrap(), 1);
    assert_eq!(*map.get("pytest tests/").unwrap(), 1);
}

#[test]
fn frequency_map_empty() {
    let entries: Vec<LogEntry> = vec![];
    let map = build_frequency_map(&entries);
    assert!(map.is_empty());
}

// ---------------------------------------------------------------------------
// parse_local_timestamp
// ---------------------------------------------------------------------------

#[test]
fn parse_valid_timestamp() {
    let secs = parse_local_timestamp("2026-04-06T10:30:15");
    assert!(secs > 0);
}

#[test]
fn parse_timestamp_bad_format() {
    assert_eq!(parse_local_timestamp("not-a-date"), 0);
    assert_eq!(parse_local_timestamp(""), 0);
    assert_eq!(parse_local_timestamp("2026-04-06"), 0); // no T separator
    assert_eq!(parse_local_timestamp("T10:30:15"), 0);  // no date part
}

#[test]
fn parse_timestamp_consistency() {
    // Earlier timestamp → smaller value
    let t1 = parse_local_timestamp("2026-04-05T10:00:00");
    let t2 = parse_local_timestamp("2026-04-06T10:00:00");
    assert!(t1 < t2, "t1={} should be < t2={}", t1, t2);
}

#[test]
fn parse_timestamp_one_day_apart() {
    let t1 = parse_local_timestamp("2026-04-05T00:00:00");
    let t2 = parse_local_timestamp("2026-04-06T00:00:00");
    assert_eq!(t2 - t1, 86400); // exactly one day
}

// ---------------------------------------------------------------------------
// age_string
// ---------------------------------------------------------------------------

#[test]
fn age_string_future_date() {
    // A date far in the future should return "just now"
    let result = age_string("2099-12-31T23:59:59");
    assert_eq!(result, "just now");
}

// We can't easily test exact age_string output without controlling "now",
// but we can verify the format pattern for various known timestamps.
#[test]
fn age_string_returns_nonempty() {
    let result = age_string("2020-01-01T00:00:00");
    assert!(!result.is_empty());
    // Should be years ago
    assert!(result.contains("y ago"), "expected 'y ago', got '{}'", result);
}

// ---------------------------------------------------------------------------
// apply_pipeline: no filters
// ---------------------------------------------------------------------------

#[test]
fn pipeline_no_filters() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let state = AppState::new();
    // Disable context grouping by making current_dir something that doesn't match
    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 7);
}

// ---------------------------------------------------------------------------
// apply_pipeline: shell filter
// ---------------------------------------------------------------------------

#[test]
fn pipeline_filter_this_shell() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    let idx = state.filter.filters.iter().position(|(f, _)| *f == FilterToggle::ThisShell).unwrap();
    state.filter.toggle_filter(idx);

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/nonexistent", ""),
    );
    // 5 bash entries
    assert_eq!(result.len(), 5);
    for de in &result {
        assert_eq!(de.entry.shell, "bash");
    }
}

// ---------------------------------------------------------------------------
// apply_pipeline: dir filter
// ---------------------------------------------------------------------------

#[test]
fn pipeline_filter_this_dir() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    let idx = state.filter.filters.iter().position(|(f, _)| *f == FilterToggle::ThisDir).unwrap();
    state.filter.toggle_filter(idx);

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home/user/proj-a", ""),
    );
    // 5 entries in proj-a
    assert_eq!(result.len(), 5);
}

// ---------------------------------------------------------------------------
// apply_pipeline: piped filter
// ---------------------------------------------------------------------------

#[test]
fn pipeline_filter_piped() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    let idx = state.filter.filters.iter().position(|(f, _)| *f == FilterToggle::Operator).unwrap();
    state.filter.toggle_filter(idx); // off → piped

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 1);
    assert!(result[0].entry.cmd.contains('|'));
}

// ---------------------------------------------------------------------------
// apply_pipeline: dedup
// ---------------------------------------------------------------------------

#[test]
fn pipeline_dedup() {
    let entries = sample_entries(); // has "git status" at index 0 and 6
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.filter.dedup = true;

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    // 7 entries, 1 duplicate "git status" → 6 unique
    assert_eq!(result.len(), 6);
    // The latest occurrence (index 6) should be kept
    let git_status: Vec<_> = result.iter().filter(|de| de.entry.cmd == "git status").collect();
    assert_eq!(git_status.len(), 1);
    assert_eq!(git_status[0].entry.date, "2026-04-06T12:00:00");
}

// ---------------------------------------------------------------------------
// apply_pipeline: fuzzy search
// ---------------------------------------------------------------------------

#[test]
fn pipeline_search_substring_match() {
    // Substring queries are a subset of fuzzy matching — "git" matches
    // every entry containing g→i→t in order, which for sample_entries
    // is just the three literal "git ..." lines.
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.search.search_input = "git".to_string();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 3, "expected git status x2 + git pull");
    for de in &result {
        assert!(de.entry.cmd.contains("git"));
        assert!(de.search_score > 0, "matched entries must have non-zero fuzzy score");
    }
}

#[test]
fn pipeline_search_subsequence_match() {
    // Fuzzy matching tolerates gaps: "gst" should match "git status"
    // because g, s, t appear in order (with gaps).
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.search.search_input = "gst".to_string();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert!(
        result.iter().any(|de| de.entry.cmd == "git status"),
        "fuzzy 'gst' must match 'git status'",
    );
}

#[test]
fn pipeline_search_no_match_returns_empty() {
    // No subsequence match → zero results.
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.search.search_input = "@@@".to_string();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 0);
}

#[test]
fn pipeline_search_orders_by_fuzzy_score() {
    // When a query is active, best fuzzy match comes first regardless of
    // chronological order. Construct entries where the best match is
    // logically older than a worse match.
    let entries = vec![
        LogEntry {
            date: "2026-01-01T00:00:00".to_string(),
            shell: "bash".to_string(),
            pwd: "/home".to_string(),
            exit_code: "0".to_string(),
            cmd: "git status".to_string(),  // strong match for "git"
        },
        LogEntry {
            date: "2026-01-02T00:00:00".to_string(),
            shell: "bash".to_string(),
            pwd: "/home".to_string(),
            exit_code: "0".to_string(),
            cmd: "vagrant init".to_string(), // weaker fuzzy match for "git"
        },
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.search.search_input = "git".to_string();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home", ""),
    );
    assert!(!result.is_empty(), "expected at least one fuzzy match");
    assert_eq!(
        result[0].entry.cmd, "git status",
        "strongest fuzzy match must sort first when search is active",
    );
}

// ---------------------------------------------------------------------------
// apply_pipeline: frequency metadata
// ---------------------------------------------------------------------------

#[test]
fn pipeline_frequency_in_display_entries() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let state = AppState::new();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    // Dedup off (default): all frequencies are 1
    for de in &result {
        assert_eq!(de.frequency, 1);
    }
}

#[test]
fn pipeline_frequency_with_dedup_on() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.filter.dedup = true;

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    // "git status" appears twice → frequency 2; all others → 1
    for de in &result {
        if de.entry.cmd == "git status" {
            assert_eq!(de.frequency, 2);
        } else {
            assert_eq!(de.frequency, 1);
        }
    }
}

// ---------------------------------------------------------------------------
// apply_pipeline: ordering
// ---------------------------------------------------------------------------

#[test]
fn pipeline_order_recency_new_first() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let state = AppState::new();
    // Default: recency new first (ascending=true in code means "new first" for recency)

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    // Most recent entry first
    assert_eq!(result[0].entry.date, "2026-04-06T12:00:00");
}

#[test]
fn pipeline_order_recency_old_first() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.filter.toggle_order_direction(0); // flip recency to "old first"

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    // Oldest entry first
    assert_eq!(result[0].entry.date, "2026-04-05T09:00:00");
}

// ---------------------------------------------------------------------------
// apply_pipeline: combined filters
// ---------------------------------------------------------------------------

#[test]
fn pipeline_combined_shell_and_search() {
    let entries = sample_entries();
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    let idx = state.filter.filters.iter().position(|(f, _)| *f == FilterToggle::ThisShell).unwrap();
    state.filter.toggle_filter(idx);
    state.search.search_input = "git".to_string();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/nonexistent", ""),
    );
    // bash + git: "git status"x2 + "git pull" = 3
    assert_eq!(result.len(), 3);
}

// ---------------------------------------------------------------------------
// apply_pipeline: empty input
// ---------------------------------------------------------------------------

#[test]
fn pipeline_empty_entries() {
    let entries: Vec<LogEntry> = vec![];
    let freq_map = build_frequency_map(&entries);
    let state = AppState::new();

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/tmp", ""),
    );
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// apply_pipeline: waive filter
// ---------------------------------------------------------------------------

#[test]
fn pipeline_filters_waived_commands() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home", "git status"),
        entry("2026-04-06T10:01:00", "bash", "/home", "ls -la"),
        entry("2026-04-06T10:02:00", "bash", "/home", "grep foo bar"),
        entry("2026-04-06T10:03:00", "bash", "/home", "make -j8"),
    ];
    let freq = build_frequency_map(&entries);
    let mut cache = MockRepoResolver::new();
    let mut state = AppState::new();
    state.session.waive_commands = vec!["ls".to_string(), "grep".to_string()];

    let result = apply_pipeline(&entries, &FilterSpec::from(&state), &mut cache, &freq, &ctx("bash", "/home", ""));
    let cmds: Vec<&str> = result.iter().map(|d| d.entry.cmd.as_str()).collect();
    assert_eq!(cmds, vec!["make -j8", "git status"]);
}

// ---------------------------------------------------------------------------
// apply_pipeline: context group
// ---------------------------------------------------------------------------

#[test]
fn pipeline_group_dir_partitions_entries() {
    // Entries from two dirs: /home/user/proj-a and /tmp
    let entries = vec![
        entry("2026-04-06T08:00:00", "bash", "/tmp", "old-cmd"),
        entry("2026-04-06T10:00:00", "bash", "/home/user/proj-a", "git pull"),
        entry("2026-04-06T11:00:00", "bash", "/tmp", "newer-cmd"),
        entry("2026-04-06T12:00:00", "bash", "/home/user/proj-a", "make"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    // Enable only Dir grouping
    for (dim, en) in &mut state.filter.group {
        *en = *dim == GroupDimension::Dir;
    }

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home/user/proj-a", ""),
    );
    assert_eq!(result.len(), 4);
    // Matching dir entries must all come before non-matching
    assert_eq!(result[0].entry.pwd, "/home/user/proj-a");
    assert_eq!(result[1].entry.pwd, "/home/user/proj-a");
    assert_eq!(result[2].entry.pwd, "/tmp");
    assert_eq!(result[3].entry.pwd, "/tmp");
}

#[test]
fn pipeline_group_sort_within_bucket() {
    // Two entries in current dir, two outside. Recency sort should apply within each bucket.
    let entries = vec![
        entry("2026-04-06T08:00:00", "bash", "/home/user", "early-here"),
        entry("2026-04-06T09:00:00", "bash", "/other", "early-other"),
        entry("2026-04-06T10:00:00", "bash", "/home/user", "late-here"),
        entry("2026-04-06T11:00:00", "bash", "/other", "late-other"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    for (dim, en) in &mut state.filter.group {
        *en = *dim == GroupDimension::Dir;
    }
    // Default order: recency new-first

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home/user", ""),
    );
    // Bucket 1 (matching dir, new first): late-here, early-here
    // Bucket 2 (non-matching, new first): late-other, early-other
    assert_eq!(result[0].entry.cmd, "late-here");
    assert_eq!(result[1].entry.cmd, "early-here");
    assert_eq!(result[2].entry.cmd, "late-other");
    assert_eq!(result[3].entry.cmd, "early-other");
}

#[test]
fn pipeline_group_newer_nonmatch_stays_below_older_match() {
    // The key invariant: a newer entry outside the current dir must NOT
    // appear above an older entry inside the current dir.
    let entries = vec![
        entry("2026-04-06T08:00:00", "bash", "/home/user", "old-matching"),
        entry("2026-04-06T12:00:00", "bash", "/other", "new-nonmatching"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    for (dim, en) in &mut state.filter.group {
        *en = *dim == GroupDimension::Dir;
    }

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home/user", ""),
    );
    assert_eq!(result[0].entry.cmd, "old-matching");
    assert_eq!(result[1].entry.cmd, "new-nonmatching");
}

#[test]
fn pipeline_group_disabled_no_partition() {
    // With all groups disabled, recency sort should be flat (no partitioning)
    let entries = vec![
        entry("2026-04-06T08:00:00", "bash", "/home/user", "old-here"),
        entry("2026-04-06T12:00:00", "bash", "/other", "new-there"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    for (_, en) in &mut state.filter.group {
        *en = false;
    }

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("bash", "/home/user", ""),
    );
    // Pure recency new-first: new-there comes first
    assert_eq!(result[0].entry.cmd, "new-there");
    assert_eq!(result[1].entry.cmd, "old-here");
}

// ---------------------------------------------------------------------------
// apply_pipeline: soft-delete
// ---------------------------------------------------------------------------

#[test]
fn pipeline_soft_delete_filters_entry() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home/user", "git status"),
        entry("2026-04-06T11:00:00", "bash", "/home/user", "make"),
        entry("2026-04-06T12:00:00", "bash", "/home/user", "git push"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.delete_log.delete_batch(vec!["2026-04-06T11:00:00".to_string()]);

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 2);
    assert!(result.iter().all(|de| de.entry.cmd != "make"));
}

#[test]
fn pipeline_soft_delete_with_dedup_filters_all_occurrences() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home/user", "git status"),
        entry("2026-04-06T11:00:00", "bash", "/home/user", "make"),
        entry("2026-04-06T12:00:00", "bash", "/home/user", "git status"),
    ];
    let freq_map = build_frequency_map(&entries);
    let mut state = AppState::new();
    state.filter.dedup = true;
    // Simulate deleting both occurrences of "git status"
    state.delete_log.delete_batch(vec![
        "2026-04-06T10:00:00".to_string(),
        "2026-04-06T12:00:00".to_string(),
    ]);

    let mut cache = MockRepoResolver::new();

    let result = apply_pipeline(
        &entries, &FilterSpec::from(&state), &mut cache, &freq_map,
        &ctx("fish", "/nonexistent", ""),
    );
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].entry.cmd, "make");
}

#[test]
fn pipeline_waive_does_not_filter_piped() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home", "ls -la | head"),
        entry("2026-04-06T10:01:00", "bash", "/home", "ls -la"),
    ];
    let freq = build_frequency_map(&entries);
    let mut cache = MockRepoResolver::new();
    let mut state = AppState::new();
    state.session.waive_commands = vec!["ls".to_string()];

    let result = apply_pipeline(&entries, &FilterSpec::from(&state), &mut cache, &freq, &ctx("bash", "/home", ""));
    let cmds: Vec<&str> = result.iter().map(|d| d.entry.cmd.as_str()).collect();
    assert_eq!(cmds, vec!["ls -la | head"]);
}

// ---------------------------------------------------------------------------
// apply_pipeline: min_cmd_len filter
// ---------------------------------------------------------------------------

#[test]
fn pipeline_min_cmd_len_filters_short_single_word() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home/user/proj", "git status"),
        entry("2026-04-06T10:01:00", "bash", "/home/user/proj", "vi"),
        entry("2026-04-06T10:02:00", "bash", "/home/user/proj", "make"),
        entry("2026-04-06T10:03:00", "bash", "/home/user/proj", "gcc"),
    ];
    let freq = build_frequency_map(&entries);
    let mut cache = MockRepoResolver::new();
    let mut state = AppState::new();
    state.session.waive_min_cmd_len = 3; // hide single-word cmds of 3 chars or fewer

    let result = apply_pipeline(&entries, &FilterSpec::from(&state), &mut cache, &freq, &ctx("bash", "/home/user/proj", ""));
    let cmds: Vec<&str> = result.iter().map(|d| d.entry.cmd.as_str()).collect();
    // "git status" (multi-word) passes, "make" (4 chars > 3) passes, "vi" (2) and "gcc" (3) filtered
    assert_eq!(cmds, vec!["make", "git status"]);
}

// ---------------------------------------------------------------------------
// Bench (ignored by default): runs the pipeline against the real
// $HOME/.cmdlog.tsv to surface per-render latency on a realistic workload.
//
//   cargo test --release bench_fuzzy_pipeline -- --ignored --nocapture
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn bench_fuzzy_pipeline() {
    let path = std::env::var("CMDLOG_FILE")
        .unwrap_or_else(|_| format!("{}/.cmdlog.tsv", std::env::var("HOME").unwrap()));
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => { eprintln!("skip: no log at {}", path); return; }
    };
    let entries: Vec<LogEntry> = content.lines().filter_map(|line| {
        let parts: Vec<&str> = line.splitn(5, '\t').collect();
        if parts.len() != 5 { return None; }
        Some(LogEntry {
            date: parts[0].to_string(),
            shell: parts[1].to_string(),
            pwd: parts[2].to_string(),
            exit_code: parts[3].to_string(),
            cmd: parts[4].to_string(),
        })
    }).collect();
    eprintln!("\n--- bench: {} entries from {} ---", entries.len(), path);

    let freq_map = build_frequency_map(&entries);
    let mut cache = MockRepoResolver::new();
    let pctx = ctx("bash", "/", "");

    let mut bench = |label: &str, query: &str| {
        let mut state = AppState::new();
        state.search.search_input = query.to_string();
        let spec = FilterSpec::from(&state);

        // Warm up the matcher's DP tables.
        for _ in 0..3 {
            let _ = apply_pipeline(&entries, &spec, &mut cache, &freq_map, &pctx);
        }
        let iters = 100;
        let start = std::time::Instant::now();
        let mut last_len = 0;
        for _ in 0..iters {
            last_len = apply_pipeline(&entries, &spec, &mut cache, &freq_map, &pctx).len();
        }
        let elapsed = start.elapsed();
        eprintln!(
            "{:24} query={:?} results={:>5}  per-call={:?}",
            label, query, last_len, elapsed / iters,
        );
    };

    bench("empty query",          "");
    bench("short ASCII query",    "git");
    bench("medium ASCII query",   "git stash");
    bench("subsequence query",    "gst");
    bench("no-match query",       "xyzzzzz");

    // Head-to-head: pre-segmented needle (current code) vs re-segmenting the
    // needle inside the loop (the pre-optimization-#1 baseline). Times raw
    // scoring against each entry's cmd; isolates the segmentation cost so the
    // surrounding pipeline overhead doesn't drown out the signal.
    use nucleo_matcher::{Config, Matcher, Utf32Str};
    let cmds: Vec<String> = entries.iter().map(|e| e.cmd.clone()).collect();
    let needle = "gst";  // subsequence query — forces full DP path

    eprintln!("\n--- head-to-head: needle segmentation cost ---");

    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut hb: Vec<char> = Vec::new();

    // BEFORE: re-segment needle every iteration.
    let iters = 50;
    let start = std::time::Instant::now();
    for _ in 0..iters {
        let mut nb: Vec<char> = Vec::new();
        for cmd in &cmds {
            let n = Utf32Str::new(needle, &mut nb);
            let h = Utf32Str::new(cmd, &mut hb);
            let _ = matcher.fuzzy_match(h, n);
            nb.clear();
        }
    }
    let before = start.elapsed() / iters;

    // AFTER: segment needle once before the loop.
    let mut nb: Vec<char> = Vec::new();
    let n = Utf32Str::new(needle, &mut nb);
    let start = std::time::Instant::now();
    for _ in 0..iters {
        for cmd in &cmds {
            let h = Utf32Str::new(cmd, &mut hb);
            let _ = matcher.fuzzy_match(h, n);
        }
    }
    let after = start.elapsed() / iters;

    let saved = before.as_nanos().saturating_sub(after.as_nanos());
    let pct = (saved as f64 / before.as_nanos() as f64) * 100.0;
    eprintln!("before (in-loop segment): {:?}", before);
    eprintln!("after  (pre-segmented):   {:?}", after);
    eprintln!("saved: {} ns ({:.1}%) per render", saved, pct);
}

#[test]
fn pipeline_min_cmd_len_preserves_special_chars() {
    let entries = vec![
        entry("2026-04-06T10:00:00", "bash", "/home/user/proj", "ls"),
        entry("2026-04-06T10:01:00", "bash", "/home/user/proj", "ls; cd"),
    ];
    let freq = build_frequency_map(&entries);
    let mut cache = MockRepoResolver::new();
    let mut state = AppState::new();
    state.session.waive_min_cmd_len = 5;

    let result = apply_pipeline(&entries, &FilterSpec::from(&state), &mut cache, &freq, &ctx("bash", "/home/user/proj", ""));
    let cmds: Vec<&str> = result.iter().map(|d| d.entry.cmd.as_str()).collect();
    // "ls; cd" has special chars -> always shown. "ls" is 2 chars < 5 -> filtered.
    assert_eq!(cmds, vec!["ls; cd"]);
}

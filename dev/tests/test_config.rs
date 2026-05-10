use std::fs;
use std::path::PathBuf;

use cmdlog::tui::config::{default_inject_method, init_config, load_config, load_inject_method, save_config, doctor_config, IssueKind};
use cmdlog::tui::state::{
    AppState, GroupDimension, OrderBadge, OrderDimension, ShowColumn, TimeMode,
};

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cmdlog_test_config_{}_{}", std::process::id(), suffix
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// save_config
// ---------------------------------------------------------------------------

#[test]
fn save_config_creates_file() {
    let dir = tmp_dir("save_creates");
    let path = dir.join(".cmdlog.conf");
    let state = AppState::new();

    save_config(&path, &state).unwrap();
    assert!(path.exists());

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("[show]"));
    assert!(content.contains("[filter]"));
    assert!(content.contains("[order]"));
    assert!(content.contains("[group]"));

    cleanup(&dir);
}

#[test]
fn save_config_records_time_and_columns() {
    let dir = tmp_dir("save_enabled");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    // Enable time=date and shell
    let idx = state.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Time).unwrap();
    state.display.show_columns[idx].1 = true;
    state.display.time_mode = TimeMode::Date;
    let idx = state.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Shell).unwrap();
    state.display.show_columns[idx].1 = true;

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("time = \"date\""));
    assert!(content.contains("shell = true"));

    cleanup(&dir);
}

#[test]
fn save_config_records_time_age() {
    let dir = tmp_dir("save_time_age");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    let idx = state.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Time).unwrap();
    state.display.show_columns[idx].1 = true;
    state.display.time_mode = TimeMode::Age;

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("time = \"age\""));

    cleanup(&dir);
}

#[test]
fn save_config_records_time_off() {
    let dir = tmp_dir("save_time_off");
    let path = dir.join(".cmdlog.conf");
    let state = AppState::new(); // time disabled by default

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("time = \"off\""));

    cleanup(&dir);
}

#[test]
fn save_config_records_filters() {
    let dir = tmp_dir("save_filters");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    state.filter.filters[0].1 = true; // this_shell on
    state.filter.dedup = true;

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("this_shell = true"));
    assert!(content.contains("this_dir = false"));
    // dedup is now in [group] section
    assert!(content.contains("[group]"));
    assert!(content.contains("dedup = true"));

    cleanup(&dir);
}

#[test]
fn save_config_records_order() {
    let dir = tmp_dir("save_order");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    state.filter.order = vec![
        OrderBadge { dim: OrderDimension::Frequency, ascending: false },
        OrderBadge { dim: OrderDimension::Recency, ascending: true },
    ];

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("frequency = \"desc\""));
    assert!(content.contains("recency = \"asc\""));
    // Sequence should have frequency first
    assert!(content.contains("sequence = [\"frequency\", \"recency\"]"));

    cleanup(&dir);
}

#[test]
fn save_config_records_group() {
    let dir = tmp_dir("save_group");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    state.filter.group = vec![
        (GroupDimension::Repo, true),
        (GroupDimension::Dir, false),
        (GroupDimension::RelPath, true),
    ];

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("sequence = [\"repo\", \"abspath\", \"relpath\"]"));
    assert!(content.contains("abspath = false"));
    assert!(content.contains("repo = true"));
    assert!(content.contains("relpath = true"));

    cleanup(&dir);
}

#[test]
fn save_config_records_column_order() {
    let dir = tmp_dir("save_col_order");
    let path = dir.join(".cmdlog.conf");
    let mut state = AppState::new();
    // Swap first two columns
    state.display.show_columns.swap(0, 1);

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    // Shell should be first in order
    assert!(content.contains("order = [\"shell\", \"time\""));

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// load_config
// ---------------------------------------------------------------------------

#[test]
fn load_config_missing_file_returns_default() {
    let dir = tmp_dir("load_missing");
    let path = dir.join(".cmdlog.conf");
    let state = load_config(&path);
    assert_eq!(state.display.show_columns.len(), 6);
    assert!(state.display.show_columns.iter().all(|(_, e)| !*e));
    assert!(state.filter.filters.iter().all(|(_, e)| !*e));
    cleanup(&dir);
}

#[test]
fn load_config_roundtrip() {
    let dir = tmp_dir("roundtrip");
    let path = dir.join(".cmdlog.conf");

    let mut original = AppState::new();
    // Enable time=age, shell, count
    let idx = original.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Time).unwrap();
    original.display.show_columns[idx].1 = true;
    original.display.time_mode = TimeMode::Age;
    let idx = original.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Shell).unwrap();
    original.display.show_columns[idx].1 = true;
    let idx = original.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Count).unwrap();
    original.display.show_columns[idx].1 = true;
    // Swap: put shell first
    original.display.show_columns.swap(0, 1);

    original.filter.filters[1].1 = true; // this_dir
    original.filter.filters[3].1 = true; // today
    original.filter.order = vec![
        OrderBadge { dim: OrderDimension::Frequency, ascending: false },
        OrderBadge { dim: OrderDimension::Recency, ascending: true },
    ];
    original.filter.group = vec![
        (GroupDimension::RelPath, true),
        (GroupDimension::Repo, false),
        (GroupDimension::Dir, true),
    ];

    save_config(&path, &original).unwrap();
    let loaded = load_config(&path);

    // Verify show columns match (order and enabled state)
    assert_eq!(loaded.display.show_columns.len(), original.display.show_columns.len());
    for (i, (col, en)) in loaded.display.show_columns.iter().enumerate() {
        assert_eq!(*col, original.display.show_columns[i].0, "show col {} mismatch", i);
        assert_eq!(*en, original.display.show_columns[i].1, "show enabled {} mismatch", i);
    }
    assert_eq!(loaded.display.time_mode, TimeMode::Age);

    // Verify filters
    for (i, (_, en)) in loaded.filter.filters.iter().enumerate() {
        assert_eq!(*en, original.filter.filters[i].1, "filter {} mismatch", i);
    }

    // Verify order
    assert_eq!(loaded.filter.order.len(), original.filter.order.len());
    for (i, badge) in loaded.filter.order.iter().enumerate() {
        assert_eq!(badge.dim, original.filter.order[i].dim, "order dim {} mismatch", i);
        assert_eq!(badge.ascending, original.filter.order[i].ascending, "order asc {} mismatch", i);
    }

    // Verify group
    assert_eq!(loaded.filter.group.len(), original.filter.group.len());
    for (i, (dim, en)) in loaded.filter.group.iter().enumerate() {
        assert_eq!(*dim, original.filter.group[i].0, "group dim {} mismatch", i);
        assert_eq!(*en, original.filter.group[i].1, "group enabled {} mismatch", i);
    }

    cleanup(&dir);
}

#[test]
fn load_config_handles_partial_toml() {
    let dir = tmp_dir("load_partial_toml");
    let path = dir.join(".cmdlog.conf");
    // Only show section
    fs::write(&path, "[show]\ntime = \"date\"\nshell = true\n").unwrap();

    let state = load_config(&path);
    assert!(state.display.is_time_date());
    assert!(state.display.is_show_enabled(ShowColumn::Shell));
    // Filters should be default (all off)
    assert!(state.filter.filters.iter().all(|(_, e)| !*e));

    cleanup(&dir);
}

#[test]
fn save_config_preserves_waive_section() {
    let dir = tmp_dir("save_waive");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[waive]\ncommands = [\"ls\", \"grep\", \"cat\"]\n").unwrap();
    let state = load_config(&path);
    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("[waive]"));
    assert!(content.contains("ls"));
    assert!(content.contains("grep"));
    assert!(content.contains("cat"));
    cleanup(&dir);
}

#[test]
fn load_config_waive_commands() {
    let dir = tmp_dir("load_waive");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[waive]\ncommands = [\"ls\", \"grep\", \"sort\"]\n").unwrap();
    let state = load_config(&path);
    assert_eq!(state.session.waive_commands.len(), 3);
    assert!(state.session.waive_commands.contains(&"ls".to_string()));
    assert!(state.session.waive_commands.contains(&"grep".to_string()));
    assert!(state.session.waive_commands.contains(&"sort".to_string()));
    cleanup(&dir);
}

#[test]
fn load_config_waive_min_cmd_len() {
    let dir = tmp_dir("load_waive_mincmd");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[waive]\nmin_cmd_len = 4\ncommands = [\"ls\"]\n").unwrap();
    let state = load_config(&path);
    assert_eq!(state.session.waive_min_cmd_len, 4);
    assert_eq!(state.session.waive_commands, vec!["ls"]);
    cleanup(&dir);
}

#[test]
fn load_config_no_waive_section_returns_empty() {
    let dir = tmp_dir("load_no_waive");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[show]\ntime = \"date\"\n").unwrap();
    let state = load_config(&path);
    assert!(state.session.waive_commands.is_empty());
    cleanup(&dir);
}

#[test]
fn init_config_copies_default_when_missing() {
    let dir = tmp_dir("init_missing");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = dir.join("default.conf");

    fs::write(&default_path, "[waive]\ncommands = [\"ls\", \"grep\"]\n").unwrap();

    init_config(&config_path, &default_path);
    assert!(config_path.exists());

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[waive]"));
    assert!(content.contains("ls"));

    cleanup(&dir);
}

#[test]
fn init_config_does_not_overwrite_existing() {
    let dir = tmp_dir("init_existing");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = dir.join("default.conf");

    fs::write(&config_path, "[show]\ntime = \"date\"\n").unwrap();
    fs::write(&default_path, "[waive]\ncommands = [\"ls\"]\n").unwrap();

    init_config(&config_path, &default_path);

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("time = \"date\""));
    assert!(!content.contains("[waive]"));

    cleanup(&dir);
}

#[test]
fn save_config_preserves_comments() {
    let dir = tmp_dir("save_comments");
    let path = dir.join(".cmdlog.conf");
    let original = "\
# Top-level comment
# about the config

# Show section docs
[show]
order = [\"time\", \"shell\", \"path\", \"repo\", \"count\"]
time = \"age\"
shell = false
dir = \"off\"
repo = true
count = true

# Filter docs
[filter]
this_shell = true
this_dir = false
this_repo = false
today = false
piped = false

# Waive docs - should be untouched
[waive]
commands = [\"ls\", \"grep\"]
";
    fs::write(&path, original).unwrap();

    // Load, toggle one value, save
    let mut state = load_config(&path);
    // Flip shell to true
    let idx = state.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Shell).unwrap();
    state.display.show_columns[idx].1 = true;

    save_config(&path, &state).unwrap();
    let content = fs::read_to_string(&path).unwrap();

    // Comments preserved
    assert!(content.contains("# Top-level comment"));
    assert!(content.contains("# Show section docs"));
    assert!(content.contains("# Filter docs"));
    assert!(content.contains("# Waive docs - should be untouched"));

    // Changed value updated
    assert!(content.contains("shell = true"));

    // Waive untouched
    assert!(content.contains("commands = [\"ls\", \"grep\"]"));

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// load_inject_method
// ---------------------------------------------------------------------------

#[test]
fn inject_defaults_when_no_config_file() {
    let dir = tmp_dir("inject_no_file");
    let path = dir.join(".cmdlog.conf");
    assert_eq!(load_inject_method(&path, "bash"), "readline");
    assert_eq!(load_inject_method(&path, "zsh"), "print-z");
    assert_eq!(load_inject_method(&path, "tcsh"), default_inject_method("tcsh"));
    assert_eq!(load_inject_method(&path, "fish"), "history");
    cleanup(&dir);
}

#[test]
fn inject_defaults_when_no_inject_section() {
    let dir = tmp_dir("inject_no_section");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[show]\ntime = \"date\"\n").unwrap();
    let tcsh_default = default_inject_method("tcsh");
    assert_eq!(load_inject_method(&path, "bash"), "readline");
    assert_eq!(load_inject_method(&path, "zsh"), "print-z");
    assert_eq!(load_inject_method(&path, "tcsh"), tcsh_default);
    // Verify defaults were written back to config
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("[inject]"));
    assert!(content.contains("bash = \"readline\""));
    assert!(content.contains("zsh = \"print-z\""));
    assert!(content.contains(&format!("tcsh = \"{}\"", tcsh_default)));
    cleanup(&dir);
}

#[test]
fn inject_reads_bash_readline() {
    let dir = tmp_dir("inject_bash_readline");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nbash = \"readline\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "bash"), "readline");
    cleanup(&dir);
}

#[test]
fn inject_reads_bash_tiocsti() {
    let dir = tmp_dir("inject_bash_tiocsti");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nbash = \"tiocsti\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "bash"), "tiocsti");
    cleanup(&dir);
}

#[test]
fn inject_reads_bash_history() {
    let dir = tmp_dir("inject_bash_history");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nbash = \"history\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "bash"), "history");
    cleanup(&dir);
}

#[test]
fn inject_reads_zsh_print_z() {
    let dir = tmp_dir("inject_zsh_printz");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nzsh = \"print-z\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "zsh"), "print-z");
    cleanup(&dir);
}

#[test]
fn inject_reads_zsh_tiocsti() {
    let dir = tmp_dir("inject_zsh_tiocsti");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nzsh = \"tiocsti\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "zsh"), "tiocsti");
    cleanup(&dir);
}

#[test]
fn inject_reads_zsh_history() {
    let dir = tmp_dir("inject_zsh_history");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nzsh = \"history\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "zsh"), "history");
    cleanup(&dir);
}

#[test]
fn inject_reads_tcsh_tiocsti() {
    let dir = tmp_dir("inject_tcsh_tiocsti");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\ntcsh = \"tiocsti\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "tcsh"), "tiocsti");
    cleanup(&dir);
}

#[test]
fn inject_reads_tcsh_history() {
    let dir = tmp_dir("inject_tcsh_history");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\ntcsh = \"history\"\n").unwrap();
    assert_eq!(load_inject_method(&path, "tcsh"), "history");
    cleanup(&dir);
}

#[test]
fn inject_partial_section_uses_defaults_and_writes_back() {
    let dir = tmp_dir("inject_partial");
    let path = dir.join(".cmdlog.conf");
    fs::write(&path, "[inject]\nbash = \"tiocsti\"\n").unwrap();
    let tcsh_default = default_inject_method("tcsh");
    assert_eq!(load_inject_method(&path, "bash"), "tiocsti");
    assert_eq!(load_inject_method(&path, "zsh"), "print-z");
    assert_eq!(load_inject_method(&path, "tcsh"), tcsh_default);
    // bash was already set, zsh/tcsh should be written back
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("bash = \"tiocsti\""));
    assert!(content.contains("zsh = \"print-z\""));
    assert!(content.contains(&format!("tcsh = \"{}\"", tcsh_default)));
    cleanup(&dir);
}

#[test]
fn inject_all_shells_configured() {
    let dir = tmp_dir("inject_all");
    let path = dir.join(".cmdlog.conf");
    let original = "[inject]\nbash = \"history\"\nzsh = \"tiocsti\"\ntcsh = \"history\"\n";
    fs::write(&path, original).unwrap();
    assert_eq!(load_inject_method(&path, "bash"), "history");
    assert_eq!(load_inject_method(&path, "zsh"), "tiocsti");
    assert_eq!(load_inject_method(&path, "tcsh"), "history");
    // Nothing should be modified when all values exist
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// doctor_config — catches drift between config and app code
// ---------------------------------------------------------------------------

#[test]
fn default_conf_doctors_cleanly() {
    let dir = tmp_dir("default_conf_doctor");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    init_config(&config_path, &default_path);
    let issues = doctor_config(&config_path, &default_path, "bash");
    assert!(
        issues.is_empty(),
        "default.conf has doctor issues: {:?}",
        issues.iter().map(|i| format!("[{}] {} = {:?}", i.section, i.key, i.value)).collect::<Vec<_>>(),
    );
    // Also verify it loads without issues
    let state = load_config(&config_path);
    assert_eq!(state.display.show_columns.len(), ShowColumn::all_default_order().len());
    cleanup(&dir);
}

#[test]
fn doctor_creates_missing_config() {
    let dir = tmp_dir("doctor_creates");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    assert!(!config_path.exists());
    let issues = doctor_config(&config_path, &default_path, "bash");
    assert!(config_path.exists());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, IssueKind::FileCreated);
    assert!(!issues[0].kind.is_hard());
    // Created file should match default.conf
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[show]"));
    assert!(content.contains("[inject]"));
    cleanup(&dir);
}

#[test]
fn doctor_regenerates_unparseable_config() {
    let dir = tmp_dir("doctor_regen");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    let garbage = "this is [[[ not valid toml %%%";
    fs::write(&config_path, garbage).unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, IssueKind::FileRegenerated);
    assert!(issues[0].kind.is_hard());
    // Backup should contain original garbage
    let bak = config_path.with_extension("conf.bak");
    assert!(bak.exists());
    assert_eq!(fs::read_to_string(&bak).unwrap(), garbage);
    // Config should now be valid (from default.conf)
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[show]"));
    cleanup(&dir);
}

#[test]
fn doctor_fixes_wrong_type_string_as_int() {
    let dir = tmp_dir("doctor_wrong_type");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    fs::write(&config_path, "[show]\ntime = 123\n").unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    let type_issues: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::TypeFixed).collect();
    assert!(!type_issues.is_empty(), "should detect wrong type for show.time");
    assert_eq!(type_issues[0].section, "show");
    assert_eq!(type_issues[0].key, "time");
    // After fix, the file should have a string value for time
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("time = \"age\""));
    cleanup(&dir);
}

#[test]
fn doctor_fixes_wrong_type_bool_as_string() {
    let dir = tmp_dir("doctor_bool_str");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    fs::write(&config_path, "[filter]\nthis_shell = \"yes\"\n").unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    let type_issues: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::TypeFixed).collect();
    assert!(!type_issues.is_empty(), "should detect wrong type for filter.this_shell");
    assert_eq!(type_issues[0].section, "filter");
    assert_eq!(type_issues[0].key, "this_shell");
    // After fix, the file should have a bool value
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("this_shell = false"));
    cleanup(&dir);
}

#[test]
fn doctor_fills_missing_section() {
    let dir = tmp_dir("doctor_fill_section");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    fs::write(&config_path, "[show]\ntime = \"age\"\n").unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    let filled: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::SectionFilled).collect();
    // filter, order, group, waive, inject should all be filled
    assert!(filled.len() >= 4, "expected at least 4 sections filled, got {}", filled.len());
    // All should be soft issues
    assert!(filled.iter().all(|i| !i.kind.is_hard()));
    // File should now contain the filled sections
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[filter]"));
    assert!(content.contains("[order]"));
    assert!(content.contains("[inject]"));
    cleanup(&dir);
}

#[test]
fn doctor_fills_missing_key() {
    let dir = tmp_dir("doctor_fill_key");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    // Show section with only time — other keys like shell, dir, repo should be filled
    fs::write(&config_path, "[show]\ntime = \"age\"\n[filter]\nthis_shell = false\n[order]\nrecency = \"asc\"\n[group]\ndedup = true\n[waive]\ncommands = [\"ls\"]\n[inject]\nbash = \"readline\"\n").unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    let filled: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::KeyFilled).collect();
    assert!(!filled.is_empty(), "expected some missing keys to be filled");
    // Verify specific missing keys were filled
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("dir = "));
    assert!(content.contains("repo = "));
    cleanup(&dir);
}

#[test]
fn doctor_preserves_valid_custom_values() {
    let dir = tmp_dir("doctor_preserve");
    let config_path = dir.join(".cmdlog.conf");
    let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../default.conf");
    // Custom values that differ from defaults
    fs::write(&config_path, "[show]\ntime = \"date\"\nshell = true\ndir = \"abspath\"\nrepo = false\ncount = false\nexit_code = true\norder = [\"time\", \"shell\", \"path\", \"repo\", \"count\", \"exit\"]\n").unwrap();
    let issues = doctor_config(&config_path, &default_path, "bash");
    // No type issues should exist — all values are valid types
    let type_issues: Vec<_> = issues.iter().filter(|i| i.kind == IssueKind::TypeFixed).collect();
    assert!(type_issues.is_empty(), "should not fix valid typed values");
    // Custom values should be preserved
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("time = \"date\""));
    assert!(content.contains("shell = true"));
    assert!(content.contains("dir = \"abspath\""));
    cleanup(&dir);
}

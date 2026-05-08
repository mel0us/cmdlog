//! Config file save/load for TUI settings (~/.cmdlog.conf).
//!
//! TOML format:
//!   [show]
//!   order = ["time", "shell", "path", "repo", "count"]
//!   time = "age"       # "off", "date", or "age"
//!   dir = "abspath"    # "off", "abspath", or "relpath"
//!   shell = true
//!   ...
//!
//!   [filter]
//!   this_shell = false
//!   ...
//!
//!   [order]
//!   sequence = ["recency", "frequency"]
//!   recency = "asc"
//!   frequency = "desc"
//!
//!   [group]
//!   sequence = ["abspath", "repo", "relpath"]
//!   abspath = true
//!   repo = true
//!   relpath = true
//!
//! TOML-only format. No legacy support.

mod schema;
mod hydrate;
pub mod validate;
pub mod save;
pub mod inject;

use std::fs;
use std::path::Path;

use toml_edit::DocumentMut;

use crate::tui::state::AppState;

use self::hydrate::apply_toml;
use self::schema::ConfigFile;
use self::validate::{build_schema, toml_type_name, validate_fields};

// ---------------------------------------------------------------------------
// Public re-exports
// ---------------------------------------------------------------------------

pub use self::validate::{ConfigIssue, IssueKind};
pub use self::save::save_config;
pub use self::inject::{
    init_config, default_inject_method, valid_inject_methods,
    save_inject_method, load_inject_method,
};

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

/// Load TUI settings from a TOML config file.
/// Returns defaults if file is missing or malformed.
pub fn load_config(path: &Path) -> AppState {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return AppState::new(),
    };

    if let Ok(cfg) = toml::from_str::<ConfigFile>(&content) {
        return apply_toml(cfg);
    }

    AppState::new()
}

// ---------------------------------------------------------------------------
// Validate + Load (single pass)
// ---------------------------------------------------------------------------

/// Validate config and load state in one pass. Single toml_edit parse + single
/// serde parse. Returns `(AppState, issues)` -- caller checks issues before using state.
pub fn validate_and_load(content: &str, shell: &str) -> (AppState, Vec<ConfigIssue>) {
    let mut doc: DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            return (AppState::new(), vec![ConfigIssue::new(
                IssueKind::InvalidValue, "file", "parse", &e.to_string(), &[], "",
            )]);
        }
    };

    let mut issues: Vec<ConfigIssue> = Vec::new();

    let cfg: ConfigFile = match toml::from_str(content) {
        Ok(c) => c,
        Err(e) => {
            return (AppState::new(), vec![ConfigIssue::new(
                IssueKind::InvalidValue, "file", "parse", &e.to_string(), &[], "",
            )]);
        }
    };

    validate_fields(&cfg, &mut doc, shell, false, &mut issues);

    (apply_toml(cfg), issues)
}

// ---------------------------------------------------------------------------
// Doctor
// ---------------------------------------------------------------------------

/// Check and repair the config file (phases 1-5). Run manually via `cmdlog doctor`.
pub fn doctor_config(config_path: &Path, default_path: &Path, shell: &str) -> Vec<ConfigIssue> {
    // -- Phase 1: File existence -------------------------------------------------
    if !config_path.exists() {
        init_config(config_path, default_path);
        return vec![ConfigIssue::new(
            IssueKind::FileCreated, "file", "created", "", &[], "",
        )];
    }

    // -- Phase 2: TOML parse -----------------------------------------------------
    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            return vec![ConfigIssue::new(
                IssueKind::FileRegenerated, "file", "read", &e.to_string(), &[], "",
            )];
        }
    };

    let mut doc: DocumentMut = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            let bak = config_path.with_extension("conf.bak");
            let _ = fs::copy(config_path, &bak);
            if default_path.exists() {
                let _ = fs::copy(default_path, config_path);
            }
            return vec![ConfigIssue::new(
                IssueKind::FileRegenerated, "file", "parse", &e.to_string(), &[], "",
            )];
        }
    };

    let mut all_issues: Vec<ConfigIssue> = Vec::new();

    // -- Phase 3: Type repair ----------------------------------------------------
    let default_content = fs::read_to_string(default_path).unwrap_or_default();
    let default_doc: DocumentMut = default_content.parse().unwrap_or_default();
    let schema = build_schema(&default_doc);

    for (section, key, entry) in &schema {
        if let Some(table) = doc.get(section.as_str()).and_then(|v| v.as_table()) {
            if let Some(item) = table.get(key.as_str()) {
                let actual_type = toml_type_name(item);
                if actual_type != entry.expected_type {
                    let old_value = format!("{}", item);
                    doc[section.as_str()][key.as_str()] = entry.default_item.clone();
                    let mut issue = ConfigIssue::new(
                        IssueKind::TypeFixed, section, key,
                        &old_value,
                        &[entry.expected_type], &format!("{}", entry.default_item),
                    );
                    issue.hint = format!("was {} ({}), expected {}", old_value, actual_type, entry.expected_type);
                    all_issues.push(issue);
                }
            }
        }
    }

    // -- Phase 4: Fill missing sections/keys -------------------------------------
    for (section, item) in default_doc.iter() {
        if let Some(default_table) = item.as_table() {
            if doc.get(section).and_then(|v| v.as_table()).is_none() {
                doc[section] = item.clone();
                all_issues.push(ConfigIssue::new(
                    IssueKind::SectionFilled, section, "", "", &[], "",
                ));
            } else {
                for (key, default_val) in default_table.iter() {
                    let has_key = doc.get(section)
                        .and_then(|v| v.as_table())
                        .map(|t| t.contains_key(key))
                        .unwrap_or(false);
                    if !has_key {
                        doc[section][key] = default_val.clone();
                        all_issues.push(ConfigIssue::new(
                            IssueKind::KeyFilled, section, key, "",
                            &[], &format!("{}", default_val),
                        ));
                    }
                }
            }
        }
    }

    // -- Phase 5: Value validation (with auto-fix) -------------------------------
    let repaired_content = doc.to_string();
    let cfg: ConfigFile = match toml::from_str(&repaired_content) {
        Ok(c) => c,
        Err(e) => {
            return vec![ConfigIssue::new(
                IssueKind::InvalidValue, "file", "parse", &e.to_string(), &[], "",
            )];
        }
    };

    validate_fields(&cfg, &mut doc, shell, true, &mut all_issues);

    // -- Write back if any phase mutated the document ----------------------------
    let has_mutations = all_issues.iter().any(|i| matches!(
        i.kind,
        IssueKind::TypeFixed | IssueKind::KeyFilled | IssueKind::SectionFilled | IssueKind::EnumFixed
    ));
    if has_mutations {
        let _ = fs::write(config_path, doc.to_string());
    }

    all_issues
}

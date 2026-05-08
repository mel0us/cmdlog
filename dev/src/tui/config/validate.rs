//! Config validation infrastructure: issue types, schema, field validation.

use toml_edit::{DocumentMut, Item, value};

use crate::tui::state::ShowColumn;

use super::inject::{default_inject_method, is_tiocsti_available, valid_inject_methods};
use super::schema::ConfigFile;

// ---------------------------------------------------------------------------
// Issue types
// ---------------------------------------------------------------------------

/// Classification of a config issue found by `doctor_config`.
#[derive(Debug, Clone, PartialEq)]
pub enum IssueKind {
    FileCreated,      // soft -- config was missing, created from defaults
    FileRegenerated,  // hard -- TOML unparseable, backed up + regenerated
    TypeFixed,        // hard -- wrong type, replaced with default
    KeyFilled,        // soft -- missing key, added default
    SectionFilled,    // soft -- missing section, added defaults
    EnumFixed,        // hard -- invalid enum, auto-fixed
    UnknownKey,       // hard -- typo or obsolete key
    UnknownSection,   // hard -- unknown [section]
    InvalidValue,     // hard -- unfixable value (e.g. inject method)
}

impl IssueKind {
    /// Whether this issue should cause exit code 1 (requires user attention).
    pub fn is_hard(&self) -> bool {
        !matches!(self, IssueKind::FileCreated | IssueKind::KeyFilled | IssueKind::SectionFilled)
    }
}

/// A config issue found by `doctor_config`.
#[derive(Debug)]
pub struct ConfigIssue {
    pub kind: IssueKind,
    pub section: String,
    pub key: String,
    pub value: String,
    pub valid: Vec<String>,
    pub fixed_to: String,
    pub hint: String,
}

impl ConfigIssue {
    pub fn new(kind: IssueKind, section: &str, key: &str, value: &str, valid: &[&str], fixed_to: &str) -> Self {
        ConfigIssue {
            kind,
            section: section.to_string(),
            key: key.to_string(),
            value: value.to_string(),
            valid: valid.iter().map(|s| s.to_string()).collect(),
            fixed_to: fixed_to.to_string(),
            hint: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// Return the TOML type name of a `toml_edit::Item` value.
pub(super) fn toml_type_name(item: &Item) -> &'static str {
    match item {
        Item::Value(v) => match v {
            toml_edit::Value::String(_) => "string",
            toml_edit::Value::Integer(_) => "integer",
            toml_edit::Value::Float(_) => "float",
            toml_edit::Value::Boolean(_) => "bool",
            toml_edit::Value::Datetime(_) => "datetime",
            toml_edit::Value::Array(_) => "array",
            toml_edit::Value::InlineTable(_) => "table",
        },
        Item::Table(_) => "table",
        Item::ArrayOfTables(_) => "array_of_tables",
        Item::None => "none",
    }
}

/// Schema entry: expected type + default value for a config key.
pub(super) struct SchemaEntry {
    pub expected_type: &'static str,
    pub default_item: Item,
}

/// Build a schema map from the parsed default.conf document.
pub(super) fn build_schema(default_doc: &DocumentMut) -> Vec<(String, String, SchemaEntry)> {
    let mut schema = Vec::new();
    for (section, item) in default_doc.iter() {
        if let Some(table) = item.as_table() {
            for (key, val) in table.iter() {
                schema.push((
                    section.to_string(),
                    key.to_string(),
                    SchemaEntry {
                        expected_type: toml_type_name(val),
                        default_item: val.clone(),
                    },
                ));
            }
        }
    }
    schema
}

// ---------------------------------------------------------------------------
// Known keys + valid values
// ---------------------------------------------------------------------------

// Known keys per section -- shared by validate_and_load and doctor_config.
pub(super) const SHOW_KEYS: &[&str] = &["order", "time", "shell", "dir", "repo", "count", "exit_code"];
pub(super) const FILTER_KEYS: &[&str] = &["this_shell", "this_dir", "this_repo", "today", "operator", "exit_code"];
pub(super) const ORDER_KEYS: &[&str] = &["sequence", "recency", "frequency"];
pub(super) const GROUP_KEYS: &[&str] = &["sequence", "abspath", "repo", "relpath", "dedup"];
pub(super) const WAIVE_KEYS: &[&str] = &["commands", "min_cmd_len"];
pub(super) const INJECT_KEYS: &[&str] = &["bash", "zsh", "tcsh"];
pub(super) const KNOWN_SECTIONS: &[&str] = &["show", "filter", "order", "group", "waive", "inject"];

// Valid values per field -- shared by validate_fields.
const V_TIME: &[&str] = &["off", "date", "age"];
const V_DIR: &[&str] = &["off", "abspath", "relpath"];
const V_OP: &[&str] = &["off", "piped", "chained"];
const V_EC: &[&str] = &["off", "success", "failure"];
const V_ASC: &[&str] = &["asc", "desc"];
const V_ORDER_DIMS: &[&str] = &["recency", "frequency"];
const V_GROUP_DIMS: &[&str] = &["repo", "abspath", "relpath"];

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn push_unknown_keys(doc: &DocumentMut, section: &str, known: &[&str], issues: &mut Vec<ConfigIssue>) {
    if let Some(table) = doc.get(section).and_then(|v| v.as_table()) {
        for key in table.iter().map(|(k, _)| k) {
            if !known.contains(&key) {
                issues.push(ConfigIssue::new(
                    IssueKind::UnknownKey, section, key, "(unknown key)", known, "",
                ));
            }
        }
    }
}

fn push_unknown_sections(doc: &DocumentMut, issues: &mut Vec<ConfigIssue>) {
    for (key, _) in doc.iter() {
        if !KNOWN_SECTIONS.contains(&key) {
            issues.push(ConfigIssue::new(
                IssueKind::UnknownSection, "(root)", key, "(unknown section)", KNOWN_SECTIONS, "",
            ));
        }
    }
}

/// Validate inject method for a shell. Always report-only (no auto-fix).
fn validate_inject_method(cfg: &ConfigFile, shell: &str, issues: &mut Vec<ConfigIssue>) {
    let inject_method = cfg.inject.as_ref()
        .and_then(|i| match shell {
            "bash" => i.bash.as_deref(),
            "zsh" => i.zsh.as_deref(),
            "tcsh" => i.tcsh.as_deref(),
            _ => None,
        })
        .unwrap_or_else(|| default_inject_method(shell));
    let valid_inject = valid_inject_methods(shell);
    if !valid_inject.contains(&inject_method) {
        if inject_method == "tiocsti" && !is_tiocsti_available() {
            let mut issue = ConfigIssue::new(
                IssueKind::InvalidValue, "inject", shell, "tiocsti", &valid_inject, "",
            );
            issue.hint = "TIOCSTI blocked by kernel (dev.tty.legacy_tiocsti=0). Run: sysctl -w dev.tty.legacy_tiocsti=1".to_string();
            issues.push(issue);
        } else {
            issues.push(ConfigIssue::new(
                IssueKind::InvalidValue, "inject", shell, inject_method, &valid_inject, "",
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Shared validation walk
// ---------------------------------------------------------------------------

/// Shared validation walk: unknown sections/keys, enum checks, sequence checks, inject.
/// When `fix` is false, issues are IssueKind::InvalidValue (report only).
/// When `fix` is true, enum violations are auto-fixed in `doc` and reported as IssueKind::EnumFixed.
pub(super) fn validate_fields(
    cfg: &ConfigFile,
    doc: &mut DocumentMut,
    shell: &str,
    fix: bool,
    issues: &mut Vec<ConfigIssue>,
) {
    macro_rules! check_enum {
        ($section:expr, $key:expr, $val:expr, $valid:expr, $fix_val:expr) => {
            if !$valid.contains(&$val.as_str()) {
                if fix {
                    doc[$section][$key] = value($fix_val);
                    issues.push(ConfigIssue::new(
                        IssueKind::EnumFixed, $section, $key, $val, $valid, $fix_val,
                    ));
                } else {
                    issues.push(ConfigIssue::new(
                        IssueKind::InvalidValue, $section, $key, $val, $valid, "",
                    ));
                }
            }
        };
    }
    macro_rules! check_seq {
        ($section:expr, $key:expr, $seq:expr, $valid:expr) => {
            for item in $seq {
                if !$valid.contains(&item.as_str()) {
                    issues.push(ConfigIssue::new(
                        IssueKind::InvalidValue, $section, $key, item, $valid, "",
                    ));
                }
            }
        };
    }

    push_unknown_sections(doc, issues);

    push_unknown_keys(doc, "show", SHOW_KEYS, issues);
    if let Some(show) = &cfg.show {
        let v_cols: Vec<&str> = ShowColumn::all_default_order().iter().map(|c| c.label()).collect();
        if let Some(t) = &show.time { check_enum!("show", "time", t, V_TIME, "age"); }
        if let Some(d) = &show.dir { check_enum!("show", "dir", d, V_DIR, "off"); }
        if let Some(o) = &show.order { check_seq!("show", "order", o, v_cols.as_slice()); }
    }

    push_unknown_keys(doc, "filter", FILTER_KEYS, issues);
    if let Some(filter) = &cfg.filter {
        if let Some(op) = &filter.operator {
            check_enum!("filter", "operator", op, V_OP, "off");
        }
        if let Some(ec) = &filter.exit_code {
            check_enum!("filter", "exit_code", ec, V_EC, "off");
        }
    }

    push_unknown_keys(doc, "order", ORDER_KEYS, issues);
    if let Some(order) = &cfg.order {
        if let Some(r) = &order.recency { check_enum!("order", "recency", r, V_ASC, "asc"); }
        if let Some(f) = &order.frequency { check_enum!("order", "frequency", f, V_ASC, "asc"); }
        if let Some(s) = &order.sequence { check_seq!("order", "sequence", s, V_ORDER_DIMS); }
    }

    push_unknown_keys(doc, "group", GROUP_KEYS, issues);
    if let Some(group) = &cfg.group {
        if let Some(s) = &group.sequence { check_seq!("group", "sequence", s, V_GROUP_DIMS); }
    }

    push_unknown_keys(doc, "waive", WAIVE_KEYS, issues);

    push_unknown_keys(doc, "inject", INJECT_KEYS, issues);
    validate_inject_method(cfg, shell, issues);
}

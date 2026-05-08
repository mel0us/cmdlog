//! Serialize AppState back to TOML config file using toml_edit.

use std::fs;
use std::path::Path;

use toml_edit::{Array, DocumentMut, Item, value};

use crate::tui::state::{
    AppState, DirMode, ExitFilterMode, FilterToggle, OperatorFilterMode, ShowColumn, TimeMode,
};

/// Save TUI settings to a TOML config file.
/// If the file already exists, preserves comments, formatting, and unchanged values.
pub fn save_config(path: &Path, state: &AppState) -> Result<(), String> {
    let mut doc: DocumentMut = fs::read_to_string(path)
        .ok()
        .and_then(|c| c.parse::<DocumentMut>().ok())
        .unwrap_or_default();

    // -- [show] ---------------------------------------------------------------
    ensure_table(&mut doc, "show");

    let order_arr = str_array(
        &state.display.show_columns.iter().map(|(c, _)| c.label().to_string()).collect::<Vec<_>>(),
    );
    doc["show"]["order"] = value(order_arr);

    let time_val = if !state.display.is_show_enabled(ShowColumn::Time) {
        "off"
    } else if state.display.time_mode == TimeMode::Date {
        "date"
    } else {
        "age"
    };
    doc["show"]["time"] = value(time_val);

    let dir_val = if !state.display.is_show_enabled(ShowColumn::Dir) {
        "off"
    } else if state.display.dir_mode == DirMode::AbsPath {
        "abspath"
    } else {
        "relpath"
    };
    doc["show"]["dir"] = value(dir_val);

    for (col, en) in &state.display.show_columns {
        match col {
            ShowColumn::Time | ShowColumn::Dir => {} // handled via time_val/dir_val above
            _ => { doc["show"][col.toml_key()] = value(*en); }
        }
    }

    // -- [filter] -------------------------------------------------------------
    ensure_table(&mut doc, "filter");
    for (f, en) in &state.filter.filters {
        if *f == FilterToggle::Operator {
            let val = if !en {
                "off"
            } else {
                match state.filter.operator_filter_mode {
                    OperatorFilterMode::Piped => "piped",
                    OperatorFilterMode::Chained => "chained",
                }
            };
            doc["filter"]["operator"] = value(val);
            continue;
        }
        if *f == FilterToggle::ExitCode {
            let val = if !en {
                "off"
            } else {
                match state.filter.exit_filter_mode {
                    ExitFilterMode::Success => "success",
                    ExitFilterMode::Failure => "failure",
                }
            };
            doc["filter"]["exit_code"] = value(val);
            continue;
        }
        doc["filter"][f.toml_key()] = value(*en);
    }

    // -- [order] --------------------------------------------------------------
    ensure_table(&mut doc, "order");
    let seq = str_array(
        &state.filter.order.iter().map(|b| b.dim.toml_key().to_string()).collect::<Vec<_>>(),
    );
    doc["order"]["sequence"] = value(seq);
    for b in &state.filter.order {
        let val = if b.ascending { "asc" } else { "desc" };
        doc["order"][b.dim.toml_key()] = value(val);
    }

    // -- [group] --------------------------------------------------------------
    ensure_table(&mut doc, "group");
    let group_seq = str_array(
        &state.filter.group.iter().map(|(d, _)| d.label().to_string()).collect::<Vec<_>>(),
    );
    doc["group"]["sequence"] = value(group_seq);
    for (dim, en) in &state.filter.group {
        doc["group"][dim.label()] = value(*en);
    }
    doc["group"]["dedup"] = value(state.filter.dedup);

    // -- [waive] --------------------------------------------------------------
    // Only touch waive if the state carries waive commands; otherwise leave
    // the existing section (including its comments and formatting) untouched.
    if !state.session.waive_commands.is_empty() {
        ensure_table(&mut doc, "waive");
        let arr = str_array(&state.session.waive_commands);
        doc["waive"]["commands"] = value(arr);
    }

    let content = doc.to_string();
    fs::write(path, content).map_err(|e| format!("Cannot write {}: {}", path.display(), e))
}

/// Ensure a top-level table exists without replacing an existing one.
fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if !doc.contains_table(key) {
        doc[key] = Item::Table(toml_edit::Table::new());
    }
}

/// Build a `toml_edit::Array` from a slice of strings.
fn str_array(items: &[String]) -> Array {
    let mut arr = Array::new();
    for s in items {
        arr.push(s.as_str());
    }
    arr
}

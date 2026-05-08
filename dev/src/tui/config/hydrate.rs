//! Convert parsed ConfigFile into AppState field values.

use crate::tui::state::{
    AppState, DirMode, ExitFilterMode, FilterToggle, GroupDimension,
    OperatorFilterMode, OrderBadge, OrderDimension, ShowColumn, TimeMode,
};

use super::schema::ConfigFile;

/// Apply a parsed ConfigFile to produce an AppState.
pub(super) fn apply_toml(cfg: ConfigFile) -> AppState {
    let mut state = AppState::new();

    if let Some(show) = cfg.show {
        // Build show_columns from order array
        if let Some(ref order) = show.order {
            let mut cols = Vec::new();
            for name in order {
                if let Some(col) = ShowColumn::from_label(name) {
                    let enabled = match col {
                        ShowColumn::Time => false, // set below from time field
                        ShowColumn::Dir => false,  // set below from dir field
                        ShowColumn::Shell => show.shell.unwrap_or(false),
                        ShowColumn::Repo => show.repo.unwrap_or(false),
                        ShowColumn::Count => show.count.unwrap_or(false),
                        ShowColumn::ExitCode => show.exit_code.unwrap_or(false),
                    };
                    cols.push((col, enabled));
                }
            }
            // Add any missing columns
            let present: Vec<ShowColumn> = cols.iter().map(|(c, _)| *c).collect();
            for col in ShowColumn::all_default_order() {
                if !present.contains(&col) {
                    cols.push((col, false));
                }
            }
            if !cols.is_empty() {
                state.display.show_columns = cols;
            }
        } else {
            // No order array -- apply enabled state to defaults
            for (col, en) in &mut state.display.show_columns {
                match col {
                    ShowColumn::Shell => *en = show.shell.unwrap_or(false),
                    ShowColumn::Repo => *en = show.repo.unwrap_or(false),
                    ShowColumn::Count => *en = show.count.unwrap_or(false),
                    ShowColumn::ExitCode => *en = show.exit_code.unwrap_or(false),
                    ShowColumn::Time | ShowColumn::Dir => {} // set below
                }
            }
        }

        // Time mode
        if let Some(ref time_val) = show.time {
            match time_val.as_str() {
                "date" => {
                    set_time_enabled(&mut state, true);
                    state.display.time_mode = TimeMode::Date;
                }
                "age" => {
                    set_time_enabled(&mut state, true);
                    state.display.time_mode = TimeMode::Age;
                }
                _ => {
                    set_time_enabled(&mut state, false);
                }
            }
        }

        // Dir mode
        if let Some(ref dir_val) = show.dir {
            match dir_val.as_str() {
                "abspath" => {
                    set_dir_enabled(&mut state, true);
                    state.display.dir_mode = DirMode::AbsPath;
                }
                "relpath" => {
                    set_dir_enabled(&mut state, true);
                    state.display.dir_mode = DirMode::RelPath;
                }
                _ => {
                    set_dir_enabled(&mut state, false);
                }
            }
        }
    }

    if let Some(filter) = cfg.filter {
        for (f, en) in &mut state.filter.filters {
            match f {
                FilterToggle::ThisShell => {
                    if let Some(v) = filter.this_shell { *en = v; }
                }
                FilterToggle::ThisDir => {
                    if let Some(v) = filter.this_dir { *en = v; }
                }
                FilterToggle::ThisRepo => {
                    if let Some(v) = filter.this_repo { *en = v; }
                }
                FilterToggle::Today => {
                    if let Some(v) = filter.today { *en = v; }
                }
                FilterToggle::Operator => {
                    if let Some(ref v) = filter.operator {
                        match v.as_str() {
                            "piped" => {
                                *en = true;
                                state.filter.operator_filter_mode = OperatorFilterMode::Piped;
                            }
                            "chained" => {
                                *en = true;
                                state.filter.operator_filter_mode = OperatorFilterMode::Chained;
                            }
                            _ => { *en = false; }
                        }
                    }
                }
                FilterToggle::ExitCode => {
                    if let Some(ref v) = filter.exit_code {
                        match v.as_str() {
                            "success" => {
                                *en = true;
                                state.filter.exit_filter_mode = ExitFilterMode::Success;
                            }
                            "failure" => {
                                *en = true;
                                state.filter.exit_filter_mode = ExitFilterMode::Failure;
                            }
                            _ => { *en = false; }
                        }
                    }
                }
            }
        }
    }

    if let Some(order) = cfg.order {
        if let Some(ref seq) = order.sequence {
            let mut badges = Vec::new();
            for name in seq {
                if let Some(dim) = OrderDimension::from_toml_key(name) {
                    let ascending = match dim {
                        OrderDimension::Recency => {
                            order.recency.as_deref().unwrap_or("asc") == "asc"
                        }
                        OrderDimension::Frequency => {
                            order.frequency.as_deref().unwrap_or("asc") == "asc"
                        }
                    };
                    badges.push(OrderBadge { dim, ascending });
                }
            }
            if !badges.is_empty() {
                state.filter.order = badges;
            }
        }
    }

    if let Some(group) = cfg.group {
        if let Some(ref seq) = group.sequence {
            let dims: Vec<(GroupDimension, bool)> = seq
                .iter()
                .filter_map(|s| {
                    let dim = GroupDimension::from_label(s)?;
                    let enabled = match dim {
                        GroupDimension::Dir => group.abspath.unwrap_or(true),
                        GroupDimension::Repo => group.repo.unwrap_or(true),
                        GroupDimension::RelPath => group.relpath.unwrap_or(true),
                    };
                    Some((dim, enabled))
                })
                .collect();
            if !dims.is_empty() {
                state.filter.group = dims;
            }
        }
        if let Some(v) = group.dedup {
            state.filter.dedup = v;
        }
    }

    if let Some(waive) = cfg.waive {
        if let Some(cmds) = waive.commands {
            state.session.waive_commands = cmds;
        }
        if let Some(n) = waive.min_cmd_len {
            state.session.waive_min_cmd_len = n;
        }
    }

    state
}

fn set_time_enabled(state: &mut AppState, enabled: bool) {
    for (col, en) in &mut state.display.show_columns {
        if *col == ShowColumn::Time {
            *en = enabled;
        }
    }
}

fn set_dir_enabled(state: &mut AppState, enabled: bool) {
    for (col, en) in &mut state.display.show_columns {
        if *col == ShowColumn::Dir {
            *en = enabled;
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers

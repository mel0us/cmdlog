use cmdlog::tui::state::*;

// ---------------------------------------------------------------------------
// AppState::new() defaults
// ---------------------------------------------------------------------------

#[test]
fn new_state_defaults() {
    let s = AppState::new();
    assert_eq!(s.nav.focus, FocusZone::List);
    assert_eq!(s.nav.focus_index, 0);
    assert_eq!(s.nav.selected_index, 0);
    assert_eq!(s.nav.scroll_offset, 0);
    assert!(!s.quit);
    assert!(!s.nav.has_navigated);
    assert!(s.exec_cmd.is_none());
    assert!(s.search.search_input.is_empty());
}

#[test]
fn new_state_all_columns_disabled() {
    let s = AppState::new();
    assert!(!s.display.is_show_enabled(ShowColumn::Time));
    assert!(!s.display.is_show_enabled(ShowColumn::Shell));
    assert!(!s.display.is_show_enabled(ShowColumn::Dir));
    assert!(!s.display.is_show_enabled(ShowColumn::Repo));
    assert!(!s.display.is_show_enabled(ShowColumn::Count));
    assert!(!s.display.is_show_enabled(ShowColumn::ExitCode));
}

#[test]
fn new_state_all_filters_disabled() {
    let s = AppState::new();
    assert!(!s.filter.is_filter_enabled(FilterToggle::ThisShell));
    assert!(!s.filter.is_filter_enabled(FilterToggle::ThisDir));
    assert!(!s.filter.is_filter_enabled(FilterToggle::ThisRepo));
    assert!(!s.filter.is_filter_enabled(FilterToggle::Today));
    assert!(!s.filter.dedup);
    assert!(!s.filter.is_filter_enabled(FilterToggle::Operator));
    assert!(!s.filter.is_filter_enabled(FilterToggle::ExitCode));
}

#[test]
fn new_state_order_defaults() {
    let s = AppState::new();
    assert_eq!(s.filter.order.len(), 2);
    assert_eq!(s.filter.order[0].dim, OrderDimension::Recency);
    assert!(s.filter.order[0].ascending);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Frequency);
    assert!(s.filter.order[1].ascending);
}

#[test]
fn new_state_group_defaults() {
    let s = AppState::new();
    assert_eq!(s.filter.group.len(), 3);
    assert_eq!(s.filter.group[0].0, GroupDimension::Dir);
    assert!(s.filter.group[0].1);
    assert_eq!(s.filter.group[1].0, GroupDimension::Repo);
    assert!(s.filter.group[1].1);
    assert_eq!(s.filter.group[2].0, GroupDimension::RelPath);
    assert!(s.filter.group[2].1);
}

#[test]
fn new_state_time_mode_default() {
    let s = AppState::new();
    assert_eq!(s.display.time_mode, TimeMode::Date);
    assert!(!s.display.is_time_date()); // Time column disabled
    assert!(!s.display.is_time_age());
}

// ---------------------------------------------------------------------------
// toggle_show
// ---------------------------------------------------------------------------

#[test]
fn toggle_show_enables_column() {
    let mut s = AppState::new();
    assert!(!s.display.is_show_enabled(ShowColumn::Shell));
    let idx = s.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Shell).unwrap();
    s.display.toggle_show(idx);
    assert!(s.display.is_show_enabled(ShowColumn::Shell));
}

#[test]
fn toggle_show_disables_column() {
    let mut s = AppState::new();
    let idx = s.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Shell).unwrap();
    s.display.toggle_show(idx); // enable
    s.display.toggle_show(idx); // disable
    assert!(!s.display.is_show_enabled(ShowColumn::Shell));
}

#[test]
fn toggle_show_time_cycles() {
    // Time cycles: off → date → age → off
    let mut s = AppState::new();
    let idx = s.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::Time).unwrap();

    // off → date
    s.display.toggle_show(idx);
    assert!(s.display.is_time_date());
    assert!(!s.display.is_time_age());

    // date → age
    s.display.toggle_show(idx);
    assert!(!s.display.is_time_date());
    assert!(s.display.is_time_age());

    // age → off
    s.display.toggle_show(idx);
    assert!(!s.display.is_time_date());
    assert!(!s.display.is_time_age());
    assert!(!s.display.is_show_enabled(ShowColumn::Time));
}

#[test]
fn toggle_show_out_of_bounds() {
    let mut s = AppState::new();
    s.display.toggle_show(999); // should not panic
}

#[test]
fn toggle_show_exit_code() {
    let mut s = AppState::new();
    assert!(!s.display.is_show_enabled(ShowColumn::ExitCode));
    let idx = s.display.show_columns.iter().position(|(c, _)| *c == ShowColumn::ExitCode).unwrap();
    s.display.toggle_show(idx);
    assert!(s.display.is_show_enabled(ShowColumn::ExitCode));
    s.display.toggle_show(idx);
    assert!(!s.display.is_show_enabled(ShowColumn::ExitCode));
}

// ---------------------------------------------------------------------------
// toggle_filter
// ---------------------------------------------------------------------------

#[test]
fn toggle_filter_basic() {
    let mut s = AppState::new();
    assert!(!s.filter.is_filter_enabled(FilterToggle::ThisDir));
    let idx = s.filter.filters.iter().position(|(f, _)| *f == FilterToggle::ThisDir).unwrap();
    s.filter.toggle_filter(idx);
    assert!(s.filter.is_filter_enabled(FilterToggle::ThisDir));
    s.filter.toggle_filter(idx);
    assert!(!s.filter.is_filter_enabled(FilterToggle::ThisDir));
}

#[test]
fn toggle_filter_out_of_bounds() {
    let mut s = AppState::new();
    s.filter.toggle_filter(999); // should not panic
}

// ---------------------------------------------------------------------------
// toggle_order_direction
// ---------------------------------------------------------------------------

#[test]
fn toggle_order_direction() {
    let mut s = AppState::new();
    assert!(s.filter.order[0].ascending);
    s.filter.toggle_order_direction(0);
    assert!(!s.filter.order[0].ascending);
    s.filter.toggle_order_direction(0);
    assert!(s.filter.order[0].ascending);
}

#[test]
fn toggle_order_direction_out_of_bounds() {
    let mut s = AppState::new();
    s.filter.toggle_order_direction(999); // should not panic
}

// ---------------------------------------------------------------------------
// toggle_group
// ---------------------------------------------------------------------------

#[test]
fn toggle_group_basic() {
    let mut s = AppState::new();
    assert!(s.filter.is_group_enabled(GroupDimension::Dir));
    s.filter.toggle_group(0);
    assert!(!s.filter.is_group_enabled(GroupDimension::Dir));
    s.filter.toggle_group(0);
    assert!(s.filter.is_group_enabled(GroupDimension::Dir));
}

#[test]
fn toggle_group_out_of_bounds() {
    let mut s = AppState::new();
    s.filter.toggle_group(999); // should not panic
}

// ---------------------------------------------------------------------------
// swap_show
// ---------------------------------------------------------------------------

#[test]
fn swap_show_right() {
    let mut s = AppState::new();
    let first = s.display.show_columns[0].0;
    let second = s.display.show_columns[1].0;
    s.display.swap_show(0, 1);
    assert_eq!(s.display.show_columns[0].0, second);
    assert_eq!(s.display.show_columns[1].0, first);
}

#[test]
fn swap_show_left() {
    let mut s = AppState::new();
    let first = s.display.show_columns[0].0;
    let second = s.display.show_columns[1].0;
    s.display.swap_show(1, -1);
    assert_eq!(s.display.show_columns[0].0, second);
    assert_eq!(s.display.show_columns[1].0, first);
}

#[test]
fn swap_show_boundary_no_panic() {
    let mut s = AppState::new();
    s.display.swap_show(0, -1); // left boundary
    let last = s.display.show_columns.len() - 1;
    s.display.swap_show(last, 1); // right boundary
}

// ---------------------------------------------------------------------------
// swap_order
// ---------------------------------------------------------------------------

#[test]
fn swap_order_swaps_badges() {
    let mut s = AppState::new();
    assert_eq!(s.filter.order[0].dim, OrderDimension::Recency);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Frequency);
    s.filter.swap_order(0, 1);
    assert_eq!(s.filter.order[0].dim, OrderDimension::Frequency);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Recency);
}

#[test]
fn swap_order_boundary() {
    let mut s = AppState::new();
    s.filter.swap_order(0, -1); // no-op
    assert_eq!(s.filter.order[0].dim, OrderDimension::Recency);
}

// ---------------------------------------------------------------------------
// swap_group
// ---------------------------------------------------------------------------

#[test]
fn swap_group_swaps_dimensions() {
    let mut s = AppState::new();
    assert_eq!(s.filter.group[0].0, GroupDimension::Dir);
    assert_eq!(s.filter.group[1].0, GroupDimension::Repo);
    s.filter.swap_group(0, 1);
    assert_eq!(s.filter.group[0].0, GroupDimension::Repo);
    assert_eq!(s.filter.group[1].0, GroupDimension::Dir);
}

#[test]
fn swap_group_boundary() {
    let mut s = AppState::new();
    s.filter.swap_group(0, -1); // no-op
    let last = s.filter.group.len() - 1;
    s.filter.swap_group(last, 1); // no-op
}

// ---------------------------------------------------------------------------
// next_focus
// ---------------------------------------------------------------------------

#[test]
fn next_focus_cycles() {
    let mut s = AppState::new();
    assert_eq!(s.nav.focus, FocusZone::List);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::Show);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::Filter);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::Group);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::Order);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::Search);
    s.next_focus();
    assert_eq!(s.nav.focus, FocusZone::List);
}

#[test]
fn next_focus_resets_focus_index() {
    let mut s = AppState::new();
    s.nav.focus_index = 5;
    s.next_focus();
    assert_eq!(s.nav.focus_index, 0);
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------

#[test]
fn show_column_labels() {
    assert_eq!(ShowColumn::Time.label(), "time");
    assert_eq!(ShowColumn::Shell.label(), "shell");
    assert_eq!(ShowColumn::Dir.label(), "path");
    assert_eq!(ShowColumn::Repo.label(), "repo");
    assert_eq!(ShowColumn::Count.label(), "count");
    assert_eq!(ShowColumn::ExitCode.label(), "exit");
}

#[test]
fn filter_toggle_labels() {
    assert_eq!(FilterToggle::ThisShell.label(), "this shell");
    assert_eq!(FilterToggle::ThisDir.label(), "pwd");
    assert_eq!(FilterToggle::ThisRepo.label(), "this repo");
    assert_eq!(FilterToggle::Today.label(), "today");
    assert_eq!(FilterToggle::Operator.label(), "operator");
    assert_eq!(FilterToggle::ExitCode.label(), "exit");
}

#[test]
fn order_dimension_labels() {
    assert_eq!(OrderDimension::Recency.label(true), "recency: new first");
    assert_eq!(OrderDimension::Recency.label(false), "recency: old first");
    assert_eq!(OrderDimension::Frequency.label(true), "frequency: most first");
    assert_eq!(OrderDimension::Frequency.label(false), "frequency: least first");
}

#[test]
fn group_dimension_labels() {
    assert_eq!(GroupDimension::Dir.label(), "abspath");
    assert_eq!(GroupDimension::Repo.label(), "repo");
    assert_eq!(GroupDimension::RelPath.label(), "relpath");
}

// ---------------------------------------------------------------------------
// Enum all() helpers
// ---------------------------------------------------------------------------

#[test]
fn show_column_all_default_order() {
    let all = ShowColumn::all_default_order();
    assert_eq!(all.len(), 6);
}

#[test]
fn filter_toggle_all() {
    let all = FilterToggle::all();
    assert_eq!(all.len(), 6);
}

#[test]
fn new_state_delete_defaults() {
    let s = AppState::new();
    assert!(!s.nav.pending_d);
    assert!(!s.delete_requested);
    assert!(!s.undo_requested);
    assert!(s.delete_log.is_empty());
    assert!(!s.delete_log.can_undo());
}

// --- visual mode ---

#[test]
fn new_state_visual_mode_off() {
    let s = AppState::new();
    assert!(!s.nav.visual_mode);
    assert!(s.nav.visual_anchor.is_none());
}

#[test]
fn visual_range_when_active() {
    let mut s = AppState::new();
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(3);
    s.nav.selected_index = 7;
    assert_eq!(s.nav.visual_range(), Some((3, 7)));
    // Reversed direction
    s.nav.selected_index = 1;
    assert_eq!(s.nav.visual_range(), Some((1, 3)));
}

#[test]
fn visual_range_when_inactive() {
    let s = AppState::new();
    assert_eq!(s.nav.visual_range(), None);
}

#[test]
fn exit_visual_mode_clears_state() {
    let mut s = AppState::new();
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(5);
    s.nav.exit_visual_mode();
    assert!(!s.nav.visual_mode);
    assert!(s.nav.visual_anchor.is_none());
}

#[test]
fn reset_selection_exits_visual_mode() {
    let mut s = AppState::new();
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(5);
    s.nav.selected_index = 10;
    s.nav.reset_selection();
    assert!(!s.nav.visual_mode);
    assert!(s.nav.visual_anchor.is_none());
    assert_eq!(s.nav.selected_index, 0);
}

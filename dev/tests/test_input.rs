use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use cmdlog::tui::input::handle_event;
use cmdlog::tui::state::{AppState, GroupDimension, FocusZone, OrderDimension, TimeMode};

fn key_event(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn mouse_event(kind: MouseEventKind, row: u16, col: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn mouse_click(row: u16, col: u16) -> Event {
    mouse_event(MouseEventKind::Down(MouseButton::Left), row, col)
}

// ---------------------------------------------------------------------------
// Global keys
// ---------------------------------------------------------------------------

#[test]
fn esc_quits() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Esc), &mut s, 10);
    assert!(s.quit);
}

#[test]
fn q_does_not_quit() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('q')), &mut s, 10);
    assert!(!s.quit);
}

#[test]
fn tab_cycles_focus() {
    let mut s = AppState::new();
    assert_eq!(s.nav.focus, FocusZone::List);
    handle_event(key_event(KeyCode::Tab), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Show);
    handle_event(key_event(KeyCode::Tab), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Filter);
}

// ---------------------------------------------------------------------------
// Search zone
// ---------------------------------------------------------------------------

#[test]
fn search_char_input() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    let refilter = handle_event(key_event(KeyCode::Char('g')), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.search.search_input, "g");

    let refilter = handle_event(key_event(KeyCode::Char('i')), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.search.search_input, "gi");
}

#[test]
fn search_backspace() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    s.search.search_input = "git".to_string();
    s.search.search_cursor = 3;
    let refilter = handle_event(key_event(KeyCode::Backspace), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.search.search_input, "gi");
    assert_eq!(s.search.search_cursor, 2);
}

#[test]
fn search_backspace_empty() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    let refilter = handle_event(key_event(KeyCode::Backspace), &mut s, 10);
    assert!(!refilter);
    assert!(s.search.search_input.is_empty());
}

#[test]
fn search_enter_moves_to_list() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    let refilter = handle_event(key_event(KeyCode::Enter), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::List);
}

#[test]
fn search_down_moves_to_list() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    let refilter = handle_event(key_event(KeyCode::Down), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::List);
}

#[test]
fn search_up_stays_in_search() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn list_up_at_top_moves_to_search() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 0;
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn list_k_at_top_stays_in_list() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 0;
    handle_event(key_event(KeyCode::Char('k')), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::List);
    assert_eq!(s.nav.selected_index, 0);
}

#[test]
fn slash_from_list_focuses_search() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('/')), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn slash_from_show_focuses_search() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    handle_event(key_event(KeyCode::Char('/')), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn slash_in_search_types_slash() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    handle_event(key_event(KeyCode::Char('/')), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Search);
    assert_eq!(s.search.search_input, "/");
}

#[test]
fn search_resets_selection() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    s.nav.selected_index = 5;
    s.nav.scroll_offset = 3;
    handle_event(key_event(KeyCode::Char('x')), &mut s, 10);
    assert_eq!(s.nav.selected_index, 0);
    assert_eq!(s.nav.scroll_offset, 0);
}

// ---------------------------------------------------------------------------
// List zone
// ---------------------------------------------------------------------------

#[test]
fn list_down_increments_selection() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Down), &mut s, 10);
    assert_eq!(s.nav.selected_index, 1);
}

#[test]
fn list_down_clamps_at_end() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 9;
    handle_event(key_event(KeyCode::Down), &mut s, 10);
    assert_eq!(s.nav.selected_index, 9);
}

#[test]
fn list_up_decrements_selection() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 5;
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.selected_index, 4);
}

#[test]
fn list_up_clamps_at_zero() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 0;
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.selected_index, 0);
}

#[test]
fn list_page_down() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::PageDown), &mut s, 50);
    assert_eq!(s.nav.selected_index, 20);
}

#[test]
fn list_page_down_clamps() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::PageDown), &mut s, 5);
    assert_eq!(s.nav.selected_index, 4); // entry_count - 1
}

#[test]
fn list_page_up() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 25;
    s.nav.scroll_offset = 10;
    handle_event(key_event(KeyCode::PageUp), &mut s, 50);
    assert_eq!(s.nav.selected_index, 5);
}

#[test]
fn list_home() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 15;
    s.nav.scroll_offset = 10;
    handle_event(key_event(KeyCode::Home), &mut s, 50);
    assert_eq!(s.nav.selected_index, 0);
    assert_eq!(s.nav.scroll_offset, 0);
}

#[test]
fn list_end() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::End), &mut s, 50);
    assert_eq!(s.nav.selected_index, 49);
}

#[test]
fn list_enter_sets_exec_cmd() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Enter), &mut s, 10);
    assert!(s.exec_cmd.is_some());
    assert_eq!(s.exec_cmd.unwrap(), ""); // empty string signals exec
}

#[test]
fn list_empty_entry_count_no_panic() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Down), &mut s, 0);
    handle_event(key_event(KeyCode::Up), &mut s, 0);
    handle_event(key_event(KeyCode::Enter), &mut s, 0);
    // No panic, no crash
}

// ---------------------------------------------------------------------------
// Show zone
// ---------------------------------------------------------------------------

#[test]
fn show_space_toggles() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 2; // Shell
    let refilter = handle_event(key_event(KeyCode::Char(' ')), &mut s, 10);
    assert!(!refilter); // show toggle doesn't need refilter
    assert!(s.display.show_columns[2].1); // toggled on
}

#[test]
fn show_left_right_swaps() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 1;
    let first = s.display.show_columns[1].0;
    let second = s.display.show_columns[2].0;
    handle_event(key_event(KeyCode::Right), &mut s, 10);
    assert_eq!(s.display.show_columns[1].0, second);
    assert_eq!(s.display.show_columns[2].0, first);
    assert_eq!(s.nav.focus_index, 2);
}

#[test]
fn show_up_down_navigates() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    handle_event(key_event(KeyCode::Down), &mut s, 10);
    assert_eq!(s.nav.focus_index, 1);
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.focus_index, 0);
}

// ---------------------------------------------------------------------------
// Filter zone
// ---------------------------------------------------------------------------

#[test]
fn filter_space_toggles_and_refilters() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Filter;
    s.nav.focus_index = 0; // ThisShell
    let refilter = handle_event(key_event(KeyCode::Char(' ')), &mut s, 10);
    assert!(refilter);
    assert!(s.filter.filters[0].1);
    assert_eq!(s.nav.selected_index, 0); // reset on filter toggle
}

#[test]
fn filter_navigation() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Filter;
    s.nav.focus_index = 0;
    handle_event(key_event(KeyCode::Right), &mut s, 10);
    assert_eq!(s.nav.focus_index, 1);
    handle_event(key_event(KeyCode::Left), &mut s, 10);
    assert_eq!(s.nav.focus_index, 0);
}

// ---------------------------------------------------------------------------
// Group zone
// ---------------------------------------------------------------------------

#[test]
fn group_space_toggles_and_refilters() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Group;
    s.nav.focus_index = 0;
    assert!(s.filter.group[0].1); // Dir enabled
    let refilter = handle_event(key_event(KeyCode::Char(' ')), &mut s, 10);
    assert!(refilter);
    assert!(!s.filter.group[0].1); // Dir disabled
}

#[test]
fn group_left_right_swaps_and_refilters() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Group;
    s.nav.focus_index = 0;
    let refilter = handle_event(key_event(KeyCode::Right), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.nav.focus_index, 1);
}

#[test]
fn group_up_down_navigates() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Group;
    s.nav.focus_index = 0;
    handle_event(key_event(KeyCode::Down), &mut s, 10);
    assert_eq!(s.nav.focus_index, 1);
}

// ---------------------------------------------------------------------------
// Order zone
// ---------------------------------------------------------------------------

#[test]
fn order_space_toggles_direction_and_refilters() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Order;
    s.nav.focus_index = 0;
    assert!(s.filter.order[0].ascending);
    let refilter = handle_event(key_event(KeyCode::Char(' ')), &mut s, 10);
    assert!(refilter);
    assert!(!s.filter.order[0].ascending);
}

#[test]
fn order_left_right_swaps_and_refilters() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Order;
    s.nav.focus_index = 0;
    let refilter = handle_event(key_event(KeyCode::Right), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.nav.focus_index, 1);
}

// ---------------------------------------------------------------------------
// Mouse events
// ---------------------------------------------------------------------------

#[test]
fn mouse_click_search_area() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(mouse_click(5, 10), &mut s, 10); // row 5 = search input
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn mouse_click_filter_row() {
    let mut s = AppState::new();
    handle_event(mouse_click(1, 10), &mut s, 10); // row 1 = filter
    assert_eq!(s.nav.focus, FocusZone::Filter);
}

#[test]
fn mouse_click_show_row() {
    let mut s = AppState::new();
    handle_event(mouse_click(0, 10), &mut s, 10); // row 0 = show
    assert_eq!(s.nav.focus, FocusZone::Show);
}

#[test]
fn mouse_click_group_row() {
    let mut s = AppState::new();
    handle_event(mouse_click(2, 10), &mut s, 10); // row 2 = group
    assert_eq!(s.nav.focus, FocusZone::Group);
}

#[test]
fn mouse_click_order_row() {
    let mut s = AppState::new();
    handle_event(mouse_click(3, 10), &mut s, 10); // row 3 = order
    assert_eq!(s.nav.focus, FocusZone::Order);
}

#[test]
fn mouse_click_list_area() {
    let mut s = AppState::new();
    handle_event(mouse_click(9, 10), &mut s, 10); // row 9 = list (>=7)
    assert_eq!(s.nav.focus, FocusZone::List);
    assert_eq!(s.nav.selected_index, 2); // scroll_offset(0) + (9-7) = 2
}

#[test]
fn mouse_click_list_clamps_to_count() {
    let mut s = AppState::new();
    handle_event(mouse_click(50, 10), &mut s, 3); // way past entry count
    assert_eq!(s.nav.selected_index, 2); // clamped to entry_count - 1
}

// ===========================================================================
// Mouse: Show row — arrow reorder + emoji toggle
// ===========================================================================

#[test]
fn show_mouse_click_right_arrow_swaps_right() {
    let mut s = AppState::new();
    let first = s.display.show_columns[0].0;
    let second = s.display.show_columns[1].0;
    handle_event(mouse_click(0, 21), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Show);
    assert_eq!(s.display.show_columns[0].0, second);
    assert_eq!(s.display.show_columns[1].0, first);
    assert_eq!(s.nav.focus_index, 1);
}

#[test]
fn show_mouse_click_left_arrow_swaps_left() {
    let mut s = AppState::new();
    let first = s.display.show_columns[0].0;
    let second = s.display.show_columns[1].0;
    handle_event(mouse_click(0, 24), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Show);
    assert_eq!(s.display.show_columns[0].0, second);
    assert_eq!(s.display.show_columns[1].0, first);
    assert_eq!(s.nav.focus_index, 0);
}

#[test]
fn show_mouse_click_label_does_not_toggle() {
    let mut s = AppState::new();
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 18), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::Show);
    assert_eq!(s.nav.focus_index, 0);
    assert!(!s.display.show_columns[0].1);
}

#[test]
fn show_mouse_click_emoji_toggles_visibility() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 16), &mut s, 10);
    assert!(s.display.is_time_date());
}

#[test]
fn show_mouse_click_time_label_does_not_toggle() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 18), &mut s, 10);
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 27), &mut s, 10);
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 22), &mut s, 10);
    assert!(!s.display.show_columns[0].1);
}

#[test]
fn show_mouse_click_age_checkbox_toggles() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    assert!(!s.display.show_columns[0].1);
    handle_event(mouse_click(0, 25), &mut s, 10);
    assert!(s.display.is_time_age());
}

#[test]
fn show_mouse_click_date_checkbox_deselects_date() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    s.display.show_columns[0].1 = true;
    s.display.time_mode = TimeMode::Date;
    handle_event(mouse_click(0, 16), &mut s, 10);
    assert!(!s.display.show_columns[0].1);
}

#[test]
fn show_mouse_click_age_checkbox_deselects_age() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Show;
    s.nav.focus_index = 0;
    s.display.show_columns[0].1 = true;
    s.display.time_mode = TimeMode::Age;
    handle_event(mouse_click(0, 25), &mut s, 10);
    assert!(!s.display.show_columns[0].1);
}

// ===========================================================================
// Mouse: Filter row
// ===========================================================================

#[test]
fn filter_mouse_click_this_shell() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(1, 18), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::Filter);
    assert_eq!(s.nav.focus_index, 0);
    assert!(!s.filter.filters[0].1);

    let refilter = handle_event(mouse_click(1, 18), &mut s, 10);
    assert!(refilter);
    assert!(s.filter.filters[0].1);
}

#[test]
fn filter_mouse_click_this_dir() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(1, 27), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::Filter);
    assert_eq!(s.nav.focus_index, 1);
    assert!(!s.filter.filters[1].1);
}

#[test]
fn filter_mouse_click_this_repo() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(1, 32), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::Filter);
    assert_eq!(s.nav.focus_index, 2);
    assert!(!s.filter.filters[2].1);
}

#[test]
fn filter_mouse_click_toggle_off() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Filter;
    s.nav.focus_index = 0;
    handle_event(mouse_click(1, 14), &mut s, 10);
    assert!(s.filter.filters[0].1);
    handle_event(mouse_click(1, 14), &mut s, 10);
    assert!(!s.filter.filters[0].1);
}

// ===========================================================================
// Mouse: Group row
// ===========================================================================

#[test]
fn group_mouse_click_right_arrow_swaps() {
    let mut s = AppState::new();
    assert_eq!(s.filter.group[0].0, GroupDimension::Dir);
    assert_eq!(s.filter.group[1].0, GroupDimension::Repo);
    let refilter = handle_event(mouse_click(2, 25), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.filter.group[0].0, GroupDimension::Repo);
    assert_eq!(s.filter.group[1].0, GroupDimension::Dir);
    assert_eq!(s.nav.focus_index, 1);
}

#[test]
fn group_mouse_click_left_arrow_swaps() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(2, 29), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.filter.group[0].0, GroupDimension::Repo);
    assert_eq!(s.filter.group[1].0, GroupDimension::Dir);
    assert_eq!(s.nav.focus_index, 0);
}

#[test]
fn group_mouse_click_label_just_focuses() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(2, 31), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::Group);
    assert_eq!(s.nav.focus_index, 1);
    assert_eq!(s.filter.group[0].0, GroupDimension::Dir);
    assert_eq!(s.filter.group[1].0, GroupDimension::Repo);
}

#[test]
fn group_mouse_click_label_second_click_toggles() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Group;
    s.nav.focus_index = 0;
    assert!(s.filter.group[0].1);
    let refilter = handle_event(mouse_click(2, 21), &mut s, 10);
    assert!(refilter);
    assert!(!s.filter.group[0].1);
}

#[test]
fn group_mouse_click_toggle_indicator() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Group;
    s.nav.focus_index = 0;
    assert!(s.filter.group[0].1);
    let refilter = handle_event(mouse_click(2, 17), &mut s, 10);
    assert!(refilter);
    assert!(!s.filter.group[0].1);
}

// ===========================================================================
// Mouse: Order row
// ===========================================================================

#[test]
fn order_mouse_click_right_arrow_swaps() {
    let mut s = AppState::new();
    assert_eq!(s.filter.order[0].dim, OrderDimension::Recency);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Frequency);
    let refilter = handle_event(mouse_click(3, 34), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.filter.order[0].dim, OrderDimension::Frequency);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Recency);
    assert_eq!(s.nav.focus_index, 1);
}

#[test]
fn order_mouse_click_left_arrow_swaps() {
    let mut s = AppState::new();
    let refilter = handle_event(mouse_click(3, 39), &mut s, 10);
    assert!(refilter);
    assert_eq!(s.filter.order[0].dim, OrderDimension::Frequency);
    assert_eq!(s.filter.order[1].dim, OrderDimension::Recency);
    assert_eq!(s.nav.focus_index, 0);
}

#[test]
fn order_mouse_first_click_label_does_not_toggle() {
    let mut s = AppState::new();
    assert!(s.filter.order[0].ascending);
    let refilter = handle_event(mouse_click(3, 20), &mut s, 10);
    assert!(!refilter);
    assert_eq!(s.nav.focus, FocusZone::Order);
    assert_eq!(s.nav.focus_index, 0);
    assert!(s.filter.order[0].ascending);
}

#[test]
fn order_mouse_second_click_label_toggles() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Order;
    s.nav.focus_index = 0;
    assert!(s.filter.order[0].ascending);
    let refilter = handle_event(mouse_click(3, 20), &mut s, 10);
    assert!(refilter);
    assert!(!s.filter.order[0].ascending);
}

// ===========================================================================
// Mouse: scroll
// ===========================================================================

fn mouse_scroll_up() -> Event {
    mouse_event(MouseEventKind::ScrollUp, 10, 10)
}

fn mouse_scroll_down() -> Event {
    mouse_event(MouseEventKind::ScrollDown, 10, 10)
}

#[test]
fn mouse_scroll_down_moves_viewport_not_selection() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 0;
    s.nav.scroll_offset = 0;
    handle_event(mouse_scroll_down(), &mut s, 100);
    assert_eq!(s.nav.scroll_offset, 3);
    assert_eq!(s.nav.selected_index, 0);
    assert_eq!(s.nav.focus, FocusZone::List);
}

#[test]
fn mouse_scroll_up_moves_viewport_not_selection() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::Search;
    s.nav.selected_index = 10;
    s.nav.scroll_offset = 10;
    handle_event(mouse_scroll_up(), &mut s, 100);
    assert_eq!(s.nav.scroll_offset, 7);
    assert_eq!(s.nav.selected_index, 10);
    assert_eq!(s.nav.focus, FocusZone::List);
}

#[test]
fn mouse_scroll_up_clamps_at_zero() {
    let mut s = AppState::new();
    s.nav.scroll_offset = 1;
    handle_event(mouse_scroll_up(), &mut s, 50);
    assert_eq!(s.nav.scroll_offset, 0);
}

#[test]
fn mouse_scroll_down_clamps_at_end() {
    let mut s = AppState::new();
    s.nav.scroll_offset = 48;
    handle_event(mouse_scroll_down(), &mut s, 50);
    assert_eq!(s.nav.scroll_offset, 49);
}

#[test]
fn mouse_scroll_down_no_entries_no_panic() {
    let mut s = AppState::new();
    handle_event(mouse_scroll_down(), &mut s, 0);
    assert_eq!(s.nav.scroll_offset, 0);
}

#[test]
fn mouse_scroll_clears_detail() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.detail_index = Some(5);
    handle_event(mouse_scroll_down(), &mut s, 100);
    assert!(s.nav.detail_index.is_none());
}

// ---------------------------------------------------------------------------
// Non-key/mouse events ignored
// ---------------------------------------------------------------------------

#[test]
fn resize_event_ignored() {
    let mut s = AppState::new();
    let refilter = handle_event(Event::Resize(80, 24), &mut s, 10);
    assert!(!refilter);
    assert!(!s.quit);
}

// ---------------------------------------------------------------------------
// dd delete sequence
// ---------------------------------------------------------------------------

#[test]
fn list_d_sets_pending_d() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    assert!(s.nav.pending_d);
    assert!(!s.delete_requested);
}

#[test]
fn list_dd_sets_delete_requested() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    assert!(!s.nav.pending_d);
    assert!(s.delete_requested);
}

#[test]
fn list_d_then_other_key_clears_pending() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    assert!(s.nav.pending_d);
    handle_event(key_event(KeyCode::Char('j')), &mut s, 10);
    assert!(!s.nav.pending_d);
    assert!(!s.delete_requested);
}

#[test]
fn list_u_requests_undo() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.delete_log.delete_batch(vec!["2026-01-01T00:00:00".to_string()]);
    handle_event(key_event(KeyCode::Char('u')), &mut s, 10);
    assert!(s.undo_requested);
}

#[test]
fn list_u_noop_when_stack_empty() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    handle_event(key_event(KeyCode::Char('u')), &mut s, 10);
    assert!(!s.undo_requested);
}

// ---------------------------------------------------------------------------
// Visual mode (Shift+V)
// ---------------------------------------------------------------------------

#[test]
fn shift_v_enters_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 3;
    handle_event(key_event(KeyCode::Char('V')), &mut s, 10);
    assert!(s.nav.visual_mode);
    assert_eq!(s.nav.visual_anchor, Some(3));
}

#[test]
fn shift_v_toggles_visual_mode_off() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(3);
    s.nav.selected_index = 5;
    handle_event(key_event(KeyCode::Char('V')), &mut s, 10);
    assert!(!s.nav.visual_mode);
    assert!(s.nav.visual_anchor.is_none());
}

#[test]
fn esc_quits_even_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(2);
    handle_event(key_event(KeyCode::Esc), &mut s, 10);
    assert!(s.quit);
}

#[test]
fn j_k_move_cursor_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 3;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(3);
    // Move down
    handle_event(key_event(KeyCode::Char('j')), &mut s, 10);
    assert_eq!(s.nav.selected_index, 4);
    assert_eq!(s.nav.visual_anchor, Some(3));
    assert!(s.nav.visual_mode);
    // Move up
    handle_event(key_event(KeyCode::Char('k')), &mut s, 10);
    assert_eq!(s.nav.selected_index, 3);
    assert_eq!(s.nav.visual_anchor, Some(3));
}

#[test]
fn enter_disabled_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    handle_event(key_event(KeyCode::Enter), &mut s, 10);
    assert!(s.exec_cmd.is_none());
}

#[test]
fn space_disabled_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    s.nav.detail_index = None;
    handle_event(key_event(KeyCode::Char(' ')), &mut s, 10);
    assert!(s.nav.detail_index.is_none());
}

#[test]
fn u_disabled_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    s.delete_log.delete_batch(vec!["date1".to_string()]);
    handle_event(key_event(KeyCode::Char('u')), &mut s, 10);
    assert!(!s.undo_requested);
}

#[test]
fn tab_cancels_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    handle_event(key_event(KeyCode::Tab), &mut s, 10);
    assert!(!s.nav.visual_mode);
}

#[test]
fn slash_cancels_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    handle_event(key_event(KeyCode::Char('/')), &mut s, 10);
    assert!(!s.nav.visual_mode);
    assert_eq!(s.nav.focus, FocusZone::Search);
}

#[test]
fn dd_sets_delete_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(2);
    s.nav.selected_index = 5;
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    handle_event(key_event(KeyCode::Char('d')), &mut s, 10);
    assert!(s.delete_requested);
    assert!(s.nav.visual_mode);
}

#[test]
fn up_at_top_does_not_leave_list_in_visual_mode() {
    let mut s = AppState::new();
    s.nav.focus = FocusZone::List;
    s.nav.selected_index = 0;
    s.nav.visual_mode = true;
    s.nav.visual_anchor = Some(0);
    handle_event(key_event(KeyCode::Up), &mut s, 10);
    assert_eq!(s.nav.focus, FocusZone::List);
    assert!(s.nav.visual_mode);
}

use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};

use crate::tui::badge::{self, RowLayout, SlotKind};
use crate::tui::state::{AppState, FilterToggle, FocusZone, ShowColumn};

const SCROLL_STEP: usize = 3;

/// Handle a crossterm event. Returns true if the display list needs re-filtering/sorting.
pub fn handle_event(event: Event, state: &mut AppState, entry_count: usize) -> bool {
    match event {
        Event::Key(key) => handle_key(key.code, key.modifiers, state, entry_count),
        Event::Mouse(mouse) => handle_mouse(mouse, state, entry_count),
        Event::Paste(text) => {
            if state.nav.focus == FocusZone::Search {
                state.search.search_input.insert_str(state.search.search_cursor, &text);
                state.search.search_cursor += text.len();
                on_search_changed(state);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

fn handle_key(code: KeyCode, modifiers: KeyModifiers, state: &mut AppState, entry_count: usize) -> bool {
    // Ctrl+C — quit cleanly (in raw mode, Ctrl+C is a key event, not SIGINT)
    if code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
        state.quit = true;
        return false;
    }

    // Global keys
    match code {
        KeyCode::Esc => {
            state.quit = true;
            return false;
        }
        KeyCode::Tab => {
            state.nav.exit_visual_mode();
            state.next_focus();
            return false;
        }
        KeyCode::BackTab => {
            state.nav.exit_visual_mode();
            state.prev_focus();
            return false;
        }
        KeyCode::Char('/') if state.nav.focus != FocusZone::Search => {
            state.nav.exit_visual_mode();
            state.nav.focus = FocusZone::Search;
            return false;
        }
        _ => {}
    }

    match state.nav.focus {
        FocusZone::Search => handle_search_key(code, modifiers, state),
        FocusZone::List => handle_list_key(code, state, entry_count),
        FocusZone::Show => handle_show_key(code, state),
        FocusZone::Filter => handle_filter_key(code, state),
        FocusZone::Group => handle_group_key(code, state),
        FocusZone::Order => handle_order_key(code, state),
    }
}

/// Hook invoked after the search input changes. Currently a no-op — fuzzy
/// scoring is done lazily inside `apply_pipeline` each render — but kept as
/// a named seam so future cached state (precomputed needle codepoints,
/// match-position highlighting, etc.) has an obvious place to land.
fn on_search_changed(_state: &mut AppState) {}

fn handle_search_key(code: KeyCode, modifiers: KeyModifiers, state: &mut AppState) -> bool {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);

    // Readline shortcuts (Ctrl+key)
    if ctrl {
        match code {
            KeyCode::Char('a') => {
                state.search.search_cursor = 0;
                return false;
            }
            KeyCode::Char('e') => {
                state.search.search_cursor = state.search.search_input.len();
                return false;
            }
            KeyCode::Char('u') => {
                let pos = state.search.search_cursor;
                state.search.search_input.drain(..pos);
                state.search.search_cursor = 0;
                on_search_changed(state);
                state.nav.reset_selection();
                return true;
            }
            KeyCode::Char('k') => {
                state.search.search_input.truncate(state.search.search_cursor);
                on_search_changed(state);
                state.nav.reset_selection();
                return true;
            }
            KeyCode::Char('w') => {
                let pos = state.search.search_cursor;
                if pos > 0 {
                    let before = &state.search.search_input[..pos];
                    let new_pos = before.trim_end().rfind(' ').map(|i| i + 1).unwrap_or(0);
                    state.search.search_input.drain(new_pos..pos);
                    state.search.search_cursor = new_pos;
                    on_search_changed(state);
                    state.nav.reset_selection();
                    return true;
                }
                return false;
            }
            _ => return false,
        }
    }

    match code {
        KeyCode::Char(c) => {
            state.search.search_input.insert(state.search.search_cursor, c);
            state.search.search_cursor += c.len_utf8();
            on_search_changed(state);
            state.nav.reset_selection();
            true
        }
        KeyCode::Backspace => {
            if state.search.search_cursor > 0 {
                let prev = state.search.search_input[..state.search.search_cursor]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                state.search.search_input.drain(prev..state.search.search_cursor);
                state.search.search_cursor = prev;
                on_search_changed(state);
                state.nav.reset_selection();
                true
            } else {
                false
            }
        }
        KeyCode::Delete => {
            if state.search.search_cursor < state.search.search_input.len() {
                let next = state.search.search_input[state.search.search_cursor..]
                    .char_indices().nth(1).map(|(i, _)| state.search.search_cursor + i)
                    .unwrap_or(state.search.search_input.len());
                state.search.search_input.drain(state.search.search_cursor..next);
                on_search_changed(state);
                state.nav.reset_selection();
                true
            } else {
                false
            }
        }
        KeyCode::Left => {
            if state.search.search_cursor > 0 {
                state.search.search_cursor = state.search.search_input[..state.search.search_cursor]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            }
            false
        }
        KeyCode::Right => {
            if state.search.search_cursor < state.search.search_input.len() {
                state.search.search_cursor = state.search.search_input[state.search.search_cursor..]
                    .char_indices().nth(1).map(|(i, _)| state.search.search_cursor + i)
                    .unwrap_or(state.search.search_input.len());
            }
            false
        }
        KeyCode::Home => {
            state.search.search_cursor = 0;
            false
        }
        KeyCode::End => {
            state.search.search_cursor = state.search.search_input.len();
            false
        }
        KeyCode::Enter | KeyCode::Down => {
            state.nav.focus = FocusZone::List;
            false
        }
        _ => false,
    }
}


fn handle_list_key(code: KeyCode, state: &mut AppState, entry_count: usize) -> bool {
    if entry_count == 0 {
        return false;
    }

    // Handle 'gg' sequence: if pending_g and we get another 'g', go to top
    if state.nav.pending_g {
        state.nav.pending_g = false;
        if code == KeyCode::Char('g') {
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.reset_selection();
            return false;
        }
        // Not 'g' — fall through to normal handling
    }

    // Handle 'dd' sequence: immediate delete (no confirm prompt)
    if state.nav.pending_d {
        state.nav.pending_d = false;
        if code == KeyCode::Char('d') {
            state.delete_requested = true;
            return false;
        }
    }

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.nav.has_navigated = true;
            if state.nav.selected_index > 0 {
                state.nav.detail_index = None;
                state.nav.selected_index -= 1;
                if state.nav.selected_index < state.nav.scroll_offset {
                    state.nav.scroll_offset = state.nav.selected_index;
                }
            } else if code == KeyCode::Up && !state.nav.visual_mode {
                // At top of list, Up moves focus to search bar (not in visual mode)
                state.nav.focus = FocusZone::Search;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.nav.has_navigated = true;
            if state.nav.selected_index + 1 < entry_count {
                state.nav.detail_index = None;
                state.nav.selected_index += 1;
            }
        }
        KeyCode::PageUp => {
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.selected_index = state.nav.selected_index.saturating_sub(20);
            state.nav.scroll_offset = state.nav.scroll_offset.saturating_sub(20);
        }
        KeyCode::PageDown => {
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.selected_index = (state.nav.selected_index + 20).min(entry_count - 1);
        }
        KeyCode::Home => {
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.reset_selection();
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.selected_index = entry_count - 1;
        }
        KeyCode::Char('g') => {
            state.nav.pending_g = true;
        }
        KeyCode::Char('d') => {
            if state.nav.visual_mode {
                state.delete_requested = true;
            } else {
                state.nav.pending_d = true;
            }
        }
        KeyCode::Char('V') => {
            if state.nav.visual_mode {
                state.nav.exit_visual_mode();
            } else {
                state.nav.visual_mode = true;
                state.nav.visual_anchor = Some(state.nav.selected_index);
            }
        }
        KeyCode::Char('u') => {
            if !state.nav.visual_mode {
                state.undo_requested = state.delete_log.can_undo();
            }
        }
        KeyCode::Char(' ') => {
            if !state.nav.visual_mode {
                state.nav.has_navigated = true;
                let idx = state.nav.selected_index;
                if state.nav.detail_index == Some(idx) {
                    state.nav.detail_index = None;
                } else {
                    state.nav.detail_index = Some(idx);
                }
            }
        }
        KeyCode::Char('r') if !state.nav.visual_mode => {
            state.filter.toggle_filter_by_kind(FilterToggle::ThisRepo);
            return true;
        }
        KeyCode::Char('p') if !state.nav.visual_mode => {
            state.filter.toggle_filter_by_kind(FilterToggle::ThisDir);
            return true;
        }
        KeyCode::Char('t') if !state.nav.visual_mode => {
            state.filter.toggle_filter_by_kind(FilterToggle::Today);
            return true;
        }
        KeyCode::Char('s') if !state.nav.visual_mode => {
            state.filter.toggle_exit_filter_success();
            return true;
        }
        KeyCode::Char('f') if !state.nav.visual_mode => {
            state.filter.toggle_exit_filter_failure();
            return true;
        }
        KeyCode::Enter => {
            if !state.nav.visual_mode {
                state.nav.has_navigated = true;
                state.exec_cmd = Some(String::new());
            }
        }
        _ => {}
    }
    false
}

fn handle_show_key(code: KeyCode, state: &mut AppState) -> bool {
    let count = state.display.show_columns.len();
    match code {
        KeyCode::Left => {
            state.display.swap_show(state.nav.focus_index, -1);
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            false
        }
        KeyCode::Right => {
            state.display.swap_show(state.nav.focus_index, 1);
            if state.nav.focus_index + 1 < count {
                state.nav.focus_index += 1;
            }
            false
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            state.display.toggle_show(state.nav.focus_index);
            false
        }
        KeyCode::Up => {
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            false
        }
        KeyCode::Down => {
            if state.nav.focus_index + 1 < count {
                state.nav.focus_index += 1;
            }
            false
        }
        _ => false,
    }
}

fn handle_filter_key(code: KeyCode, state: &mut AppState) -> bool {
    let count = state.filter.filters.len();
    match code {
        KeyCode::Char(' ') | KeyCode::Enter => {
            state.filter.toggle_filter(state.nav.focus_index);
            true
        }
        KeyCode::Left | KeyCode::Up => {
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            false
        }
        KeyCode::Right | KeyCode::Down => {
            if state.nav.focus_index + 1 < count {
                state.nav.focus_index += 1;
            }
            false
        }
        _ => false,
    }
}

fn handle_group_key(code: KeyCode, state: &mut AppState) -> bool {
    let dim_count = state.filter.group.len();
    let total_count = dim_count + 1; // dimensions + dedup
    let on_dedup = state.nav.focus_index == dim_count;
    match code {
        KeyCode::Left => {
            if on_dedup {
                // Move focus back to last dimension (no swap)
                if state.nav.focus_index > 0 {
                    state.nav.focus_index -= 1;
                }
                false
            } else {
                state.filter.swap_group(state.nav.focus_index, -1);
                if state.nav.focus_index > 0 {
                    state.nav.focus_index -= 1;
                }
                true
            }
        }
        KeyCode::Right => {
            if on_dedup {
                false
            } else if state.nav.focus_index + 1 < dim_count {
                state.filter.swap_group(state.nav.focus_index, 1);
                state.nav.focus_index += 1;
                true
            } else {
                // On last dimension — advance focus to dedup (no swap)
                state.nav.focus_index = dim_count;
                false
            }
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            if on_dedup {
                state.filter.dedup = !state.filter.dedup;
            } else {
                state.filter.toggle_group(state.nav.focus_index);
            }
            true
        }
        KeyCode::Up => {
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            false
        }
        KeyCode::Down => {
            if state.nav.focus_index + 1 < total_count {
                state.nav.focus_index += 1;
            }
            false
        }
        _ => false,
    }
}

fn handle_order_key(code: KeyCode, state: &mut AppState) -> bool {
    let count = state.filter.order.len();
    match code {
        KeyCode::Left => {
            state.filter.swap_order(state.nav.focus_index, -1);
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            true
        }
        KeyCode::Right => {
            state.filter.swap_order(state.nav.focus_index, 1);
            if state.nav.focus_index + 1 < count {
                state.nav.focus_index += 1;
            }
            true
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            state.filter.toggle_order_direction(state.nav.focus_index);
            true
        }
        KeyCode::Up => {
            if state.nav.focus_index > 0 {
                state.nav.focus_index -= 1;
            }
            false
        }
        KeyCode::Down => {
            if state.nav.focus_index + 1 < count {
                state.nav.focus_index += 1;
            }
            false
        }
        _ => false,
    }
}

fn handle_mouse(mouse: crossterm::event::MouseEvent, state: &mut AppState, entry_count: usize) -> bool {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Right | MouseButton::Middle)
        | MouseEventKind::Up(_)
        | MouseEventKind::Drag(_)
        | MouseEventKind::Moved => return false,
        MouseEventKind::Down(MouseButton::Left) => {
            let row = mouse.row as usize;
            let col = mouse.column as usize;

            // Row mapping:
            // 0: Show, 1: Filter, 2: Group, 3: Order
            // 4: search top rule, 5: search input, 6: search bottom rule
            // 7+: command list
            match row {
                0 => handle_badge_row_mouse(col, state, FocusZone::Show),
                1 => handle_badge_row_mouse(col, state, FocusZone::Filter),
                2 => handle_badge_row_mouse(col, state, FocusZone::Group),
                3 => handle_badge_row_mouse(col, state, FocusZone::Order),
                4 | 5 | 6 => {
                    state.nav.focus = FocusZone::Search;
                    false
                }
                r if r >= 7 => {
                    state.nav.focus = FocusZone::List;
                    state.nav.has_navigated = true;
                    state.nav.detail_index = None;
                    if entry_count > 0 {
                        let clicked_index = state.nav.scroll_offset + (r - 7);
                        state.nav.selected_index = clicked_index.min(entry_count.saturating_sub(1));
                    }
                    false
                }
                _ => false,
            }
        }
        MouseEventKind::ScrollUp => {
            state.nav.focus = FocusZone::List;
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            state.nav.scroll_offset = state.nav.scroll_offset.saturating_sub(SCROLL_STEP);
            false
        }
        MouseEventKind::ScrollDown => {
            state.nav.focus = FocusZone::List;
            state.nav.has_navigated = true;
            state.nav.detail_index = None;
            if entry_count > 0 {
                state.nav.scroll_offset = (state.nav.scroll_offset + SCROLL_STEP)
                    .min(entry_count.saturating_sub(1));
            }
            false
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Unified badge row mouse handler — uses shared RowLayout from badge.rs
// ---------------------------------------------------------------------------

fn handle_badge_row_mouse(col: usize, state: &mut AppState, zone: FocusZone) -> bool {
    let (prefix, badges) = match zone {
        FocusZone::Show => (badge::SHOW_PREFIX, badge::show_badges(state)),
        FocusZone::Filter => (badge::FILTER_PREFIX, badge::filter_badges(state)),
        FocusZone::Group => (badge::GROUP_PREFIX, badge::group_badges(state)),
        FocusZone::Order => (badge::ORDER_PREFIX, badge::order_badges(state)),
        _ => return false,
    };

    if col < prefix {
        state.nav.focus = zone;
        return false;
    }

    let was_focused = state.nav.focus == zone;
    let prev_focus_idx = state.nav.focus_index;
    let layout = RowLayout::build(prefix, &badges);

    state.nav.focus = zone;

    match layout.hit(col) {
        Some(kind) => apply_badge_action(state, zone, kind, was_focused, prev_focus_idx),
        None => false,
    }
}

/// Map a SlotKind hit to a state mutation, returning true if refilter is needed.
fn apply_badge_action(
    state: &mut AppState, zone: FocusZone,
    kind: &SlotKind, was_focused: bool, prev_focus_idx: usize,
) -> bool {
    match kind {
        SlotKind::LeftArrow(idx) => {
            state.nav.focus_index = *idx;
            match zone {
                FocusZone::Show => {
                    state.display.swap_show(*idx, -1);
                    if *idx > 0 { state.nav.focus_index = idx - 1; }
                }
                FocusZone::Group => {
                    state.filter.swap_group(*idx, -1);
                    if *idx > 0 { state.nav.focus_index = idx - 1; }
                    return true;
                }
                FocusZone::Order => {
                    state.filter.swap_order(*idx, -1);
                    if *idx > 0 { state.nav.focus_index = idx - 1; }
                    return true;
                }
                _ => {}
            }
            false
        }
        SlotKind::RightArrow(idx) => {
            state.nav.focus_index = *idx;
            match zone {
                FocusZone::Show => {
                    state.display.swap_show(*idx, 1);
                    if idx + 1 < state.display.show_columns.len() { state.nav.focus_index = idx + 1; }
                }
                FocusZone::Group => {
                    state.filter.swap_group(*idx, 1);
                    if idx + 1 < state.filter.group.len() { state.nav.focus_index = idx + 1; }
                    return true;
                }
                FocusZone::Order => {
                    state.filter.swap_order(*idx, 1);
                    if idx + 1 < state.filter.order.len() { state.nav.focus_index = idx + 1; }
                    return true;
                }
                _ => {}
            }
            false
        }
        SlotKind::Indicator(idx) => {
            state.nav.focus_index = *idx;
            match zone {
                FocusZone::Show => { state.display.toggle_show(*idx); }
                FocusZone::Filter => { state.filter.toggle_filter(*idx); return true; }
                FocusZone::Group => {
                    if *idx == state.filter.group.len() {
                        state.filter.dedup = !state.filter.dedup;
                    } else {
                        state.filter.toggle_group(*idx);
                    }
                    return true;
                }
                _ => {}
            }
            false
        }
        SlotKind::IndicatorSubA(idx) => {
            state.nav.focus_index = *idx;
            match zone {
                FocusZone::Show => {
                    let col = state.display.show_columns[*idx].0;
                    match col {
                        ShowColumn::Time => state.display.toggle_time_date(),
                        ShowColumn::Dir => state.display.toggle_dir_abspath(),
                        _ => {}
                    }
                }
                FocusZone::Filter => {
                    let filter = state.filter.filters[*idx].0;
                    match filter {
                        FilterToggle::Operator => state.filter.toggle_operator_filter_piped(),
                        FilterToggle::ExitCode => state.filter.toggle_exit_filter_success(),
                        _ => {}
                    }
                    return true;
                }
                _ => {}
            }
            false
        }
        SlotKind::IndicatorSubB(idx) => {
            state.nav.focus_index = *idx;
            match zone {
                FocusZone::Show => {
                    let col = state.display.show_columns[*idx].0;
                    match col {
                        ShowColumn::Time => state.display.toggle_time_age(),
                        ShowColumn::Dir => state.display.toggle_dir_relpath(),
                        _ => {}
                    }
                }
                FocusZone::Filter => {
                    let filter = state.filter.filters[*idx].0;
                    match filter {
                        FilterToggle::Operator => state.filter.toggle_operator_filter_chained(),
                        FilterToggle::ExitCode => state.filter.toggle_exit_filter_failure(),
                        _ => {}
                    }
                    return true;
                }
                _ => {}
            }
            false
        }
        SlotKind::Label(idx) | SlotKind::LabelSubA(idx) | SlotKind::LabelSubB(idx)
        | SlotKind::Separator(idx) => {
            state.nav.focus_index = *idx;
            // Second click on an already-focused badge toggles (for non-grouped simple badges)
            if was_focused && prev_focus_idx == *idx {
                match zone {
                    FocusZone::Filter => {
                        let is_grouped = matches!(state.filter.filters[*idx].0, FilterToggle::Operator | FilterToggle::ExitCode);
                        if !is_grouped {
                            state.filter.toggle_filter(*idx);
                            return true;
                        }
                    }
                    FocusZone::Group => {
                        if *idx == state.filter.group.len() {
                            state.filter.dedup = !state.filter.dedup;
                        } else {
                            state.filter.toggle_group(*idx);
                        }
                        return true;
                    }
                    FocusZone::Order => {
                        state.filter.toggle_order_direction(*idx);
                        return true;
                    }
                    _ => {}
                }
            }
            false
        }
        SlotKind::RowSeparator => {
            // Click on "| " before dedup — focus dedup
            state.nav.focus_index = state.filter.group.len();
            false
        }
        SlotKind::Gap | SlotKind::Prefix => false,
    }
}

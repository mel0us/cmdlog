use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use crate::tui::badge::{self, BadgeDesc, RowLayout, SlotKind};
use crate::tui::filter::{age_string, fuzzy_indices, DisplayEntry, NeedleBuf};
use crate::tui::state::{AppState, DirMode, FilterToggle, FocusZone, ShowColumn, TimeMode};

/// Display text for entries whose path matches the current working directory.
pub const PWD_DISPLAY: &str = "$PWD";

const CYAN: Color = Color::Indexed(73); // muted cyan (#5fafaf)

fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}
fn active() -> Style {
    Style::new().fg(CYAN).add_modifier(Modifier::BOLD)
}
fn highlight() -> Style {
    Style::new().fg(Color::Black).bg(CYAN)
}
fn visual_highlight() -> Style {
    Style::new().fg(Color::Black).bg(Color::Yellow)
}
fn normal() -> Style {
    Style::new().fg(Color::White)
}
fn label() -> Style {
    Style::new().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
}
/// Shorten a path with middle ellipsis, preserving first segment and end of last segment.
/// Preserves leading "/" for absolute paths, omits it for relative paths.
/// "/home/user/deep/dir/longname" → "/home/.../long...name"
/// ".build/LongConfigName"        → ".build/Long...Name"
fn shorten_path_middle(path: &str, max_width: usize) -> String {
    if path.len() <= max_width || max_width < 8 {
        return path.to_string();
    }
    let is_abs = path.starts_with('/');
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return path[..max_width].to_string();
    }
    let prefix = if is_abs { "/" } else { "" };
    let first = parts[0];
    let last = parts[parts.len() - 1];

    if parts.len() >= 3 {
        // Multi-segment: try "prefix first/.../last"
        let candidate = format!("{}{}/.../{}", prefix, first, last);
        if candidate.len() <= max_width {
            return candidate;
        }
        // Last component too long — middle-truncate it
        let overhead = prefix.len() + first.len() + 5; // "prefix first/.../"
        let avail = max_width.saturating_sub(overhead);
        if avail >= 7 {
            let end_len = avail * 2 / 3;
            let start_len = avail - end_len - 3;
            if start_len > 0 && end_len > 0 && last.len() > start_len + end_len + 3 {
                return format!(
                    "{}{}/.../{}{}{}", prefix, first,
                    &last[..start_len], "...", &last[last.len() - end_len..]
                );
            }
        }
    } else {
        // 1-2 segments: middle-truncate the long segment directly
        // e.g., ".build/VeryLongName" → ".build/Very...Name"
        if parts.len() == 2 {
            let short_part = parts[0];
            let long_part = parts[1];
            let overhead = prefix.len() + short_part.len() + 1; // "prefix short/"
            let avail = max_width.saturating_sub(overhead);
            if avail >= 7 && long_part.len() > avail {
                let end_len = avail * 2 / 3;
                let start_len = avail - end_len - 3;
                if start_len > 0 && end_len > 0 {
                    return format!(
                        "{}{}/{}{}{}", prefix, short_part,
                        &long_part[..start_len], "...", &long_part[long_part.len() - end_len..]
                    );
                }
            }
        }
    }

    // Fallback: truncate end
    format!("{}...", &path[..max_width.saturating_sub(3)])
}

fn shell_color(shell: &str) -> Style {
    match shell.trim() {
        "bash" => Style::new().fg(Color::Green),
        "zsh" => Style::new().fg(Color::Blue),
        "tcsh" | "csh" => Style::new().fg(Color::Yellow),
        _ => dim(),
    }
}

// ---------------------------------------------------------------------------
// Shell command syntax coloring
// ---------------------------------------------------------------------------

fn style_cmd() -> Style {
    Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)
}
fn style_flag() -> Style {
    Style::new().fg(Color::Green)
}
fn style_op() -> Style {
    Style::new().fg(Color::White).add_modifier(Modifier::BOLD)
}
fn style_string() -> Style {
    Style::new().fg(Color::Blue)
}
fn style_var() -> Style {
    Style::new().fg(CYAN)
}
fn style_redirect() -> Style {
    Style::new().fg(Color::DarkGray)
}
fn style_number() -> Style {
    Style::new().fg(Color::Red)
}
fn style_arg() -> Style {
    Style::new().fg(Color::Yellow)
}

/// Check if a token looks like a number (integer, float) or version string.
fn is_number_or_version(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // Must start with a digit
    let first = s.as_bytes()[0];
    if !first.is_ascii_digit() {
        return false;
    }
    // All chars must be digits or dots (covers 100, 3.14, 1.2.3)
    s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Style applied to chars that matched the active fuzzy needle. Adds bold
/// + underline + a vivid fg, layered on top of the existing syntax color.
fn fuzzy_overlay_style(base: Style) -> Style {
    base.fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED)
}

/// Re-emit `spans` with each char at a position in `match_chars` wearing
/// the fuzzy overlay style. Assumes the haystack is ASCII (codepoint index
/// == byte offset) — same assumption `colorize_cmd` already makes about
/// shell command text.
fn apply_match_highlight(
    spans: Vec<Span<'static>>,
    match_chars: &[u32],
) -> Vec<Span<'static>> {
    if match_chars.is_empty() {
        return spans;
    }
    let mut out: Vec<Span<'static>> = Vec::with_capacity(spans.len() + match_chars.len() * 2);
    let mut byte_pos: usize = 0;
    for span in spans {
        let text_len = span.content.len();
        let span_start = byte_pos;
        let span_end = byte_pos + text_len;
        let base_style = span.style;
        let mut local_cursor: usize = 0;
        for &m in match_chars {
            let m = m as usize;
            if m < span_start || m >= span_end {
                continue;
            }
            let local = m - span_start;
            if local > local_cursor {
                out.push(Span::styled(
                    span.content[local_cursor..local].to_string(),
                    base_style,
                ));
            }
            let next = local + 1; // ASCII assumption
            out.push(Span::styled(
                span.content[local..next].to_string(),
                fuzzy_overlay_style(base_style),
            ));
            local_cursor = next;
        }
        if local_cursor < text_len {
            out.push(Span::styled(
                span.content[local_cursor..].to_string(),
                base_style,
            ));
        }
        byte_pos = span_end;
    }
    out
}

/// Tokenize a shell command into styled spans (Rich-like pretty printing).
/// Uses byte-offset slicing (shell commands are ASCII) to avoid Vec<char> allocation.
fn colorize_cmd(cmd: &str) -> Vec<Span<'static>> {
    let b = cmd.as_bytes();
    let len = b.len();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut is_first_word = true;
    let mut i = 0;

    while i < len {
        let ch = b[i];

        // Whitespace
        if ch.is_ascii_whitespace() {
            let start = i;
            while i < len && b[i].is_ascii_whitespace() { i += 1; }
            spans.push(Span::raw(cmd[start..i].to_string()));
            continue;
        }

        // Quoted strings
        if ch == b'\'' || ch == b'"' {
            let start = i;
            i += 1;
            while i < len && b[i] != ch {
                if b[i] == b'\\' && ch == b'"' { i += 1; }
                i += 1;
            }
            if i < len { i += 1; }
            spans.push(Span::styled(cmd[start..i].to_string(), style_string()));
            is_first_word = false;
            continue;
        }

        // Env variables: $VAR, ${VAR}
        if ch == b'$' {
            let start = i;
            i += 1;
            if i < len && b[i] == b'{' {
                while i < len && b[i] != b'}' { i += 1; }
                if i < len { i += 1; }
            } else {
                while i < len && (b[i].is_ascii_alphanumeric() || b[i] == b'_') { i += 1; }
            }
            spans.push(Span::styled(cmd[start..i].to_string(), style_var()));
            is_first_word = false;
            continue;
        }

        // Operators: |, ||, &&, ;
        if ch == b'|' || ch == b';' || (ch == b'&' && i + 1 < len && b[i + 1] == b'&') {
            let start = i;
            if ch == b'|' && i + 1 < len && b[i + 1] == b'|' { i += 2; }
            else if ch == b'&' { i += 2; }
            else { i += 1; }
            spans.push(Span::styled(cmd[start..i].to_string(), style_op()));
            is_first_word = true;
            continue;
        }

        // Redirects: >, >>, 2>, 2>&1, <
        if ch == b'>' || ch == b'<' || (ch.is_ascii_digit() && i + 1 < len && b[i + 1] == b'>') {
            let start = i;
            if ch.is_ascii_digit() { i += 1; }
            i += 1;
            if i < len && (b[i] == b'>' || b[i] == b'&') {
                i += 1;
                if i < len && b[i].is_ascii_digit() { i += 1; }
            }
            spans.push(Span::styled(cmd[start..i].to_string(), style_redirect()));
            continue;
        }

        // Regular word
        let start = i;
        while i < len
            && !b[i].is_ascii_whitespace()
            && b[i] != b'\'' && b[i] != b'"' && b[i] != b'$'
            && b[i] != b'|' && b[i] != b';' && b[i] != b'>' && b[i] != b'<'
            && !(b[i] == b'&' && i + 1 < len && b[i + 1] == b'&')
        { i += 1; }
        let token = &cmd[start..i];

        let style = if is_first_word {
            is_first_word = false;
            style_cmd()
        } else if token.starts_with('-') {
            style_flag()
        } else if is_number_or_version(token) {
            style_number()
        } else {
            style_arg()
        };
        spans.push(Span::styled(token.to_string(), style));
    }

    spans
}

pub fn draw(frame: &mut Frame, state: &AppState, entries: &[DisplayEntry]) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // header (Show, Filter, Group, Order)
            Constraint::Length(3), // search bar (rule + input + rule)
            Constraint::Min(3),    // command list
            Constraint::Length(1), // footer
        ])
        .split(area);

    draw_header(frame, state, chunks[0]);
    draw_search(frame, state, chunks[1]);
    draw_list(frame, state, entries, chunks[2]);
    draw_footer(frame, state, entries.len(), chunks[3]);
}

/// Append a right-aligned description to a span line, filling with spaces.
fn append_right_desc(spans: &mut Vec<Span<'static>>, desc: &'static str, width: u16) {
    // Use char count for display width (Unicode symbols used here are all single-width)
    let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let desc_len = desc.chars().count();
    let total = width as usize;
    if content_width + desc_len + 1 < total {
        let pad = total - content_width - desc_len;
        spans.push(Span::raw(" ".repeat(pad)));
        // Colorize key bindings vs description text
        let key_style = Style::new().fg(CYAN).add_modifier(Modifier::BOLD);
        let desc_style = Style::new().fg(Color::DarkGray);
        let parts: Vec<&str> = desc.split("  ").filter(|p| !p.is_empty()).collect();
        for (i, part) in parts.iter().enumerate() {
            if let Some(space_pos) = find_key_end(part) {
                spans.push(Span::styled(&part[..space_pos], key_style));
                spans.push(Span::styled(&part[space_pos..], desc_style));
            } else {
                spans.push(Span::styled(*part, desc_style));
            }
            if i + 1 < parts.len() {
                spans.push(Span::styled("  ", desc_style));
            }
        }
    }
}

/// Find the end of the key binding prefix in a hint segment like "Space toggle" or "← → reorder".
fn find_key_end(s: &str) -> Option<usize> {
    const KEYS: &[&str] = &["← →", "Space", "Tab", "Enter", "Esc", "dd", "gg", "u"];
    for key in KEYS {
        if s.starts_with(key) {
            return Some(key.len());
        }
    }
    None
}

/// Convert a RowLayout + BadgeDesc list into styled Spans for rendering.
fn layout_to_spans<'a>(layout: &RowLayout, badges: &[BadgeDesc<'a>], prefix_text: &'static str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    // Build a map from badge index → BadgeDesc for quick lookup
    let badge_map: std::collections::HashMap<usize, &BadgeDesc> = badges.iter().map(|b| {
        let idx = match b {
            BadgeDesc::Normal { index, .. } | BadgeDesc::Grouped { index, .. }
            | BadgeDesc::Order { index, .. } | BadgeDesc::Fixed { index, .. } => *index,
        };
        (idx, b)
    }).collect();

    for slot in &layout.slots {
        match &slot.kind {
            SlotKind::Prefix => {
                spans.push(Span::styled(prefix_text, label()));
            }
            SlotKind::LeftArrow(i) => {
                let bd = badge_map[i];
                let is_focused = badge_is_focused(bd);
                let style = if is_focused { highlight() } else { dim() };
                let text = if matches!(bd, BadgeDesc::Order { .. }) { "[◀ " } else { "◀ " };
                spans.push(Span::styled(text, style));
            }
            SlotKind::RightArrow(i) => {
                let bd = badge_map[i];
                let is_focused = badge_is_focused(bd);
                let style = if is_focused { highlight() } else { dim() };
                let text = if matches!(bd, BadgeDesc::Order { .. }) { " ▶]" } else { " ▶" };
                spans.push(Span::styled(text, style));
            }
            SlotKind::Indicator(i) => {
                let bd = badge_map[i];
                let enabled = match bd {
                    BadgeDesc::Normal { enabled, .. } | BadgeDesc::Fixed { enabled, .. } => *enabled,
                    _ => false,
                };
                let text = if enabled { "☑ " } else { "☐ " };
                spans.push(Span::styled(text, highlight()));
            }
            SlotKind::IndicatorSubA(i) => {
                let a_on = match badge_map[i] {
                    BadgeDesc::Grouped { a_on, .. } => *a_on,
                    _ => false,
                };
                spans.push(Span::styled(if a_on { "☑ " } else { "☐ " }, highlight()));
            }
            SlotKind::IndicatorSubB(i) => {
                let b_on = match badge_map[i] {
                    BadgeDesc::Grouped { b_on, .. } => *b_on,
                    _ => false,
                };
                spans.push(Span::styled(if b_on { "☑ " } else { "☐ " }, highlight()));
            }
            SlotKind::Label(i) => {
                let bd = badge_map[i];
                let is_focused = badge_is_focused(bd);
                let enabled = match bd {
                    BadgeDesc::Normal { enabled, .. } | BadgeDesc::Fixed { enabled, .. } => *enabled,
                    BadgeDesc::Order { .. } => true,
                    BadgeDesc::Grouped { a_on, b_on, .. } => *a_on || *b_on,
                };
                let style = if is_focused { highlight() } else if enabled { active() } else { dim() };
                spans.push(Span::styled(badge_label_text(bd).to_string(), style));
            }
            SlotKind::LabelSubA(i) => {
                let text = match badge_map[i] {
                    BadgeDesc::Grouped { label_a, .. } => *label_a,
                    _ => "",
                };
                spans.push(Span::styled(text.to_string(), highlight()));
            }
            SlotKind::LabelSubB(i) => {
                let text = match badge_map[i] {
                    BadgeDesc::Grouped { label_b, .. } => *label_b,
                    _ => "",
                };
                spans.push(Span::styled(text.to_string(), highlight()));
            }
            SlotKind::Separator(_) => {
                spans.push(Span::styled(" | ", highlight()));
            }
            SlotKind::RowSeparator => {
                spans.push(Span::styled("| ", dim()));
            }
            SlotKind::Gap => {
                spans.push(Span::raw("  "));
            }
        }
    }
    spans
}

fn badge_is_focused(bd: &BadgeDesc) -> bool {
    match bd {
        BadgeDesc::Normal { is_focused, .. } | BadgeDesc::Grouped { is_focused, .. }
        | BadgeDesc::Order { is_focused, .. } | BadgeDesc::Fixed { is_focused, .. } => *is_focused,
    }
}

/// Get the visible label text for a badge slot's Label kind (unfocused grouped or normal).
fn badge_label_text<'a>(bd: &'a BadgeDesc<'a>) -> &'a str {
    match bd {
        BadgeDesc::Normal { label, .. } | BadgeDesc::Order { label, .. }
        | BadgeDesc::Fixed { label, .. } => label,
        BadgeDesc::Grouped { label_a, label_b, group_label, a_on, b_on, .. } => {
            if *a_on { label_a } else if *b_on { label_b } else { group_label }
        }
    }
}

fn draw_header(frame: &mut Frame, state: &AppState, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    // Show row
    let show_badges = badge::show_badges(state);
    let show_layout = RowLayout::build(badge::SHOW_PREFIX, &show_badges);
    let mut spans = layout_to_spans(&show_layout, &show_badges, "Show Columns: ");
    let show_desc = if state.nav.focus == FocusZone::Show {
        let (col, _) = state.display.show_columns[state.nav.focus_index];
        match col {
            ShowColumn::Time if state.display.is_time_date() => "← → reorder  Space cycle: date → age → off",
            ShowColumn::Time if state.display.is_time_age() => "← → reorder  Space cycle: age → off → date",
            ShowColumn::Time => "← → reorder  Space cycle: off → date → age",
            ShowColumn::Dir if state.display.is_dir_abspath() => "← → reorder  Space cycle: abspath → relpath → off",
            ShowColumn::Dir if state.display.is_dir_relpath() => "← → reorder  Space cycle: relpath → off → abspath",
            ShowColumn::Dir => "← → reorder  Space cycle: off → abspath → relpath",
            _ => "← → reorder  Space toggle on/off",
        }
    } else {
        "toggle column visibility"
    };
    append_right_desc(&mut spans, show_desc, area.width);
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[0]);

    // Filter row
    let filter_badges = badge::filter_badges(state);
    let filter_layout = RowLayout::build(badge::FILTER_PREFIX, &filter_badges);
    let mut spans = layout_to_spans(&filter_layout, &filter_badges, "Quick Filter: ");
    let piped_on = state.filter.is_operator_filter_piped();
    let chained_on = state.filter.is_operator_filter_chained();
    let success_on = state.filter.is_exit_filter_success();
    let failure_on = state.filter.is_exit_filter_failure();
    let filter_desc = if state.nav.focus == FocusZone::Filter {
        let (filter, _) = state.filter.filters[state.nav.focus_index];
        match filter {
            FilterToggle::ThisShell => "only commands from current shell",
            FilterToggle::ThisDir => "only commands from current directory",
            FilterToggle::ThisRepo => "only commands from current git repo",
            FilterToggle::Today => "only today's commands",
            FilterToggle::Operator if piped_on => "Space cycle: piped → chained → off",
            FilterToggle::Operator if chained_on => "Space cycle: chained → off → piped",
            FilterToggle::Operator => "Space cycle: off → piped → chained",
            FilterToggle::ExitCode if success_on => "Space cycle: success → failure → off",
            FilterToggle::ExitCode if failure_on => "Space cycle: failure → off → success",
            FilterToggle::ExitCode => "Space cycle: off → success → failure",
        }
    } else {
        "narrow results"
    };
    append_right_desc(&mut spans, filter_desc, area.width);
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[1]);

    // Group row
    let group_badges = badge::group_badges(state);
    let group_layout = RowLayout::build(badge::GROUP_PREFIX, &group_badges);
    let mut spans = layout_to_spans(&group_layout, &group_badges, "Context Group: ");
    let group_desc = if state.nav.focus == FocusZone::Group {
        if state.nav.focus_index == state.filter.group.len() {
            "Space toggle  collapse identical commands across all groups"
        } else {
            "← → reorder  Space toggle  group matching entries on top"
        }
    } else {
        "group current context on top"
    };
    append_right_desc(&mut spans, group_desc, area.width);
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[2]);

    // Order row
    let order_badges = badge::order_badges(state);
    let order_layout = RowLayout::build(badge::ORDER_PREFIX, &order_badges);
    let mut spans = layout_to_spans(&order_layout, &order_badges, "Sort Order: ");
    let order_desc = if state.nav.focus == FocusZone::Order {
        "← → set primary sort  Space flip direction"
    } else {
        "sort by time or frequency"
    };
    append_right_desc(&mut spans, order_desc, area.width);
    frame.render_widget(Paragraph::new(Line::from(spans)), rows[3]);
}

fn draw_search(frame: &mut Frame, state: &AppState, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let rule = "─".repeat(area.width as usize);
    let rule_style = if state.nav.focus == FocusZone::Search {
        Style::new().fg(CYAN)
    } else {
        dim()
    };
    frame.render_widget(Paragraph::new(Span::styled(rule.clone(), rule_style)), rows[0]);
    frame.render_widget(Paragraph::new(Span::styled(rule, rule_style)), rows[2]);

    let prompt_style = if state.nav.focus == FocusZone::Search {
        active()
    } else {
        dim()
    };
    let mut input_spans = vec![Span::styled("❯ ", prompt_style)];
    if state.nav.focus == FocusZone::Search {
        let pos = state.search.search_cursor;
        let input = &state.search.search_input;
        let before = &input[..pos];
        let cursor_char = input[pos..].chars().next();
        let after_pos = pos + cursor_char.map(|c| c.len_utf8()).unwrap_or(0);
        let after = &input[after_pos..];
        input_spans.push(Span::styled(before, normal()));
        input_spans.push(Span::styled(
            cursor_char.map(|c| c.to_string()).unwrap_or_else(|| " ".to_string()),
            Style::new().fg(Color::Black).bg(CYAN),
        ));
        input_spans.push(Span::styled(after, normal()));
    } else {
        input_spans.push(Span::styled(&state.search.search_input, normal()));
    };
    frame.render_widget(Paragraph::new(Line::from(input_spans)), rows[1]);
}

fn draw_list(frame: &mut Frame, state: &AppState, entries: &[DisplayEntry], area: Rect) {
    let visible_height = area.height as usize;
    let total = entries.len();

    if total == 0 {
        let msg = Paragraph::new(Span::styled("  No matching entries.", dim()));
        frame.render_widget(msg, area);
        return;
    }

    let start = state.nav.scroll_offset;
    let end = (start + visible_height).min(total);
    let visible = &entries[start..end];

    // Collect enabled columns in order
    let enabled_cols: Vec<ShowColumn> = state
        .display.show_columns
        .iter()
        .filter(|(_, e)| *e)
        .map(|(c, _)| *c)
        .collect();

    let viewport_width = area.width as usize;
    let prefix_width = 2; // "> " or "  "

    // Pre-compute column text for each visible entry so we can measure widths
    let mut col_text: Vec<Vec<String>> = visible
        .iter()
        .map(|de| {
            let mut row = vec![de.entry.cmd.clone()]; // column 0 = cmd
            for col in &enabled_cols {
                row.push(match col {
                    ShowColumn::Time => {
                        if state.display.time_mode == TimeMode::Date {
                            de.entry.date.clone()
                        } else {
                            age_string(&de.entry.date)
                        }
                    }
                    ShowColumn::Shell => de.entry.shell.clone(),
                    ShowColumn::Dir => {
                        if de.entry.pwd == state.session.current_dir {
                            PWD_DISPLAY.to_string()
                        } else if state.display.dir_mode == DirMode::RelPath && !de.relpath.is_empty() {
                            de.relpath.clone()
                        } else {
                            de.entry.pwd.clone()
                        }
                    }
                    ShowColumn::Repo => de.repo_name.clone(),
                    ShowColumn::Count => format!("({})", de.frequency),
                    ShowColumn::ExitCode => {
                        if de.entry.exit_code == "0" {
                            String::new()
                        } else {
                            de.entry.exit_code.clone()
                        }
                    }
                });
            }
            row
        })
        .collect();

    // Compute max width per metadata column (skip cmd at index 0)
    let meta_count = enabled_cols.len();
    let mut meta_widths = vec![0usize; meta_count];
    for row in &col_text {
        for (j, cell) in row[1..].iter().enumerate() {
            meta_widths[j] = meta_widths[j].max(cell.len());
        }
    }

    // If metadata takes more than half the viewport, shorten repo "owner/name" to "name"
    let meta_total: usize = meta_widths.iter().map(|w| 2 + w).sum();
    if meta_total > viewport_width / 2 {
        if let Some(repo_idx) = enabled_cols.iter().position(|c| *c == ShowColumn::Repo) {
            let col_idx = repo_idx + 1; // +1 because col 0 is cmd
            for row in &mut col_text {
                if let Some(slash) = row[col_idx].rfind('/') {
                    row[col_idx] = row[col_idx][slash + 1..].to_string();
                }
            }
            // Recompute widths
            for w in &mut meta_widths { *w = 0; }
            for row in &col_text {
                for (j, cell) in row[1..].iter().enumerate() {
                    meta_widths[j] = meta_widths[j].max(cell.len());
                }
            }
        }
    }

    // Shrink Dir column before truncating commands. Find the longest cmd
    // and ensure it fits without truncation by shortening paths first.
    if let Some(dir_idx) = enabled_cols.iter().position(|c| *c == ShowColumn::Dir) {
        let max_cmd_len: usize = col_text.iter().map(|r| r[0].len()).max().unwrap_or(0);
        let meta_total_now: usize = meta_widths.iter().map(|w| 2 + w).sum();
        let cmd_avail = viewport_width.saturating_sub(prefix_width + meta_total_now);
        if max_cmd_len > cmd_avail && meta_widths[dir_idx] > 8 {
            // Shrink Dir to give cmd more room
            let need = max_cmd_len - cmd_avail; // how many chars cmd needs
            let can_shrink = meta_widths[dir_idx].saturating_sub(8); // Dir won't go below 8
            let shrink_by = need.min(can_shrink);
            let max_dir = meta_widths[dir_idx] - shrink_by;
            let col_idx = dir_idx + 1;
            for row in &mut col_text {
                if row[col_idx].len() > max_dir {
                    row[col_idx] = shorten_path_middle(&row[col_idx], max_dir);
                }
            }
            meta_widths[dir_idx] = max_dir;
        }
    }

    // Ensure metadata fits within the viewport (reduce widest column first)
    let max_meta = viewport_width.saturating_sub(prefix_width);
    loop {
        let meta_total: usize = meta_widths.iter().map(|w| 2 + w).sum();
        if meta_total <= max_meta || meta_widths.is_empty() {
            break;
        }
        // Find the widest column (skip Dir if it's already <= 8)
        let max_idx = meta_widths.iter().enumerate()
            .filter(|(_, w)| **w > 1)
            .max_by_key(|(_, w)| *w)
            .map(|(i, _)| i);
        match max_idx {
            Some(idx) => {
                let excess = meta_total - max_meta;
                let reduce = excess.min(meta_widths[idx].saturating_sub(1));
                if reduce == 0 { break; }
                meta_widths[idx] -= reduce;
                // Truncate cell text to fit the reduced width
                let col_idx = idx + 1; // +1 because col 0 is cmd
                let is_path_col = enabled_cols[idx] == ShowColumn::Dir
                    || enabled_cols[idx] == ShowColumn::Repo;
                for row in &mut col_text {
                    if row[col_idx].len() > meta_widths[idx] {
                        if is_path_col && meta_widths[idx] > 3 {
                            row[col_idx] = shorten_path_middle(&row[col_idx], meta_widths[idx]);
                        } else {
                            row[col_idx] = row[col_idx][..meta_widths[idx]].to_string();
                        }
                    }
                }
            }
            None => break,
        }
    }

    // Total width of all metadata columns: each has "  " prefix + padded content
    let mut meta_total: usize = meta_widths.iter().map(|w| 2 + w).sum();

    // Debug build: reserve space for group bitmap column
    let grp_width: usize = if cfg!(debug_assertions) {
        state.filter.group.iter().filter(|(_, en)| *en).count()
    } else {
        0
    };
    if grp_width > 0 {
        meta_total += 2 + grp_width; // "  " prefix + bitmap digits
    }

    // Build the fuzzy needle buffer once for the whole row pass. None when
    // the search bar is empty — entries render without overlay in that case.
    let needle = state.search.search_input.as_str();
    let needle_buf = if needle.is_empty() {
        None
    } else {
        Some(NeedleBuf::new(needle))
    };

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, de)| {
            let abs_idx = start + i;
            let is_cursor = state.nav.focus != FocusZone::Search && abs_idx == state.nav.selected_index;
            let is_in_visual = state.nav.visual_range().map_or(false, |(lo, hi)| {
                abs_idx >= lo && abs_idx <= hi
            });
            let is_undo_highlight = state.undo_highlight.contains(&de.entry.date);
            let is_selected = is_cursor || is_in_visual || is_undo_highlight;
            let row = &col_text[i];

            let mut spans = Vec::new();

            // Selection indicator
            if is_cursor && is_in_visual {
                spans.push(Span::styled("> ", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else if is_cursor {
                spans.push(Span::styled("> ", Style::new().fg(CYAN).add_modifier(Modifier::BOLD)));
            } else if is_in_visual || is_undo_highlight {
                spans.push(Span::styled("│ ", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
            } else if de.entry.exit_code != "0" {
                spans.push(Span::styled("! ", Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)));
            } else {
                spans.push(Span::styled("● ", Style::new().fg(Color::Green)));
            }

            // Command column — syntax-colored shell command, truncated to fit
            let cmd_text = &row[0];
            let cmd_width = if meta_count > 0 {
                viewport_width.saturating_sub(prefix_width + meta_total)
            } else {
                viewport_width.saturating_sub(prefix_width)
            };
            let truncated = cmd_text.len() > cmd_width;
            let display_cmd = if truncated && cmd_width > 3 {
                format!("{}...", &cmd_text[..cmd_width - 3])
            } else if truncated {
                cmd_text[..cmd_width].to_string()
            } else {
                cmd_text.clone()
            };
            if is_cursor && (is_in_visual || is_undo_highlight) {
                spans.push(Span::styled(format!("{:<width$}", display_cmd, width = cmd_width), visual_highlight()));
            } else if is_in_visual || is_undo_highlight {
                spans.push(Span::styled(format!("{:<width$}", display_cmd, width = cmd_width), visual_highlight()));
            } else if is_cursor {
                spans.push(Span::styled(format!("{:<width$}", display_cmd, width = cmd_width), highlight()));
            } else {
                let base_spans = colorize_cmd(&display_cmd);
                // Layer fuzzy match highlight on top of syntax coloring when
                // a search query is active and this row matched. We recompute
                // indices against `display_cmd` (truncated) so positions align
                // with what's actually rendered.
                let with_highlight = match needle_buf.as_ref() {
                    Some(buf) => {
                        let mut idx: Vec<u32> = Vec::new();
                        if fuzzy_indices(&display_cmd, needle, buf, &mut idx).is_some() {
                            apply_match_highlight(base_spans, &idx)
                        } else {
                            base_spans
                        }
                    }
                    None => base_spans,
                };
                spans.extend(with_highlight);
                if display_cmd.len() < cmd_width {
                    spans.push(Span::raw(" ".repeat(cmd_width - display_cmd.len())));
                }
            };

            // Metadata columns (dir is left-aligned, others right-aligned)
            for (j, col) in enabled_cols.iter().enumerate() {
                let cell = &row[j + 1];
                let col_style = if is_cursor && (is_in_visual || is_undo_highlight) {
                    visual_highlight()
                } else if is_in_visual || is_undo_highlight {
                    visual_highlight()
                } else if is_cursor {
                    highlight()
                } else if *col == ShowColumn::Shell {
                    shell_color(cell)
                } else if *col == ShowColumn::ExitCode && !cell.is_empty() {
                    Style::new().fg(Color::Red)
                } else {
                    dim()
                };
                let expected_len = 2 + meta_widths[j];
                let formatted = match col {
                    ShowColumn::Count | ShowColumn::ExitCode => {
                        format!("  {:>width$}", cell, width = meta_widths[j])
                    }
                    _ => format!("  {:<width$}", cell, width = meta_widths[j]),
                };
                // Guard: truncate if formatted exceeds expected width
                let formatted = if formatted.len() > expected_len {
                    formatted[..expected_len].to_string()
                } else {
                    formatted
                };
                spans.push(Span::styled(formatted, col_style));
            }

            // Debug build: append group score bitmap column
            if grp_width > 0 {
                let grp_style = if is_selected { highlight() } else { dim() };
                spans.push(Span::styled(format!("  {:0>width$b}", de.group_score, width = grp_width), grp_style));
            }

            // Tint neutral text for non-selected rows based on exit code
            if !is_selected {
                let tint = if de.entry.exit_code != "0" {
                    Color::Indexed(131) // muted red (#af5f5f)
                } else {
                    Color::Indexed(108) // muted green (#87af87)
                };
                for span in &mut spans {
                    match span.style.fg {
                        Some(Color::White) | Some(Color::DarkGray) | None => {
                            span.style = span.style.fg(tint);
                        }
                        _ => {}
                    }
                }
            }

            let mut lines = vec![Line::from(spans)];

            // Detail expansion: show all fields when Space-toggled
            if state.nav.detail_index == Some(abs_idx) {
                let detail_style = Style::new().fg(Color::DarkGray);
                let indent = "    ";
                let cont_indent = "      ";
                let wrap_w = viewport_width.saturating_sub(indent.len());

                // Helper: wrap a detail line across multiple Line entries
                let mut wrap_line = |text: &str| {
                    if wrap_w == 0 || text.len() <= wrap_w {
                        lines.push(Line::from(Span::styled(format!("{}{}", indent, text), detail_style)));
                    } else {
                        let mut pos = 0;
                        let mut first = true;
                        while pos < text.len() {
                            let end = (pos + wrap_w).min(text.len());
                            let pfx = if first { indent } else { cont_indent };
                            first = false;
                            lines.push(Line::from(Span::styled(
                                format!("{}{}", pfx, &text[pos..end]), detail_style,
                            )));
                            pos = end;
                        }
                    }
                };

                // Line 1: shell (count): full command
                wrap_line(&format!("{} ({}): {}", de.entry.shell, de.frequency, de.entry.cmd));

                // Line 2: repo (only if in a repo)
                if !de.repo_name.is_empty() {
                    let repo_root = if !de.relpath.is_empty() {
                        de.entry.pwd.strip_suffix(&de.relpath)
                            .unwrap_or(&de.entry.pwd)
                            .trim_end_matches('/')
                    } else {
                        &de.entry.pwd
                    };
                    wrap_line(&format!("repo: {} | {}", repo_root, de.repo_name));
                }

                // Line 3: dir — $PWD if matches, else relpath if in repo, else abspath
                let dir_text = if de.entry.pwd == state.session.current_dir {
                    PWD_DISPLAY
                } else if !de.relpath.is_empty() {
                    &de.relpath
                } else {
                    &de.entry.pwd
                };
                wrap_line(&format!("dir: {}", dir_text));

                // Line 4: exit code
                if de.entry.exit_code != "0" {
                    wrap_line(&format!("exit: {} (failure)", de.entry.exit_code));
                } else {
                    wrap_line(&format!("exit: {}", de.entry.exit_code));
                }

                // Line 5: date | age
                wrap_line(&format!("date: {} | {}", de.entry.date, age_string(&de.entry.date)));
            }

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, area);
}

macro_rules! hint {
    ($spans:expr, $key:expr, $desc:expr) => {{
        $spans.push(Span::styled($key, active()));
        $spans.push(Span::styled(concat!(" ", $desc, "  "), dim()));
    }};
}

fn draw_footer(frame: &mut Frame, state: &AppState, entry_count: usize, area: Rect) {
    if let Some(ref msg) = state.delete_log.message {
        let spans = vec![
            Span::styled(msg.as_str(), Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    } else if let Some((lo, hi)) = state.nav.visual_range() {
        let count = hi - lo + 1;
        let mut spans = vec![
            Span::styled("-- VISUAL LINE --", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("  ", dim()),
        ];
        hint!(spans, "Esc", "quit");
        hint!(spans, "Tab", "next focus");
        hint!(spans, "Shift+Tab", "prev focus");
        hint!(spans, "↑↓/jk", "select");
        hint!(spans, "d", "delete");
        hint!(spans, "V", "cancel");
        let sel = format!("{} selected", count);
        let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let total = area.width as usize;
        let sel_len = sel.len();
        if content_width + sel_len + 1 < total {
            let pad = total - content_width - sel_len;
            spans.push(Span::raw(" ".repeat(pad)));
            spans.push(Span::styled(sel, Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    } else {
        draw_normal_footer(frame, state, entry_count, area);
    }
}

fn draw_normal_footer(frame: &mut Frame, state: &AppState, entry_count: usize, area: Rect) {
    let mut spans: Vec<Span<'static>> = Vec::new();
    hint!(spans, "Esc", "quit");
    hint!(spans, "Tab", "next focus");
    hint!(spans, "Shift+Tab", "prev focus");
    match state.nav.focus {
        FocusZone::List => {
            hint!(spans, "/", "search");
            hint!(spans, "↑↓/jk", "navigate");
            hint!(spans, "gg/G", "top/bottom");
            hint!(spans, "Space", "detail");
            hint!(spans, "Enter", "select");
            hint!(spans, "V", "visual");
            hint!(spans, "dd", "delete");
            hint!(spans, "u", "undo");
            spans.push(Span::styled("| ", dim()));
            spans.push(Span::styled("Quick Filter: ", label()));
            hint!(spans, "r", "repo");
            hint!(spans, "p", "pwd");
            hint!(spans, "t", "today");
            hint!(spans, "s", "success");
            hint!(spans, "f", "failure");
        }
        FocusZone::Search => {
            hint!(spans, "←→", "move cursor");
            hint!(spans, "Ctrl+A", "jump to start");
            hint!(spans, "Ctrl+E", "jump to end");
            hint!(spans, "Ctrl+U", "clear before cursor");
            hint!(spans, "Ctrl+K", "clear after cursor");
            hint!(spans, "Ctrl+W", "delete word");
        }
        _ => {
            hint!(spans, "/", "search");
            hint!(spans, "←→", "reorder");
            hint!(spans, "Space", "toggle");
        }
    }

    // Right-aligned position indicator: current_line/total_lines
    let pos = if entry_count > 0 {
        format!("{}/{}", state.nav.selected_index + 1, entry_count)
    } else {
        "0/0".to_string()
    };
    let content_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let total = area.width as usize;
    let pos_len = pos.len();
    if content_width + pos_len + 1 < total {
        let pad = total - content_width - pos_len;
        spans.push(Span::raw(" ".repeat(pad)));
        spans.push(Span::styled(pos, Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD)));
    }

    let footer = Paragraph::new(Line::from(spans));
    frame.render_widget(footer, area);
}

// ---------------------------------------------------------------------------
// Tests: colorize_cmd
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: extract (text, fg_color, is_bold) from spans
    fn span_info(spans: &[Span]) -> Vec<(String, Color, bool)> {
        spans.iter().map(|s| {
            let fg = s.style.fg.unwrap_or(Color::Reset);
            let bold = s.style.add_modifier.contains(Modifier::BOLD);
            (s.content.to_string(), fg, bold)
        }).collect()
    }

    fn find_span(info: &[(String, Color, bool)], text: &str) -> (String, Color, bool) {
        info.iter().find(|(t, _, _)| t == text).unwrap_or_else(|| panic!("span '{}' not found", text)).clone()
    }

    fn find_span_containing(info: &[(String, Color, bool)], substr: &str) -> (String, Color, bool) {
        info.iter().find(|(t, _, _)| t.contains(substr)).unwrap_or_else(|| panic!("span containing '{}' not found", substr)).clone()
    }

    #[test]
    fn colorize_command_is_magenta_bold() {
        let spans = colorize_cmd("git status");
        let info = span_info(&spans);
        assert_eq!(info[0].0, "git");
        assert_eq!(info[0].1, Color::Magenta);
        assert!(info[0].2);
    }

    #[test]
    fn colorize_flag_is_green() {
        let spans = colorize_cmd("git --verbose");
        let info = span_info(&spans);
        let flag = find_span(&info, "--verbose");
        assert_eq!(flag.1, Color::Green);
    }

    #[test]
    fn colorize_short_flag_is_green() {
        let spans = colorize_cmd("ls -la");
        let info = span_info(&spans);
        let flag = find_span(&info, "-la");
        assert_eq!(flag.1, Color::Green);
    }

    #[test]
    fn colorize_quoted_string_is_blue() {
        let spans = colorize_cmd("echo \"hello world\"");
        let info = span_info(&spans);
        let quoted = find_span_containing(&info, "hello");
        assert_eq!(quoted.1, Color::Blue);
    }

    #[test]
    fn colorize_single_quoted_is_blue() {
        let spans = colorize_cmd("grep '*.py' .");
        let info = span_info(&spans);
        let quoted = find_span_containing(&info, "*.py");
        assert_eq!(quoted.1, Color::Blue);
    }

    #[test]
    fn colorize_number_is_red() {
        let spans = colorize_cmd("head -n 100 file");
        let info = span_info(&spans);
        let num = find_span(&info, "100");
        assert_eq!(num.1, Color::Red);
    }

    #[test]
    fn colorize_version_is_red() {
        let spans = colorize_cmd("python 3.12.1");
        let info = span_info(&spans);
        let ver = find_span(&info, "3.12.1");
        assert_eq!(ver.1, Color::Red);
    }

    #[test]
    fn colorize_arg_value_is_yellow() {
        let spans = colorize_cmd("git push origin main");
        let info = span_info(&spans);
        assert_eq!(find_span(&info, "origin").1, Color::Yellow);
        assert_eq!(find_span(&info, "main").1, Color::Yellow);
    }

    #[test]
    fn colorize_variable_is_cyan() {
        let spans = colorize_cmd("echo $HOME");
        let info = span_info(&spans);
        let var = find_span(&info, "$HOME");
        assert_eq!(var.1, Color::Indexed(73));
    }

    #[test]
    fn colorize_operator_is_white_bold() {
        let spans = colorize_cmd("cat file | grep err");
        let info = span_info(&spans);
        let pipe = find_span(&info, "|");
        assert_eq!(pipe.1, Color::White);
        assert!(pipe.2, "operator should be bold");
    }

    #[test]
    fn colorize_command_after_pipe_is_magenta() {
        let spans = colorize_cmd("cat file | grep err");
        let info = span_info(&spans);
        let grep = find_span(&info, "grep");
        assert_eq!(grep.1, Color::Magenta);
        assert!(grep.2, "command after pipe should be bold");
    }

    #[test]
    fn colorize_redirect_is_dimmed() {
        let spans = colorize_cmd("echo hi > out.txt");
        let info = span_info(&spans);
        let redir = find_span(&info, ">");
        assert_eq!(redir.1, Color::DarkGray);
    }

    #[test]
    fn shorten_path_short_path_unchanged() {
        assert_eq!(shorten_path_middle("/home/user", 40), "/home/user");
    }

    #[test]
    fn shorten_path_long_path_middle_ellipsis() {
        let long = "/home/user/projects/workspace/build/very_long_config_directory_name_here";
        let result = shorten_path_middle(long, 40);
        assert!(result.len() <= 40, "got len={}: {}", result.len(), result);
        assert!(result.starts_with("/home/"), "got: {}", result);
        assert!(result.contains("..."), "got: {}", result);
    }

    #[test]
    fn shorten_path_preserves_end_of_last_component() {
        let long = "/home/user/very_long_directory_name_that_is_important_suffix";
        let result = shorten_path_middle(long, 35);
        assert!(result.len() <= 35, "got len={}: {}", result.len(), result);
        assert!(result.contains("suffix"), "got: {}", result);
    }

    #[test]
    fn shorten_path_relpath_no_leading_slash() {
        let relpath = ".build/very_long_config_directory_name_with_many_options_suffix";
        let result = shorten_path_middle(relpath, 30);
        assert!(result.len() <= 30, "got len={}: {}", result.len(), result);
        assert!(!result.starts_with('/'), "relpath must not start with /: {}", result);
        assert!(result.starts_with(".build/"), "got: {}", result);
        assert!(result.contains("..."), "got: {}", result);
    }

    #[test]
    fn shorten_path_abspath_keeps_leading_slash() {
        let abspath = "/home/user/deep/nested/very_long_directory_name";
        let result = shorten_path_middle(abspath, 30);
        assert!(result.starts_with('/'), "abspath must start with /: {}", result);
    }

    // -----------------------------------------------------------------------
    // Fuzzy match highlighting overlay
    // -----------------------------------------------------------------------

    #[test]
    fn apply_match_highlight_empty_indices_is_passthrough() {
        let spans = colorize_cmd("git status");
        let before_len = spans.len();
        let after = apply_match_highlight(spans.clone(), &[]);
        assert_eq!(after.len(), before_len);
        assert_eq!(span_info(&after), span_info(&spans));
    }

    #[test]
    fn apply_match_highlight_marks_matched_chars_with_overlay_style() {
        // "git status", needle "gst" → match positions 0 (g), 4 (s), 5 (t)
        let spans = colorize_cmd("git status");
        let highlighted = apply_match_highlight(spans, &[0, 4, 5]);

        // Every match char should appear in its own span with BOLD+UNDERLINED.
        let g = highlighted.iter().find(|s| s.content == "g").expect("g span");
        assert!(g.style.add_modifier.contains(Modifier::BOLD));
        assert!(g.style.add_modifier.contains(Modifier::UNDERLINED));

        let s = highlighted.iter().find(|s| s.content == "s").expect("s span");
        assert!(s.style.add_modifier.contains(Modifier::UNDERLINED));

        // Reassembling all span contents must give the original text back.
        let joined: String = highlighted.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(joined, "git status");
    }

    #[test]
    fn apply_match_highlight_preserves_base_syntax_modifiers() {
        // The first token "git" is rendered bold-magenta by colorize_cmd.
        // Highlighting position 0 must KEEP bold (already there from base
        // style) and ADD underline.
        let spans = colorize_cmd("git status");
        let highlighted = apply_match_highlight(spans, &[0]);
        let g = highlighted.iter().find(|s| s.content == "g").expect("g span");
        assert!(g.style.add_modifier.contains(Modifier::BOLD), "base BOLD lost");
        assert!(g.style.add_modifier.contains(Modifier::UNDERLINED), "overlay UNDERLINED missing");
    }

    #[test]
    fn apply_match_highlight_out_of_range_indices_ignored() {
        // Indices past the haystack length are silently skipped.
        let spans = colorize_cmd("ls");
        let highlighted = apply_match_highlight(spans, &[99]);
        let joined: String = highlighted.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(joined, "ls");
    }
}

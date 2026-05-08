//! Shared badge layout: geometry computation, badge descriptors, and hit-testing.
//! Used by both ui.rs (rendering) and input.rs (mouse click handling).

use super::state::*;

// Geometry constants — single source of truth for all badge widths.
const INDICATOR_W: usize = 2;  // "☑ " or "☐ "
const SEPARATOR_W: usize = 3;  // " | "
const ARROW_W: usize = 2;      // "◀ " or " ▶"
const BRACKET_W: usize = 3;    // "[◀ " or " ▶]"
const GAP_W: usize = 2;        // "  "
const ROW_SEP_W: usize = 2;    // "| "

// Prefix lengths for each badge row.
pub const SHOW_PREFIX: usize = 14;   // "Show Columns: "
pub const FILTER_PREFIX: usize = 14; // "Quick Filter: "
pub const GROUP_PREFIX: usize = 15;  // "Context Group: "
pub const ORDER_PREFIX: usize = 12;  // "Sort Order: "

/// A single unit of the rendered row with a known width and semantic identity.
pub struct Slot {
    pub width: usize,
    pub kind: SlotKind,
}

/// Semantic identity of a slot within a badge row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotKind {
    Prefix,
    LeftArrow(usize),
    RightArrow(usize),
    Indicator(usize),
    IndicatorSubA(usize),
    IndicatorSubB(usize),
    Label(usize),
    LabelSubA(usize),
    LabelSubB(usize),
    Separator(usize),
    RowSeparator,
    Gap,
}

/// Description of a badge's data — independent of focus styling.
pub enum BadgeDesc<'a> {
    Normal {
        index: usize,
        label: &'a str,
        enabled: bool,
        is_focused: bool,
        has_arrows: bool,
    },
    Grouped {
        index: usize,
        label_a: &'a str,
        label_b: &'a str,
        group_label: &'a str,
        a_on: bool,
        b_on: bool,
        is_focused: bool,
        has_arrows: bool,
    },
    Order {
        index: usize,
        label: &'a str,
        is_focused: bool,
    },
    Fixed {
        index: usize,
        label: &'a str,
        enabled: bool,
        is_focused: bool,
    },
}

/// Computed layout of a badge row: a sequence of positioned slots.
pub struct RowLayout {
    pub slots: Vec<Slot>,
}

impl RowLayout {
    /// Build the layout for a row from prefix width and badge descriptors.
    pub fn build(prefix_len: usize, badges: &[BadgeDesc]) -> Self {
        let mut slots = Vec::new();
        slots.push(Slot { width: prefix_len, kind: SlotKind::Prefix });

        for badge in badges {
            match badge {
                BadgeDesc::Normal { index, label, enabled: _, is_focused, has_arrows } => {
                    let i = *index;
                    if *has_arrows {
                        slots.push(Slot { width: ARROW_W, kind: SlotKind::LeftArrow(i) });
                    }
                    if *is_focused {
                        slots.push(Slot { width: INDICATOR_W, kind: SlotKind::Indicator(i) });
                    }
                    slots.push(Slot { width: label.len(), kind: SlotKind::Label(i) });
                    if *has_arrows {
                        slots.push(Slot { width: ARROW_W, kind: SlotKind::RightArrow(i) });
                    }
                    slots.push(Slot { width: GAP_W, kind: SlotKind::Gap });
                }
                BadgeDesc::Grouped { index, label_a, label_b, group_label,
                                     a_on, b_on, is_focused, has_arrows } => {
                    let i = *index;
                    if *has_arrows {
                        slots.push(Slot { width: ARROW_W, kind: SlotKind::LeftArrow(i) });
                    }
                    if *is_focused {
                        slots.push(Slot { width: INDICATOR_W, kind: SlotKind::IndicatorSubA(i) });
                        slots.push(Slot { width: label_a.len(), kind: SlotKind::LabelSubA(i) });
                        slots.push(Slot { width: SEPARATOR_W, kind: SlotKind::Separator(i) });
                        slots.push(Slot { width: INDICATOR_W, kind: SlotKind::IndicatorSubB(i) });
                        slots.push(Slot { width: label_b.len(), kind: SlotKind::LabelSubB(i) });
                    } else if *a_on {
                        slots.push(Slot { width: label_a.len(), kind: SlotKind::Label(i) });
                    } else if *b_on {
                        slots.push(Slot { width: label_b.len(), kind: SlotKind::Label(i) });
                    } else {
                        slots.push(Slot { width: group_label.len(), kind: SlotKind::Label(i) });
                    }
                    if *has_arrows {
                        slots.push(Slot { width: ARROW_W, kind: SlotKind::RightArrow(i) });
                    }
                    slots.push(Slot { width: GAP_W, kind: SlotKind::Gap });
                }
                BadgeDesc::Order { index, label, is_focused: _ } => {
                    let i = *index;
                    slots.push(Slot { width: BRACKET_W, kind: SlotKind::LeftArrow(i) });
                    slots.push(Slot { width: label.len(), kind: SlotKind::Label(i) });
                    slots.push(Slot { width: BRACKET_W, kind: SlotKind::RightArrow(i) });
                    slots.push(Slot { width: GAP_W, kind: SlotKind::Gap });
                }
                BadgeDesc::Fixed { index, label, enabled: _, is_focused } => {
                    let i = *index;
                    slots.push(Slot { width: ROW_SEP_W, kind: SlotKind::RowSeparator });
                    if *is_focused {
                        slots.push(Slot { width: INDICATOR_W, kind: SlotKind::Indicator(i) });
                    }
                    slots.push(Slot { width: label.len(), kind: SlotKind::Label(i) });
                    slots.push(Slot { width: GAP_W, kind: SlotKind::Gap });
                }
            }
        }

        RowLayout { slots }
    }

    /// Hit-test: given a terminal column, return the SlotKind at that position.
    pub fn hit(&self, col: usize) -> Option<&SlotKind> {
        let mut pos = 0;
        for slot in &self.slots {
            if col < pos + slot.width {
                return Some(&slot.kind);
            }
            pos += slot.width;
        }
        None
    }

    /// Total width of the row in terminal columns.
    pub fn total_width(&self) -> usize {
        self.slots.iter().map(|s| s.width).sum()
    }
}

// ---------------------------------------------------------------------------
// Badge constructors — build BadgeDesc slices from AppState
// ---------------------------------------------------------------------------

pub fn show_badges(state: &AppState) -> Vec<BadgeDesc<'_>> {
    let date_on = state.display.is_time_date();
    let age_on = state.display.is_time_age();
    let abspath_on = state.display.is_dir_abspath();
    let relpath_on = state.display.is_dir_relpath();

    state.display.show_columns.iter().enumerate().map(|(i, (col, _enabled))| {
        let is_focused = state.nav.focus == FocusZone::Show && state.nav.focus_index == i;
        match col {
            ShowColumn::Time => BadgeDesc::Grouped {
                index: i, label_a: "date", label_b: "age", group_label: "time",
                a_on: date_on, b_on: age_on, is_focused, has_arrows: true,
            },
            ShowColumn::Dir => BadgeDesc::Grouped {
                index: i, label_a: "abspath", label_b: "relpath", group_label: "path",
                a_on: abspath_on, b_on: relpath_on, is_focused, has_arrows: true,
            },
            _ => BadgeDesc::Normal {
                index: i, label: col.label(), enabled: state.display.is_show_enabled(*col),
                is_focused, has_arrows: true,
            },
        }
    }).collect()
}

pub fn filter_badges(state: &AppState) -> Vec<BadgeDesc<'_>> {
    let success_on = state.filter.is_exit_filter_success();
    let failure_on = state.filter.is_exit_filter_failure();
    let piped_on = state.filter.is_operator_filter_piped();
    let chained_on = state.filter.is_operator_filter_chained();

    state.filter.filters.iter().enumerate().map(|(i, (filter, enabled))| {
        let is_focused = state.nav.focus == FocusZone::Filter && state.nav.focus_index == i;
        match filter {
            FilterToggle::ExitCode => BadgeDesc::Grouped {
                index: i, label_a: "success", label_b: "failure", group_label: "exit",
                a_on: success_on, b_on: failure_on, is_focused, has_arrows: false,
            },
            FilterToggle::Operator => BadgeDesc::Grouped {
                index: i, label_a: "piped", label_b: "chained", group_label: "operator",
                a_on: piped_on, b_on: chained_on, is_focused, has_arrows: false,
            },
            _ => BadgeDesc::Normal {
                index: i, label: filter.label(), enabled: *enabled,
                is_focused, has_arrows: false,
            },
        }
    }).collect()
}

pub fn group_badges(state: &AppState) -> Vec<BadgeDesc<'_>> {
    let mut badges: Vec<BadgeDesc> = state.filter.group.iter().enumerate().map(|(i, (dim, enabled))| {
        let is_focused = state.nav.focus == FocusZone::Group && state.nav.focus_index == i;
        BadgeDesc::Normal {
            index: i, label: dim.label(), enabled: *enabled,
            is_focused, has_arrows: true,
        }
    }).collect();

    let dedup_idx = state.filter.group.len();
    let dedup_focused = state.nav.focus == FocusZone::Group && state.nav.focus_index == dedup_idx;
    badges.push(BadgeDesc::Fixed {
        index: dedup_idx, label: "dedup", enabled: state.filter.dedup, is_focused: dedup_focused,
    });

    badges
}

pub fn order_badges(state: &AppState) -> Vec<BadgeDesc<'_>> {
    state.filter.order.iter().enumerate().map(|(i, badge)| {
        let is_focused = state.nav.focus == FocusZone::Order && state.nav.focus_index == i;
        BadgeDesc::Order {
            index: i, label: badge.dim.label(badge.ascending), is_focused,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn show_row_unfocused_total_width() {
        let state = default_state();
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        // Each badge: ◀(2) + label + ▶(2) + gap(2) = label + 6
        // Time(unfocused, both off): label="time"(4) → 10
        // Shell: "shell"(5) → 11
        // Dir(unfocused, both off): "path"(4) → 10
        // Repo: "repo"(4) → 10
        // Count: "count"(5) → 11
        // ExitCode: "exit"(4) → 10
        // Total: 14 + 10 + 11 + 10 + 10 + 11 + 10 = 76
        assert_eq!(layout.total_width(), 76);
    }

    #[test]
    fn show_row_focused_grouped_badge_wider() {
        let mut state = default_state();
        state.nav.focus = FocusZone::Show;
        state.nav.focus_index = 0; // Time badge
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        // Time focused: ◀(2) + ☐(2) + "date"(4) + " | "(3) + ☐(2) + "age"(3) + ▶(2) + gap(2) = 20
        // vs unfocused: 10. Diff = +10
        assert_eq!(layout.total_width(), 76 + 10);
    }

    #[test]
    fn filter_row_no_arrows() {
        let state = default_state();
        let badges = filter_badges(&state);
        let layout = RowLayout::build(FILTER_PREFIX, &badges);
        // No arrows, so each normal badge: label + gap(2)
        // "this shell"(10)+2 + "pwd"(3)+2 + "this repo"(9)+2 + "today"(5)+2
        // Grouped (operator, unfocused both off): "operator"(8)+2
        // Grouped (exit_code, unfocused both off): "exit"(4)+2
        // Total: 14 + 12+5+11+7+10+6 = 65
        assert_eq!(layout.total_width(), 65);
    }

    #[test]
    fn group_row_with_dedup() {
        let state = default_state();
        let badges = group_badges(&state);
        let layout = RowLayout::build(GROUP_PREFIX, &badges);
        // 3 badges: ◀(2)+label+▶(2)+gap(2) each
        // "abspath"(7)→13, "repo"(4)→10, "relpath"(7)→13
        // dedup: "| "(2)+"dedup"(5)+gap(2) = 9
        // Total: 15 + 13 + 10 + 13 + 9 = 60
        assert_eq!(layout.total_width(), 60);
    }

    #[test]
    fn order_row_width() {
        let state = default_state();
        let badges = order_badges(&state);
        let layout = RowLayout::build(ORDER_PREFIX, &badges);
        // Each order: [◀(3) + label + ▶](3) + gap(2)
        // "recency: new first"(18) → 26
        // "frequency: most first"(21) → 29  (wait, need to check default ascending)
        // Default ascending=true: "recency: new first"(18), "frequency: most first"(21)
        // Total: 12 + 26 + 29 = 67
        assert_eq!(layout.total_width(), 67);
    }

    #[test]
    fn hit_prefix_returns_prefix() {
        let state = default_state();
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        assert_eq!(layout.hit(0), Some(&SlotKind::Prefix));
        assert_eq!(layout.hit(13), Some(&SlotKind::Prefix));
    }

    #[test]
    fn hit_first_arrow_returns_left_arrow() {
        let state = default_state();
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        // First badge starts at col 14: ◀ at [14,16)
        assert_eq!(layout.hit(14), Some(&SlotKind::LeftArrow(0)));
        assert_eq!(layout.hit(15), Some(&SlotKind::LeftArrow(0)));
    }

    #[test]
    fn hit_past_end_returns_none() {
        let state = default_state();
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        assert_eq!(layout.hit(999), None);
    }

    #[test]
    fn hit_focused_grouped_indicator_sub_a() {
        let mut state = default_state();
        state.nav.focus = FocusZone::Show;
        state.nav.focus_index = 0;
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        // col 14-15: LeftArrow(0), col 16-17: IndicatorSubA(0)
        assert_eq!(layout.hit(16), Some(&SlotKind::IndicatorSubA(0)));
        assert_eq!(layout.hit(17), Some(&SlotKind::IndicatorSubA(0)));
    }

    #[test]
    fn hit_focused_grouped_indicator_sub_b() {
        let mut state = default_state();
        state.nav.focus = FocusZone::Show;
        state.nav.focus_index = 0;
        let badges = show_badges(&state);
        let layout = RowLayout::build(SHOW_PREFIX, &badges);
        // col 14-15: LeftArrow, 16-17: IndSubA, 18-21: LabelSubA("date"=4),
        // 22-24: Separator(3), 25-26: IndicatorSubB
        assert_eq!(layout.hit(25), Some(&SlotKind::IndicatorSubB(0)));
    }
}

/// TUI application state: toggles, search, sort config, selection.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeMode {
    Date,
    Age,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirMode {
    AbsPath,
    RelPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitFilterMode {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorFilterMode {
    Piped,
    Chained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowColumn {
    Time,
    Shell,
    Dir,
    Repo,
    Count,
    ExitCode,
}

impl ShowColumn {
    /// UI display label and value used in show.order TOML array.
    pub fn label(&self) -> &'static str {
        match self {
            ShowColumn::Time => "time",
            ShowColumn::Shell => "shell",
            ShowColumn::Dir => "path",
            ShowColumn::Repo => "repo",
            ShowColumn::Count => "count",
            ShowColumn::ExitCode => "exit",
        }
    }

    /// TOML struct field name in [show] table.
    pub fn toml_key(&self) -> &'static str {
        match self {
            ShowColumn::Time => "time",
            ShowColumn::Shell => "shell",
            ShowColumn::Dir => "dir",
            ShowColumn::Repo => "repo",
            ShowColumn::Count => "count",
            ShowColumn::ExitCode => "exit_code",
        }
    }

    /// Inverse of label().
    pub fn from_label(s: &str) -> Option<ShowColumn> {
        Self::all_default_order().into_iter().find(|c| c.label() == s)
    }

    pub fn all_default_order() -> Vec<ShowColumn> {
        vec![
            ShowColumn::Time,
            ShowColumn::Shell,
            ShowColumn::Dir,
            ShowColumn::Repo,
            ShowColumn::Count,
            ShowColumn::ExitCode,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterToggle {
    ThisShell,
    ThisDir,
    ThisRepo,
    Today,
    Operator,
    ExitCode,
}

impl FilterToggle {
    pub fn label(&self) -> &'static str {
        match self {
            FilterToggle::ThisShell => "this shell",
            FilterToggle::ThisDir => "pwd",
            FilterToggle::ThisRepo => "this repo",
            FilterToggle::Today => "today",
            FilterToggle::Operator => "operator",
            FilterToggle::ExitCode => "exit",
        }
    }

    /// TOML struct field name in [filter] table.
    pub fn toml_key(&self) -> &'static str {
        match self {
            FilterToggle::ThisShell => "this_shell",
            FilterToggle::ThisDir => "this_dir",
            FilterToggle::ThisRepo => "this_repo",
            FilterToggle::Today => "today",
            FilterToggle::Operator => "operator",
            FilterToggle::ExitCode => "exit_code",
        }
    }

    pub fn all() -> Vec<FilterToggle> {
        vec![
            FilterToggle::ThisShell,
            FilterToggle::ThisDir,
            FilterToggle::ThisRepo,
            FilterToggle::Today,
            FilterToggle::Operator,
            FilterToggle::ExitCode,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderDimension {
    Recency,
    Frequency,
}

impl OrderDimension {
    pub fn label(&self, ascending: bool) -> &'static str {
        match (self, ascending) {
            (OrderDimension::Recency, true) => "recency: new first",
            (OrderDimension::Recency, false) => "recency: old first",
            (OrderDimension::Frequency, true) => "frequency: most first",
            (OrderDimension::Frequency, false) => "frequency: least first",
        }
    }

    /// TOML key name in [order] table.
    pub fn toml_key(&self) -> &'static str {
        match self {
            OrderDimension::Recency => "recency",
            OrderDimension::Frequency => "frequency",
        }
    }

    pub fn from_toml_key(s: &str) -> Option<OrderDimension> {
        match s {
            "recency" => Some(OrderDimension::Recency),
            "frequency" => Some(OrderDimension::Frequency),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupDimension {
    Dir,
    Repo,
    RelPath,
}

impl GroupDimension {
    /// UI label and TOML key (identical for GroupDimension).
    pub fn label(&self) -> &'static str {
        match self {
            GroupDimension::Dir => "abspath",
            GroupDimension::Repo => "repo",
            GroupDimension::RelPath => "relpath",
        }
    }

    pub fn from_label(s: &str) -> Option<GroupDimension> {
        match s {
            "abspath" => Some(GroupDimension::Dir),
            "repo" => Some(GroupDimension::Repo),
            "relpath" => Some(GroupDimension::RelPath),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusZone {
    Show,
    Filter,
    Group,
    Order,
    Search,
    List,
}

pub struct OrderBadge {
    pub dim: OrderDimension,
    pub ascending: bool,
}

// ---------------------------------------------------------------------------
// Sub-structs
// ---------------------------------------------------------------------------

pub struct DisplayConfig {
    pub show_columns: Vec<(ShowColumn, bool)>,
    pub time_mode: TimeMode,
    pub dir_mode: DirMode,
}

pub struct FilterState {
    pub filters: Vec<(FilterToggle, bool)>,
    pub exit_filter_mode: ExitFilterMode,
    pub operator_filter_mode: OperatorFilterMode,
    pub order: Vec<OrderBadge>,
    pub group: Vec<(GroupDimension, bool)>,
    pub dedup: bool,
}

pub struct NavState {
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub focus: FocusZone,
    pub focus_index: usize,
    pub detail_index: Option<usize>,
    pub visual_mode: bool,
    pub visual_anchor: Option<usize>,
    pub has_navigated: bool,
    pub pending_g: bool,
    pub pending_d: bool,
}

pub struct SearchState {
    pub search_input: String,
    pub search_cursor: usize,
    pub search_regex: Option<regex::Regex>,
}

pub struct SessionCtx {
    pub current_dir: String,
    pub waive_commands: Vec<String>,
    pub waive_min_cmd_len: usize,
}

// ---------------------------------------------------------------------------
// Sub-struct methods
// ---------------------------------------------------------------------------

impl DisplayConfig {
    pub fn is_show_enabled(&self, col: ShowColumn) -> bool {
        self.show_columns.iter().any(|(c, e)| *c == col && *e)
    }

    pub fn is_time_date(&self) -> bool {
        self.is_show_enabled(ShowColumn::Time) && self.time_mode == TimeMode::Date
    }

    pub fn is_time_age(&self) -> bool {
        self.is_show_enabled(ShowColumn::Time) && self.time_mode == TimeMode::Age
    }

    pub fn is_dir_abspath(&self) -> bool {
        self.is_show_enabled(ShowColumn::Dir) && self.dir_mode == DirMode::AbsPath
    }

    pub fn is_dir_relpath(&self) -> bool {
        self.is_show_enabled(ShowColumn::Dir) && self.dir_mode == DirMode::RelPath
    }

    pub fn toggle_show(&mut self, index: usize) {
        if index >= self.show_columns.len() {
            return;
        }
        let (col, enabled) = self.show_columns[index];
        if col == ShowColumn::Time {
            if !enabled {
                self.show_columns[index].1 = true;
                self.time_mode = TimeMode::Date;
            } else if self.time_mode == TimeMode::Date {
                self.time_mode = TimeMode::Age;
            } else {
                self.show_columns[index].1 = false;
            }
        } else if col == ShowColumn::Dir {
            if !enabled {
                self.show_columns[index].1 = true;
                self.dir_mode = DirMode::AbsPath;
            } else if self.dir_mode == DirMode::AbsPath {
                self.dir_mode = DirMode::RelPath;
            } else {
                self.show_columns[index].1 = false;
            }
        } else {
            self.show_columns[index].1 = !enabled;
        }
    }

    /// Toggle date independently: if date is on, turn time off; otherwise turn date on.
    pub fn toggle_time_date(&mut self) {
        if let Some((_, en)) = self.show_columns.iter_mut().find(|(c, _)| *c == ShowColumn::Time) {
            if *en && self.time_mode == TimeMode::Date {
                *en = false;
            } else {
                *en = true;
                self.time_mode = TimeMode::Date;
            }
        }
    }

    /// Toggle age independently: if age is on, turn time off; otherwise turn age on.
    pub fn toggle_time_age(&mut self) {
        if let Some((_, en)) = self.show_columns.iter_mut().find(|(c, _)| *c == ShowColumn::Time) {
            if *en && self.time_mode == TimeMode::Age {
                *en = false;
            } else {
                *en = true;
                self.time_mode = TimeMode::Age;
            }
        }
    }

    /// Toggle abspath independently: if abspath is on, turn dir off; otherwise turn abspath on.
    pub fn toggle_dir_abspath(&mut self) {
        if let Some((_, en)) = self.show_columns.iter_mut().find(|(c, _)| *c == ShowColumn::Dir) {
            if *en && self.dir_mode == DirMode::AbsPath {
                *en = false;
            } else {
                *en = true;
                self.dir_mode = DirMode::AbsPath;
            }
        }
    }

    /// Toggle relpath independently: if relpath is on, turn dir off; otherwise turn relpath on.
    pub fn toggle_dir_relpath(&mut self) {
        if let Some((_, en)) = self.show_columns.iter_mut().find(|(c, _)| *c == ShowColumn::Dir) {
            if *en && self.dir_mode == DirMode::RelPath {
                *en = false;
            } else {
                *en = true;
                self.dir_mode = DirMode::RelPath;
            }
        }
    }

    pub fn swap_show(&mut self, index: usize, direction: i32) {
        let new_idx = index as i32 + direction;
        if new_idx >= 0 && (new_idx as usize) < self.show_columns.len() {
            self.show_columns.swap(index, new_idx as usize);
        }
    }
}

impl FilterState {
    pub fn is_filter_enabled(&self, filter: FilterToggle) -> bool {
        self.filters.iter().any(|(f, e)| *f == filter && *e)
    }

    pub fn toggle_filter_by_kind(&mut self, filter: FilterToggle) {
        if let Some((_, en)) = self.filters.iter_mut().find(|(f, _)| *f == filter) {
            *en = !*en;
        }
    }

    pub fn toggle_filter(&mut self, index: usize) {
        if index >= self.filters.len() {
            return;
        }
        let (filter, enabled) = self.filters[index];
        if filter == FilterToggle::ExitCode {
            // Cycle: off -> success -> failure -> off
            if !enabled {
                self.filters[index].1 = true;
                self.exit_filter_mode = ExitFilterMode::Success;
            } else if self.exit_filter_mode == ExitFilterMode::Success {
                self.exit_filter_mode = ExitFilterMode::Failure;
            } else {
                self.filters[index].1 = false;
            }
        } else if filter == FilterToggle::Operator {
            // Cycle: off -> piped -> chained -> off
            if !enabled {
                self.filters[index].1 = true;
                self.operator_filter_mode = OperatorFilterMode::Piped;
            } else if self.operator_filter_mode == OperatorFilterMode::Piped {
                self.operator_filter_mode = OperatorFilterMode::Chained;
            } else {
                self.filters[index].1 = false;
            }
        } else {
            self.filters[index].1 = !enabled;
        }
    }

    pub fn is_exit_filter_success(&self) -> bool {
        self.is_filter_enabled(FilterToggle::ExitCode) && self.exit_filter_mode == ExitFilterMode::Success
    }

    pub fn is_exit_filter_failure(&self) -> bool {
        self.is_filter_enabled(FilterToggle::ExitCode) && self.exit_filter_mode == ExitFilterMode::Failure
    }

    /// Toggle success independently: if success is on, turn exit filter off; otherwise turn success on.
    pub fn toggle_exit_filter_success(&mut self) {
        if let Some((_, en)) = self.filters.iter_mut().find(|(f, _)| *f == FilterToggle::ExitCode) {
            if *en && self.exit_filter_mode == ExitFilterMode::Success {
                *en = false;
            } else {
                *en = true;
                self.exit_filter_mode = ExitFilterMode::Success;
            }
        }
    }

    /// Toggle failure independently: if failure is on, turn exit filter off; otherwise turn failure on.
    pub fn toggle_exit_filter_failure(&mut self) {
        if let Some((_, en)) = self.filters.iter_mut().find(|(f, _)| *f == FilterToggle::ExitCode) {
            if *en && self.exit_filter_mode == ExitFilterMode::Failure {
                *en = false;
            } else {
                *en = true;
                self.exit_filter_mode = ExitFilterMode::Failure;
            }
        }
    }

    pub fn is_operator_filter_piped(&self) -> bool {
        self.is_filter_enabled(FilterToggle::Operator) && self.operator_filter_mode == OperatorFilterMode::Piped
    }

    pub fn is_operator_filter_chained(&self) -> bool {
        self.is_filter_enabled(FilterToggle::Operator) && self.operator_filter_mode == OperatorFilterMode::Chained
    }

    pub fn toggle_operator_filter_piped(&mut self) {
        if let Some((_, en)) = self.filters.iter_mut().find(|(f, _)| *f == FilterToggle::Operator) {
            if *en && self.operator_filter_mode == OperatorFilterMode::Piped {
                *en = false;
            } else {
                *en = true;
                self.operator_filter_mode = OperatorFilterMode::Piped;
            }
        }
    }

    pub fn toggle_operator_filter_chained(&mut self) {
        if let Some((_, en)) = self.filters.iter_mut().find(|(f, _)| *f == FilterToggle::Operator) {
            if *en && self.operator_filter_mode == OperatorFilterMode::Chained {
                *en = false;
            } else {
                *en = true;
                self.operator_filter_mode = OperatorFilterMode::Chained;
            }
        }
    }

    pub fn toggle_order_direction(&mut self, index: usize) {
        if index < self.order.len() {
            self.order[index].ascending = !self.order[index].ascending;
        }
    }

    pub fn toggle_group(&mut self, index: usize) {
        if index < self.group.len() {
            self.group[index].1 = !self.group[index].1;
        }
    }

    pub fn is_group_enabled(&self, dim: GroupDimension) -> bool {
        self.group.iter().any(|(d, e)| *d == dim && *e)
    }

    pub fn swap_order(&mut self, index: usize, direction: i32) {
        let new_idx = index as i32 + direction;
        if new_idx >= 0 && (new_idx as usize) < self.order.len() {
            self.order.swap(index, new_idx as usize);
        }
    }

    pub fn swap_group(&mut self, index: usize, direction: i32) {
        let new_idx = index as i32 + direction;
        if new_idx >= 0 && (new_idx as usize) < self.group.len() {
            self.group.swap(index, new_idx as usize);
        }
    }
}

impl NavState {
    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.exit_visual_mode();
    }

    pub fn visual_range(&self) -> Option<(usize, usize)> {
        if self.visual_mode {
            let anchor = self.visual_anchor.unwrap_or(self.selected_index);
            Some((anchor.min(self.selected_index), anchor.max(self.selected_index)))
        } else {
            None
        }
    }

    pub fn exit_visual_mode(&mut self) {
        self.visual_mode = false;
        self.visual_anchor = None;
    }
}

// ---------------------------------------------------------------------------
// AppState — composed from sub-structs
// ---------------------------------------------------------------------------

pub struct AppState {
    pub display: DisplayConfig,
    pub filter: FilterState,
    pub nav: NavState,
    pub search: SearchState,
    pub session: SessionCtx,
    pub delete_log: super::delete::DeleteLog,
    /// Dates of entries restored by undo — highlighted until any input clears them.
    pub undo_highlight: std::collections::HashSet<String>,
    // Signals stay top-level
    pub delete_requested: bool,
    pub undo_requested: bool,
    pub quit: bool,
    pub exec_cmd: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            display: DisplayConfig {
                show_columns: ShowColumn::all_default_order()
                    .into_iter()
                    .map(|c| (c, false))
                    .collect(),
                time_mode: TimeMode::Date,
                dir_mode: DirMode::AbsPath,
            },
            filter: FilterState {
                filters: FilterToggle::all()
                    .into_iter()
                    .map(|f| (f, false))
                    .collect(),
                exit_filter_mode: ExitFilterMode::Failure,
                operator_filter_mode: OperatorFilterMode::Piped,
                order: vec![
                    OrderBadge {
                        dim: OrderDimension::Recency,
                        ascending: true,
                    },
                    OrderBadge {
                        dim: OrderDimension::Frequency,
                        ascending: true,
                    },
                ],
                group: vec![
                    (GroupDimension::Dir, true),
                    (GroupDimension::Repo, true),
                    (GroupDimension::RelPath, true),
                ],
                dedup: false,
            },
            nav: NavState {
                selected_index: 0,
                scroll_offset: 0,
                focus: FocusZone::List,
                focus_index: 0,
                detail_index: None,
                visual_mode: false,
                visual_anchor: None,
                has_navigated: false,
                pending_g: false,
                pending_d: false,
            },
            search: SearchState {
                search_input: String::new(),
                search_cursor: 0,
                search_regex: None,
            },
            session: SessionCtx {
                current_dir: String::new(),
                waive_commands: Vec::new(),
                waive_min_cmd_len: 0,
            },
            delete_log: super::delete::DeleteLog::new(),
            undo_highlight: std::collections::HashSet::new(),
            delete_requested: false,
            undo_requested: false,
            quit: false,
            exec_cmd: None,
        }
    }

    pub fn next_focus(&mut self) {
        self.nav.focus = match self.nav.focus {
            FocusZone::Show => FocusZone::Filter,
            FocusZone::Filter => FocusZone::Group,
            FocusZone::Group => FocusZone::Order,
            FocusZone::Order => FocusZone::Search,
            FocusZone::Search => FocusZone::List,
            FocusZone::List => FocusZone::Show,
        };
        self.nav.focus_index = 0;
    }

    pub fn prev_focus(&mut self) {
        self.nav.focus = match self.nav.focus {
            FocusZone::Show => FocusZone::List,
            FocusZone::Filter => FocusZone::Show,
            FocusZone::Group => FocusZone::Filter,
            FocusZone::Order => FocusZone::Group,
            FocusZone::Search => FocusZone::Order,
            FocusZone::List => FocusZone::Search,
        };
        self.nav.focus_index = 0;
    }
}

//! [`bb-tui`] is a basic abstraction over [`ratatui`] with the purpose of offering
//! reusability and shared functionality between basic `bb` apps that require a TUI.
//!
//! It declares a very basic UI and data model, though rather strict, that is short
//! to implement.
//!
//! It's oriented around encapsulating [`bb-clang`] data primarily, expecting things
//! such as entities with a source location (for file tree), a search bar (for
//! querying entities by names, often by going through [`bb_shared::glob_match`]),
//! and a view.
//!
//! It also offers you stuff like Vim-style browsing (but also "normal"/non-modal
//! browsing) and much other fun stuff.
//!
//! This is the approach we believe is best for the purposes of this project

pub mod event;
pub mod ui;

use ratatui::text::Line;

/* ──────────────────── Focus -- the 3 elements of the UI ─────────────────── */

/// Which part of the UI has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Content,
    Search,
}

/* ──────────────── FileEntry — the entries in the file tree ─────────────── */

/// An entry in the file tree.
pub struct FileEntry {
    pub name: String,
    pub count: usize,
}

/* ──── TuiData — the trait that needs to be implemented to display data ─── */

/// Domain-specific data source for the TUI.
pub trait TuiData {
    /// Label for the list pane title when no file is selected.
    fn title(&self) -> &str;
    /// File entries (index 0 is always "(all)").
    fn files(&self) -> &[FileEntry];
    /// Number of pre-built rows.
    fn row_count(&self) -> usize;
    /// Render row at `index` to a styled Line.
    fn render_row(&self, index: usize) -> Line<'static>;
    /// Rebuild the file index for the current search pattern.
    fn rebuild_index(&mut self, search: Option<&str>);
    /// Rebuild rows for the current search pattern + file filter.
    fn rebuild_rows(&mut self, search: Option<&str>, file_filter: Option<&str>);
}

/* ──────────────────── App — the basic state of the UI ──────────────────── */

pub struct App<D> {
    pub focus: Focus,
    pub search: String,
    /// Byte offset of the cursor within `search`.
    pub cursor: usize,
    pub scroll: usize,
    pub hscroll: usize,
    pub file_cursor: usize,
    pub data: D,
}

impl<D: TuiData> App<D> {
    pub fn new(data: D, initial_search: &str) -> Self {
        let search = initial_search.to_string();
        let cursor = search.len();
        let mut app = Self {
            focus: Focus::Tree,
            search,
            cursor,
            scroll: 0,
            hscroll: 0,
            file_cursor: 0,
            data,
        };
        app.rebuild();
        app
    }

    /// The currently selected file filter, or None for "(all)".
    pub fn selected_file(&self) -> Option<&str> {
        if self.file_cursor == 0 {
            None
        } else {
            self.data
                .files()
                .get(self.file_cursor)
                .map(|f| f.name.as_str())
        }
    }

    /// Rebuild file index + rows, preserving cursor.
    pub fn rebuild(&mut self) {
        let search = if self.search.is_empty() {
            None
        } else {
            Some(self.search.as_str())
        };

        let selected_name = self.selected_file().map(str::to_string);

        self.data.rebuild_index(search);

        self.file_cursor = match &selected_name {
            Some(name) => self
                .data
                .files()
                .iter()
                .position(|f| f.name == *name)
                .unwrap_or(0),
            None => 0,
        };

        let file_filter = self.selected_file().map(str::to_string);
        self.data.rebuild_rows(search, file_filter.as_deref());

        if self.data.row_count() == 0 {
            self.scroll = 0;
        } else if self.scroll >= self.data.row_count() {
            self.scroll = self.data.row_count().saturating_sub(1);
        }
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = self
            .scroll
            .saturating_add(n)
            .min(self.data.row_count().saturating_sub(1));
    }

    pub const fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    pub const fn scroll_right(&mut self, n: usize) {
        self.hscroll = self.hscroll.saturating_add(n);
    }

    pub const fn scroll_left(&mut self, n: usize) {
        self.hscroll = self.hscroll.saturating_sub(n);
    }

    pub fn tree_down(&mut self) {
        if self.file_cursor + 1 < self.data.files().len() {
            self.file_cursor += 1;
            self.scroll = 0;
            self.rebuild();
        }
    }

    pub fn tree_up(&mut self) {
        if self.file_cursor > 0 {
            self.file_cursor -= 1;
            self.scroll = 0;
            self.rebuild();
        }
    }
}

/* ─────────────────────────────── File filter ────────────────────────────── */

/// Check if an item's file matches the selected filter.
#[must_use]
pub fn matches_file(item_file: Option<&str>, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some(f) => item_file.is_some_and(|name| name == f),
    }
}

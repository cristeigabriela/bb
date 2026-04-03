# bb-tui

> A basic TUI framework built on [`ratatui`](https://ratatui.rs/), designed around a specific data model.

---

## UI elements

- A file tree;
- A content area;
- A search bar;
- A status bar.

> **Note** — Subject to change, this README might become outdated. You may always refer [to ui.rs](./src/ui.rs).

---

## UX design

Primarily built around `bb-clang` data.

- Integrates a file tree to split contents up by their source location;
- Integrates a fuzzy search bar (`bb-shared` `glob_match`) that allows you to search over the aforementioned entities;
    - Extra functionality might be integrated in the filters at project level.
- Implements Vim-style (`h`/`j`/`k`/`l`/`q`), and non-modal (normal) navigation;
- Implements a content area that can be scrolled (`j`/`k`/`up`/`down`) and panned (`h`/`l`/`left`/`right`).

---

## Data design

[In lib.rs](./src/lib.rs) there is a trait called `TuiData` which defines the data prerequisites for a TUI, respectively:

- A name for the content area;
- A way to get the file index;
- A way to build the file index;
- A way to get the number of rows (lines) that will be displayed in the content area;
- A way to draw the rows (lines) in the content area;
- A way to rebuild the rows (lines) to draw in the content area;

---

## Limitations

This is inherently and intentionally restricted, so that basic TUI applications can be **1. short**, and **2. share as much functionality (and code) as possible**.

### Benefits

An update to, for example, navigating the content area, becomes functionality that is automagically propagated to all other TUI applications.

Implementing, for example, a new way to navigate files, will allow you to use all the in-project TUI applications with those specific keybinds.

The ways this can extend is left as an imagination exercise for the user.

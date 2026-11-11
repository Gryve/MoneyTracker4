//! MoneyTracker4 — Rune UI over the MoneyTracker SQLite database.
//!
//! `db` is a 1:1 port of the Python data layer; `model` holds the domain types and the
//! formatting; `state` owns what the views read; `ui` draws it.

pub mod db;
pub mod icons;
pub mod model;
pub mod state;
pub mod ui;

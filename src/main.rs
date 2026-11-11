//! MoneyTracker4 — the MoneyTracker database in a Rune window.
//!
//! The data layer is a port of `data_manager.py`; nothing about the SQLite file changes, so
//! this app and the Python one can alternate on the same database.

use moneytracker4::state::App;
use moneytracker4::ui;
use rune::{AppSpec, Chrome, Key, KeyEvent, Tx};
use rune_theme_opencode as theme;

fn init() -> App {
    App::new()
}

fn on_key(s: &mut App, ev: &KeyEvent, _: &mut Tx) -> bool {
    if rune_kit::shell::is_sidebar_toggle(ev) {
        s.sidebar.toggle();
        return true;
    }
    if ev.is_command('n') {
        s.open_add();
        return true;
    }
    if ev.key == Key::Escape && s.dialog.is_some() {
        s.close_dialog();
        return true;
    }
    false
}

fn main() {
    rune::App::new(AppSpec {
        fonts: theme::FONTS,
        families: theme::FAMILIES,
        on_key: Some(on_key),
        min_size: Some((980.0, 640.0)),
        ..AppSpec::new("MoneyTracker 4", (1180.0, 760.0), init, ui::root)
    })
    .chrome(Chrome {
        full_size_content: true,
        traffic_lights: Some((16.0, 25.0)),
        drag_regions: true,
    })
    .run();
}

//! The window: sidebar, toolbar, the view switch, and the modal layer.
//!
//! Rune re-renders from [`App`] every frame, so a view is a plain function of the state; the
//! handlers on the elements are what change it.

pub mod budgets;
pub mod calendar;
pub mod dashboard;
pub mod dialogs;
pub mod transactions;
pub mod widgets;
pub mod year;

use rune::{col, row, text, Cx, El, ScrollSpec};
use rune_kit::shell::{Sidebar, SidebarRow, SidebarToggle, SplitView, Toolbar};
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit};
use rune_theme_opencode as theme;

use crate::icons;
use crate::model;
use crate::state::{App, View};
use widgets::{amount_style, panel};

/// The whole application surface, plus the overlays kept in `layer`.
pub fn root(s: &App, cx: &mut Cx<App>) -> El {
    let k = theme::kit(cx.system_dark());

    // The pickers' listboxes mount here, before the dialogs, so a modal always sits on top.
    let mut layer = Vec::new();
    let main = match s.view {
        View::Dashboard => dashboard::view(s, k, &mut layer),
        View::Transactions => transactions::view(s, k, &mut layer),
        View::Calendar => calendar::view(s, k, &mut layer),
        View::Budgets => budgets::view(s, k, &mut layer),
        View::Year => year::view(s, k, &mut layer),
    };

    let toolbar = Toolbar::new()
        .title("MoneyTracker")
        .trailing(
            Button::new("Export Excel")
                .variant(ButtonVariant::Neutral)
                .control_size(ControlSize::Small)
                .icon(&icons::DOWNLOAD)
                .shortcut("Ctrl+E")
                .on_press(|s: &mut App| s.open_export())
                .build(k),
        )
        .trailing(
            Button::new("Add transaction")
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::Small)
                .icon(&icons::PLUS)
                .shortcut("Ctrl+N")
                .on_press(|s: &mut App| s.open_add())
                .build(k),
        )
        .trailing(
            IconButton::new(&icons::TAG)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Manage categories")
                .on_press(|s: &mut App| s.open_categories())
                .build(k)
                .key("manage-categories"),
        );

    let shell = SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, sidebar(s, k))
        .toolbar(toolbar)
        .sidebar_toggle(SidebarToggle::new().in_header().shortcut("Ctrl+Shift+B"))
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(8.0)
        .toolbar_in_panel()
        .sidebar_padding(16.0, 10.0)
        .reduced_motion(cx.is_reduced_motion())
        .build(k);

    dialogs::mount(s, k, &mut layer);
    rune_kit::root(shell, layer)
}

fn sidebar(s: &App, k: &Kit) -> El {
    let mut bar = Sidebar::new()
        .label("Navigation")
        .inset(0.0)
        .header_gap(4.0)
        .header_padding(0.0, 14.0)
        .row_gap(2.0)
        .header(brand(k))
        .footer(balance_card(s, k));

    for view in View::ALL {
        let selected = s.view == view;
        bar = bar.child(
            SidebarRow::new(view.label())
                .icon(view.icon())
                .height(36.0)
                .selected(selected)
                .on_select(move |s: &mut App, _| s.set_view(view))
                .build(k)
                .key(format!("nav-{view:?}")),
        );
    }

    bar = bar.child(
        SidebarRow::new("Manage categories")
            .icon(&icons::TAG)
            .height(36.0)
            .on_select(|s: &mut App, _| s.open_categories())
            .build(k)
            .key("nav-categories"),
    );

    bar.build(k)
}

fn brand(k: &Kit) -> El {
    row()
        .w_full()
        .gap(10.0)
        .center_y()
        .padding(8.0)
        .child(widgets::icon_chip(k, &icons::WALLET, k.c.success.fg, 30.0))
        .child(
            col()
                .gap(0.0)
                .child(
                    text("MoneyTracker")
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base),
                )
                .child(
                    text("Rust edition")
                        .font(k.ty.caption())
                        .color(k.c.text_faint),
                ),
        )
}

fn balance_card(s: &App, k: &Kit) -> El {
    panel(k)
        .w_full()
        .padding(14.0)
        .gap(3.0)
        .child(
            text("All-time balance")
                .font(k.ty.caption())
                .color(k.c.text_muted),
        )
        .child(
            text(model::euros(s.all_time.balance))
                .font(amount_style(k, 20.0))
                .color(k.c.success.fg),
        )
        .child(
            col()
                .w_full()
                .gap(1.0)
                .child(
                    text(format!(
                        "Income ↗ {}",
                        model::euros(s.all_time.total_income)
                    ))
                    .font(k.ty.caption())
                    .color(k.c.text_faint)
                    .truncate(),
                )
                .child(
                    text(format!(
                        "Expenses ↘ {}",
                        model::euros(s.all_time.total_expense)
                    ))
                    .font(k.ty.caption())
                    .color(k.c.text_faint)
                    .truncate(),
                ),
        )
}

/// The scrolling body every view sits in, with the app's last warning above its content.
pub fn page(key: &str, s: &App, k: &Kit, body: El) -> El {
    let mut inner = col().key("page-inner").w_full().padding(22.0).gap(16.0);
    if let Some(notice) = &s.notice {
        inner = inner.child(widgets::notice_banner(k, notice));
    }
    col()
        .key(format!("page-{key}"))
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(inner.child(body))
}

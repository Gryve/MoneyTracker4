//! Budgets: one editable annual limit per expense category.

use rune::{col, row, text, El};
use rune_kit::Kit;

use crate::icons;
use crate::model;
use crate::state::App;

use super::page;
use super::widgets::{budget_row, budget_summary, empty_state, page_header, panel, year_picker};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let header = page_header(
        k,
        "Budgets",
        &format!("Annual limits for {}", s.year),
        Some(year_picker(s, k, layer)),
    );

    let total = s.total_budget();
    let spent = s.annual.total_expense;
    let summary = budget_summary(
        k,
        &format!("Total annual budget {}", s.year),
        spent,
        total,
        if total > 0.0 && spent > total {
            k.c.danger.fg
        } else {
            k.c.text_base
        },
    );

    let groups = model::display_groups(&s.categories.expense);
    let spend = s.annual_spend_by_display();

    let mut list = col().w_full().gap(8.0);
    if groups.is_empty() {
        list = list.child(panel(k).w_full().child(empty_state(
            k,
            &icons::BUDGETS,
            "No expense categories",
            "Add categories from Manage categories to give them a limit.",
        )));
    } else {
        for display in &groups {
            let value = spend.get(display).copied().unwrap_or(0.0);
            list = list.child(budget_row(s, display, value, k));
        }
    }

    let hint = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(rune::icon(&icons::CHECK, 13.0).color(k.c.icon_faint))
        .child(
            text("Type a limit and press Enter or the check to save it. An empty field clears the limit.")
                .font(k.ty.caption())
                .color(k.c.text_faint),
        );

    let body = col()
        .w_full()
        .gap(14.0)
        .child(header)
        .child(summary)
        .child(hint)
        .child(list);

    page("budgets", s, k, body)
}

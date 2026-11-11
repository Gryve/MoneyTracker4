//! Transactions: every record of the selected month, newest first.

use rune::{col, row, text, El};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit};

use crate::icons;
use crate::model;
use crate::state::App;

use super::page;
use super::widgets::{
    amount_style, empty_state, month_picker, page_header, panel, section_header, transaction_row,
};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let count = s.transactions.len();
    let subtitle = format!(
        "{} · {} record{}",
        s.month_label(),
        count,
        if count == 1 { "" } else { "s" }
    );

    let add = Button::new("Add")
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .icon(&icons::PLUS)
        .on_press(|s: &mut App| s.open_add())
        .build(k);

    let header = page_header(
        k,
        "Transactions",
        &subtitle,
        Some(month_picker(s, k, layer)),
    );

    let income: f64 = s
        .transactions
        .iter()
        .filter(|t| t.kind == model::Kind::Income)
        .map(|t| t.amount)
        .sum();
    let expense: f64 = s
        .transactions
        .iter()
        .filter(|t| t.kind == model::Kind::Expense)
        .map(|t| t.amount)
        .sum();

    let totals = row()
        .w_full()
        .gap(18.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(rune::icon(&icons::ARROW_IN, 14.0).color(k.c.success.fg))
                .child(
                    text(model::euros(income))
                        .font(amount_style(k, 13.0))
                        .color(k.c.success.fg),
                ),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(rune::icon(&icons::ARROW_OUT, 14.0).color(k.c.danger.fg))
                .child(
                    text(model::euros(expense))
                        .font(amount_style(k, 13.0))
                        .color(k.c.danger.fg),
                ),
        )
        .child(rune::el().grow(1.0))
        .child(add);

    let mut list = panel(k).w_full().padding(6.0).gap(2.0);
    if s.transactions.is_empty() {
        list = list.child(empty_state(
            k,
            &icons::TRANSACTIONS,
            "Nothing this month",
            "Add a record and it shows up here with the rest of the month.",
        ));
    } else {
        for t in &s.transactions {
            list = list.child(transaction_row(t, k));
        }
    }

    let body = col()
        .w_full()
        .gap(14.0)
        .child(header)
        .child(totals)
        .child(section_header(
            k,
            "Records",
            "Click a row to edit it; the bin deletes it",
            None,
        ))
        .child(list);

    page("transactions", s, k, body)
}

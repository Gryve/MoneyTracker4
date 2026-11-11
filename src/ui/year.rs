//! Year overview: the twelve months one under the other, with a total row.

use rune::{col, row, text, El};
use rune_kit::Kit;

use crate::model;
use crate::state::App;

use super::page;
use super::widgets::{amount_style, page_header, panel, year_picker};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let header = page_header(
        k,
        "Year overview",
        "Every month at a glance",
        Some(year_picker(s, k, layer)),
    );

    let mut table = panel(k).w_full().padding(6.0).gap(2.0);
    let mut yearly_income = 0.0;
    let mut yearly_expense = 0.0;

    for (month, summary) in &s.year_months {
        yearly_income += summary.total_income;
        yearly_expense += summary.total_expense;
        table = table.child(month_row(*month, summary, k));
    }
    table = table.child(total_row(
        yearly_income,
        yearly_expense,
        yearly_income - yearly_expense,
        k,
    ));

    let body = col().w_full().gap(14.0).child(header).child(table);

    page("year", s, k, body)
}

/// The left column: month or total label.
fn label(text_value: String, ink: rune::Color, k: &Kit) -> El {
    text(text_value).font(k.ty.ui).color(ink).w(120.0)
}

fn income_cell(income: f64, muted: bool, k: &Kit) -> El {
    text(if income > 0.0 {
        format!("+{}", model::euros(income))
    } else {
        model::euros(0.0)
    })
    .font(amount_style(k, 13.0))
    .color(if muted {
        k.c.text_faint
    } else {
        k.c.success.fg
    })
    .w(130.0)
}

fn expense_cell(expense: f64, muted: bool, k: &Kit) -> El {
    text(if expense > 0.0 {
        format!("−{}", model::euros(expense))
    } else {
        model::euros(0.0)
    })
    .font(amount_style(k, 13.0))
    .color(if muted { k.c.text_faint } else { k.c.danger.fg })
    .w(130.0)
}

fn balance_cell(balance: f64, muted: bool, k: &Kit) -> El {
    text(if muted {
        model::euros(balance)
    } else {
        model::signed_money(balance)
    })
    .font(amount_style(k, 14.0))
    .color(if muted {
        k.c.text_faint
    } else if balance < 0.0 {
        k.c.danger.fg
    } else {
        k.c.success.fg
    })
}

fn month_row(month: u32, summary: &model::MonthlySummary, k: &Kit) -> El {
    let income = summary.total_income;
    let expense = summary.total_expense;
    let muted = income == 0.0 && expense == 0.0;

    row()
        .w_full()
        .gap(18.0)
        .padding(12.0)
        .center_y()
        .radius(k.radius.md)
        .child(label(
            model::month_only(month).to_owned(),
            if muted { k.c.text_faint } else { k.c.text_base },
            k,
        ))
        .child(income_cell(income, muted, k))
        .child(expense_cell(expense, muted, k))
        .child(rune::el().grow(1.0))
        .child(balance_cell(summary.balance, muted, k))
}

/// The year's totals, set off from the months above by a hairline.
fn total_row(income: f64, expense: f64, balance: f64, k: &Kit) -> El {
    row()
        .w_full()
        .gap(18.0)
        .padding(12.0)
        .center_y()
        .border(1.0, k.c.border_muted)
        .radius(k.radius.md)
        .fill(k.c.layer_02)
        .child(label("Year total".to_owned(), k.c.text_base, k))
        .child(income_cell(income, false, k))
        .child(expense_cell(expense, false, k))
        .child(rune::el().grow(1.0))
        .child(balance_cell(balance, false, k))
}

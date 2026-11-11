//! Dashboard: the month's totals, the annual budget, and the two breakdown donuts.

use rune::{col, row, El};
use rune_kit::Kit;

use crate::icons;
use crate::model;
use crate::state::App;

use super::page;
use super::widgets::{budget_summary, donut_panel, month_picker, page_header, stat_card};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let income = s.summary.total_income;
    let expense = s.summary.total_expense;
    let balance = s.summary.balance;
    let count = s.summary.transaction_count;
    let months_label = format!(
        "{} · {} transaction{}",
        s.month_label(),
        count,
        if count == 1 { "" } else { "s" }
    );

    let header = page_header(
        k,
        "Dashboard",
        &months_label,
        Some(month_picker(s, k, layer)),
    );

    let income_categories = model::merge_by_display(&s.summary.category_income);
    let expense_categories = model::merge_by_display(&s.summary.category_expenses);

    let stats = row()
        .w_full()
        .gap(12.0)
        .items_stretch()
        .child(stat_card(
            k,
            "Monthly income",
            model::euros(income),
            format!("{} categories", income_categories.len()),
            &icons::ARROW_IN,
            k.c.success.fg,
        ))
        .child(stat_card(
            k,
            "Monthly expenses",
            model::euros(expense),
            format!("{} categories", expense_categories.len()),
            &icons::ARROW_OUT,
            k.c.danger.fg,
        ))
        .child(stat_card(
            k,
            "Monthly balance",
            model::signed_money(balance),
            if balance >= 0.0 {
                "You are in the green".to_owned()
            } else {
                "Spending more than you earn".to_owned()
            },
            &icons::SCALE,
            if balance >= 0.0 {
                k.c.success.fg
            } else {
                k.c.danger.fg
            },
        ));

    let spent = s.month_annual.total_expense;
    let total_budget = s.month_budget_total;
    let annual = budget_summary(
        k,
        &format!("Annual budget {}", s.month_year()),
        spent,
        total_budget,
        if total_budget > 0.0 && spent > total_budget {
            k.c.danger.fg
        } else {
            k.c.text_base
        },
    );

    let charts = row()
        .w_full()
        .gap(12.0)
        .items_stretch()
        .child(donut_panel(
            k,
            "Expense breakdown",
            &format!("Where the money went in {}", s.month_label()),
            expense_categories.into_iter().collect(),
            expense,
        ))
        .child(donut_panel(
            k,
            "Income breakdown",
            &format!("Where the money came from in {}", s.month_label()),
            income_categories.into_iter().collect(),
            income,
        ));

    let body = col()
        .w_full()
        .gap(16.0)
        .child(header)
        .child(stats)
        .child(annual)
        .child(charts);

    page("dashboard", s, k, body)
}

//! Dashboard: the filtered totals, the budget summary, breakdown donuts, and category line charts.

use rune::{col, row, El};
use rune_kit::Kit;

use crate::icons;
use crate::model::{self, Kind};
use crate::state::App;

use super::page;
use super::widgets::{
    budget_summary, dashboard_picker, donut_panel, line_chart_panel, page_header, stat_card,
};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let income = s.dash_summary.total_income;
    let expense = s.dash_summary.total_expense;
    let balance = s.dash_summary.balance;
    let count = s.dash_summary.transaction_count;
    let months_label = format!(
        "{} · {} transaction{}",
        s.dash_filter_label(),
        count,
        if count == 1 { "" } else { "s" }
    );

    let header = page_header(
        k,
        "Dashboard",
        &months_label,
        Some(dashboard_picker(s, k, layer)),
    );

    let income_categories = model::merge_by_display(&s.dash_summary.category_income);
    let expense_categories = model::merge_by_display(&s.dash_summary.category_expenses);

    let stats = row()
        .w_full()
        .gap(12.0)
        .items_stretch()
        .child(stat_card(
            k,
            "Total income",
            model::euros(income),
            format!("{} categories", income_categories.len()),
            &icons::ARROW_IN,
            k.c.success.fg,
        ))
        .child(stat_card(
            k,
            "Total expenses",
            model::euros(expense),
            format!("{} categories", expense_categories.len()),
            &icons::ARROW_OUT,
            k.c.danger.fg,
        ))
        .child(stat_card(
            k,
            "Net balance",
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

    let spent = if s.dash_filter.years.len() == 1 && !s.dash_filter.is_all {
        s.month_annual.total_expense
    } else {
        expense
    };
    let total_budget = s.month_budget_total;
    let budget_label = if s.dash_filter.years.len() == 1 && !s.dash_filter.is_all {
        format!("Annual budget {}", s.dash_filter.years[0])
    } else if s.dash_filter.is_all {
        "Annual budget (All time)".to_owned()
    } else {
        "Budget for selected period".to_owned()
    };
    let annual = budget_summary(
        k,
        &budget_label,
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
            &format!("Where the money went in {}", s.dash_filter_label()),
            expense_categories.into_iter().collect(),
            expense,
        ))
        .child(donut_panel(
            k,
            "Income breakdown",
            &format!("Where the money came from in {}", s.dash_filter_label()),
            income_categories.into_iter().collect(),
            income,
        ));

    let expense_chart = line_chart_panel(s, k, Kind::Expense);
    let income_chart = line_chart_panel(s, k, Kind::Income);

    let body = col()
        .w_full()
        .gap(16.0)
        .child(header)
        .child(stats)
        .child(annual)
        .child(charts)
        .child(expense_chart)
        .child(income_chart);

    page("dashboard", s, k, body)
}

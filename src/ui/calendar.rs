//! Calendar: the month's days with their daily totals, and the selected day's records.

use chrono::Datelike;
use rune::{col, el, row, text, El, ScrollSpec};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit};

use crate::db;
use crate::icons;
use crate::model;
use crate::state::{today_iso_date, App};

use super::widgets::{
    amount_style, empty_state, month_picker, notice_banner, page_header, panel, transaction_row,
};

pub fn view(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let header = page_header(
        k,
        "Calendar",
        "Pick a day to see what you recorded",
        Some(month_picker(s, k, layer)),
    );

    let content = row()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(12.0)
        .items_stretch()
        .child(grid(s, k))
        .child(detail(s, k));

    let mut body = col()
        .key("page-calendar")
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(22.0)
        .gap(14.0)
        .child(header);
    if let Some(notice) = &s.notice {
        body = body.child(notice_banner(k, notice));
    }
    body.child(content)
}

fn grid(s: &App, k: &Kit) -> El {
    let days = db::month_days(s.month_year(), s.month_number());
    let lead = days
        .first()
        .map(|d| d.weekday().num_days_from_monday() as usize)
        .unwrap_or(0);
    let today = today_iso_date();
    let selected = s.selected_day;

    let mut head = row().w_full().gap(4.0);
    for name in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
        head = head.child(
            text(name)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .grow(1.0),
        );
    }

    let mut weeks: Vec<El> = Vec::new();
    let mut week = row().w_full().gap(4.0).grow(1.0).min_h(0.0);
    let mut column = 0usize;
    for _ in 0..lead {
        week = week.child(el().grow(1.0));
        column += 1;
    }
    for day in days {
        if column == 7 {
            weeks.push(week);
            week = row().w_full().gap(4.0).grow(1.0).min_h(0.0);
            column = 0;
        }
        let iso = day.format("%Y-%m-%d").to_string();
        let daily = s.summary.daily.get(&iso).copied();
        week = week.child(cell(
            day,
            &iso,
            daily,
            today == day,
            selected == Some(day),
            k,
        ));
        column += 1;
    }
    while column < 7 {
        week = week.child(el().grow(1.0));
        column += 1;
    }
    weeks.push(week);

    panel(k)
        .grow(3.0)
        .padding(12.0)
        .gap(4.0)
        .child(head)
        .children(weeks)
}

fn cell(
    day: chrono::NaiveDate,
    iso: &str,
    daily: Option<(f64, f64)>,
    today: bool,
    selected: bool,
    k: &Kit,
) -> El {
    let ink = if selected {
        k.c.text_base
    } else if today {
        k.c.info.fg
    } else {
        k.c.text_base
    };
    let border = if selected {
        k.c.border_focus
    } else if today {
        k.c.border_strong
    } else {
        k.c.border_muted
    };

    let mut c = col()
        .grow(1.0)
        .min_w(0.0)
        .min_h(0.0)
        .padding(6.0)
        .gap(1.0)
        .radius(k.radius.md)
        .fill(if selected { k.c.layer_03 } else { k.c.layer_01 })
        .border(1.0, border)
        .hover(k.c.hover)
        .on_press(move |s: &mut App| s.select_day(day))
        .key(format!("day-{iso}"))
        .label(model::long_date(day))
        .child(
            text(day.day().to_string())
                .font(k.ty.small_medium)
                .color(ink),
        );

    if let Some((income, expense)) = daily {
        if income > 0.0 {
            c = c.child(
                text(format!("+{income:.0}"))
                    .font(k.ty.caption())
                    .color(k.c.success.fg)
                    .truncate(),
            );
        }
        if expense > 0.0 {
            c = c.child(
                text(format!("-{expense:.0}"))
                    .font(k.ty.caption())
                    .color(k.c.danger.fg)
                    .truncate(),
            );
        }
    }
    c
}

fn detail(s: &App, k: &Kit) -> El {
    let Some(day) = s.selected_day else {
        return panel(k).grow(2.0).padding(14.0).child(empty_state(
            k,
            &icons::CALENDAR,
            "No day selected",
            "Press a day on the calendar to see its records.",
        ));
    };

    let income: f64 = s
        .day_transactions
        .iter()
        .filter(|t| t.kind == model::Kind::Income)
        .map(|t| t.amount)
        .sum();
    let expense: f64 = s
        .day_transactions
        .iter()
        .filter(|t| t.kind == model::Kind::Expense)
        .map(|t| t.amount)
        .sum();

    let mut totals = row().w_full().gap(10.0).items_stretch();
    if income > 0.0 {
        totals = totals.child(
            row()
                .grow(1.0)
                .gap(6.0)
                .center_y()
                .padding(10.0)
                .radius(k.radius.md)
                .fill(k.on(k.c.success.fg.alpha(0.10), k.c.layer_01))
                .child(rune::icon(&icons::ARROW_IN, 14.0).color(k.c.success.fg))
                .child(
                    text(model::signed_money(income))
                        .font(amount_style(k, 14.0))
                        .color(k.c.success.fg),
                ),
        );
    }
    if expense > 0.0 {
        totals = totals.child(
            row()
                .grow(1.0)
                .gap(6.0)
                .center_y()
                .padding(10.0)
                .radius(k.radius.md)
                .fill(k.on(k.c.danger.fg.alpha(0.10), k.c.layer_01))
                .child(rune::icon(&icons::ARROW_OUT, 14.0).color(k.c.danger.fg))
                .child(
                    text(format!("−{}", model::euros(expense)))
                        .font(amount_style(k, 14.0))
                        .color(k.c.danger.fg),
                ),
        );
    }

    let add = Button::new("Add")
        .variant(ButtonVariant::Ghost)
        .control_size(ControlSize::Small)
        .icon(&icons::PLUS)
        .on_press(|s: &mut App| s.open_add())
        .build(k);

    let mut list = col()
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(2.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    if s.day_transactions.is_empty() {
        list = list.child(empty_state(
            k,
            &icons::TRANSACTIONS,
            "Nothing recorded",
            "No income or expense on this day.",
        ));
    } else {
        for t in &s.day_transactions {
            list = list.child(transaction_row(t, k));
        }
    }

    panel(k)
        .grow(2.0)
        .padding(14.0)
        .gap(10.0)
        .child(
            row()
                .w_full()
                .justify_between()
                .center_y()
                .child(
                    col()
                        .gap(2.0)
                        .child(
                            text(model::long_date(day))
                                .font(k.ty.ui_medium)
                                .color(k.c.text_base),
                        )
                        .child(
                            text(day.format("%Y-%m-%d").to_string())
                                .font(k.ty.caption())
                                .color(k.c.text_faint),
                        ),
                )
                .child(add),
        )
        .children(Some(totals).filter(|_| income > 0.0 || expense > 0.0))
        .child(list)
}

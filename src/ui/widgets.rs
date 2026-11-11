//! Reusable pieces: page headers, stat cards, donut, transaction rows, budget rows, navigation.

use rune::{canvas, col, el, icon, oklch, row, text, Color, El, Shape, TextStyle};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Select,
    SelectAppearance, TextInput,
};

use crate::icons;
use crate::model::{self, Kind, Transaction};
use crate::state::{App, Picker};

/// Amounts: semibold, tabular figures so the columns line up.
pub fn amount_style(k: &Kit, size: f32) -> TextStyle {
    TextStyle {
        size,
        weight: 600.0,
        features: "'tnum' 1",
        ..k.ty.ui
    }
}

/// The colour of money: income is success, expense is danger.
pub fn kind_color(k: &Kit, kind: Kind) -> Color {
    match kind {
        Kind::Income => k.c.success.fg,
        Kind::Expense => k.c.danger.fg,
    }
}

/// A card surface: rounded, hairline border, one layer above the page.
pub fn panel(k: &Kit) -> El {
    col()
        .radius(k.radius.md)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_muted)
}

/// A square tinted icon plate, as the sidebar and cards use.
pub fn icon_chip(k: &Kit, i: &'static rune::Icon, ink: Color, size: f32) -> El {
    row()
        .size(size, size)
        .radius(size * 0.3)
        .center()
        .fill(k.on(ink.alpha(0.14), k.c.layer_01))
        .child(icon(i, size * 0.6).color(ink))
}

/// The heading of a view: a large title, a subtitle, and the controls on the right.
pub fn page_header(k: &Kit, title: &str, subtitle: &str, trailing: Option<El>) -> El {
    let mut head = row().w_full().gap(16.0).center_y().justify_between();
    head = head.child(
        col()
            .gap(2.0)
            .child(
                text(title.to_owned())
                    .font(TextStyle {
                        size: 21.0,
                        weight: 600.0,
                        tracking: -0.3,
                        ..k.ty.ui
                    })
                    .color(k.c.text_base),
            )
            .child(
                text(subtitle.to_owned())
                    .font(k.ty.caption())
                    .color(k.c.text_faint),
            ),
    );
    if let Some(trailing) = trailing {
        head = head.child(trailing);
    }
    head
}

pub fn stat_card(
    k: &Kit,
    title: &str,
    value: String,
    detail: String,
    i: &'static rune::Icon,
    ink: Color,
) -> El {
    panel(k)
        .grow(1.0)
        .padding(16.0)
        .gap(10.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon_chip(k, i, ink, 24.0))
                .child(
                    text(title.to_owned())
                        .font(k.ty.caption())
                        .color(k.c.text_muted),
                ),
        )
        .child(text(value).font(amount_style(k, 24.0)).color(ink))
        .child(
            text(detail)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .truncate(),
        )
}

/// A horizontal meter; `ratio` above 1 fills the bar, as the over-budget state does.
pub fn meter(k: &Kit, ratio: f32, ink: Color, height: f32) -> El {
    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = row()
        .w_full()
        .h(height)
        .radius(height / 2.0)
        .fill(k.on(k.c.text_base.alpha(0.08), k.c.layer_01));
    if ratio <= 0.0 {
        return track;
    }
    if ratio >= 1.0 {
        return track.child(row().grow(1.0).h_full().radius(height / 2.0).fill(ink));
    }
    track
        .child(row().grow(ratio).h_full().radius(height / 2.0).fill(ink))
        .child(row().grow(1.0 - ratio))
}

/// The ring of a donut: thick arc strokes, one per slice, drawn in a 150 point box.
fn ring(slices: &[(String, f64)], total: f64, k: &Kit) -> Shape {
    const SIZE: f32 = 150.0;
    let (cx, cy) = (SIZE / 2.0, SIZE / 2.0);
    let radius = 59.0;
    let width = 20.0;
    let mut shape = Shape::new();
    if total <= 0.0 || slices.is_empty() {
        let points: Vec<[f32; 2]> = (0..=96)
            .map(|s| {
                let a = std::f32::consts::TAU * s as f32 / 96.0;
                [cx + radius * a.cos(), cy + radius * a.sin()]
            })
            .collect();
        return shape
            .stroke(points, width - 4.0, k.c.border_muted)
            .view_box(SIZE, SIZE);
    }
    let gap = if slices.len() > 1 { 0.02 } else { 0.0 };
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for (i, (_, value)) in slices.iter().enumerate() {
        let sweep = (value / total) as f32 * std::f32::consts::TAU;
        if sweep <= 0.001 {
            continue;
        }
        let drawn = (sweep - gap).max(sweep * 0.35);
        let steps = ((drawn / 0.06).ceil() as usize).max(3);
        let points: Vec<[f32; 2]> = (0..=steps)
            .map(|s| {
                let a = angle + drawn * (s as f32 / steps as f32);
                [cx + radius * a.cos(), cy + radius * a.sin()]
            })
            .collect();
        shape = shape.stroke(points, width, slice_color(k, i));
        angle += sweep;
    }
    shape.view_box(SIZE, SIZE)
}

/// The slice palette: hues spread round the wheel at one lightness, so a series reads as a set.
pub fn slice_color(k: &Kit, i: usize) -> Color {
    let hue = (162.0 + i as f32 * 47.0) % 360.0;
    if k.dark {
        oklch(0.70, 0.125, hue)
    } else {
        oklch(0.60, 0.145, hue)
    }
}

/// A donut with its legend. `data` is `(label, amount)`; at most 6 slices plus "Other".
pub fn donut_panel(
    k: &Kit,
    title: &str,
    subtitle: &str,
    data: Vec<(String, f64)>,
    total: f64,
) -> El {
    let mut slices: Vec<(String, f64)> = data.into_iter().filter(|(_, v)| *v > 0.0).collect();
    slices.sort_by(|a, b| b.1.total_cmp(&a.1));
    if slices.len() > 6 {
        let rest: f64 = slices.iter().skip(5).map(|(_, v)| *v).sum();
        let mut head: Vec<(String, f64)> = slices.into_iter().take(5).collect();
        head.push(("Other".to_owned(), rest));
        slices = head;
    }

    let ring_el = el()
        .size(150.0, 150.0)
        .child(canvas(ring(&slices, total, k)).size(150.0, 150.0))
        .child(
            col()
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .center()
                .gap(2.0)
                .child(text("Total").font(k.ty.caption()).color(k.c.text_faint))
                .child(
                    text(model::euros(total))
                        .font(amount_style(k, 15.0))
                        .color(k.c.text_base),
                ),
        );

    let legend = {
        let mut legend = col().grow(1.0).gap(8.0);
        if slices.is_empty() {
            legend = legend.child(
                text("Nothing recorded yet")
                    .font(k.ty.ui)
                    .color(k.c.text_faint),
            );
        }
        for (i, (label, value)) in slices.iter().enumerate() {
            let pct = if total > 0.0 {
                value / total * 100.0
            } else {
                0.0
            };
            legend = legend.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(row().size(8.0, 8.0).radius(4.0).fill(slice_color(k, i)))
                    .child(
                        text(label.clone())
                            .font(k.ty.small)
                            .color(k.c.text_base)
                            .truncate()
                            .grow(1.0),
                    )
                    .child(
                        text(format!("{pct:.0}%"))
                            .font(k.ty.caption())
                            .color(k.c.text_faint),
                    )
                    .child(
                        text(model::euros(*value))
                            .font(amount_style(k, 12.0))
                            .color(k.c.text_muted),
                    ),
            );
        }
        legend
    };

    panel(k)
        .grow(1.0)
        .padding(16.0)
        .gap(12.0)
        .child(
            col()
                .gap(2.0)
                .child(
                    text(title.to_owned())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base),
                )
                .child(
                    text(subtitle.to_owned())
                        .font(k.ty.caption())
                        .color(k.c.text_faint),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(20.0)
                .center_y()
                .child(ring_el)
                .child(legend),
        )
}

/// The annual budget card: spent against the sum of every limit, with a wide meter.
pub fn budget_summary(k: &Kit, title: &str, spent: f64, total: f64, spent_color: Color) -> El {
    let ratio = if total > 0.0 {
        (spent / total) as f32
    } else {
        0.0
    };
    let ink = if total <= 0.0 {
        k.c.text_faint
    } else if ratio >= 1.0 {
        k.c.danger.fg
    } else if ratio >= 0.8 {
        k.c.warning.fg
    } else {
        k.c.info.fg
    };
    let value = if total > 0.0 {
        format!("{} / {}", model::euros(spent), model::euros(total))
    } else {
        "No budgets set".to_owned()
    };

    panel(k)
        .w_full()
        .padding(16.0)
        .gap(12.0)
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(icon_chip(k, &icons::BUDGETS, k.c.info.fg, 30.0))
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(
                            text(title.to_owned())
                                .font(k.ty.ui_medium)
                                .color(k.c.text_base),
                        )
                        .child(
                            text(if total > 0.0 {
                                if spent > total {
                                    format!("{} over the annual plan", model::euros(spent - total))
                                } else {
                                    format!(
                                        "{} left of the annual plan",
                                        model::euros(total - spent)
                                    )
                                }
                            } else {
                                "Set a limit per category in Budgets".to_owned()
                            })
                            .font(k.ty.caption())
                            .color(k.c.text_faint),
                        ),
                )
                .child(text(value).font(amount_style(k, 15.0)).color(spent_color)),
        )
        .child(meter(k, ratio, ink, 8.0))
}

/// One transaction, clickable to edit, with a delete button.
pub fn transaction_row(t: &Transaction, k: &Kit) -> El {
    let display = model::display_name(&t.category);
    let ink = kind_color(k, t.kind);
    let amount = format!("{}{}", t.kind.sign(), model::euros(t.amount));
    let subtitle = if t.description.trim().is_empty() {
        t.date.clone()
    } else {
        format!("{} · {}", t.date, t.description.trim())
    };

    let edit_source = t.clone();
    let delete_source = t.clone();

    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .px(10.0)
        .py(8.0)
        .radius(k.radius.md)
        .hover(k.c.hover)
        .press(k.c.pressed)
        .on_press(move |s: &mut App| s.open_edit(&edit_source))
        .child(icon_chip(
            k,
            icons::for_category(&display, t.kind),
            ink,
            30.0,
        ))
        .child(
            col()
                .grow(1.0)
                .gap(2.0)
                .child(text(display).font(k.ty.ui).color(k.c.text_base).truncate())
                .child(
                    text(subtitle)
                        .font(k.ty.caption())
                        .color(k.c.text_faint)
                        .truncate(),
                ),
        )
        .child(text(amount).font(amount_style(k, 14.0)).color(ink))
        .child(
            IconButton::new(&icons::TRASH)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Delete transaction")
                .on_press(move |s: &mut App| s.ask_delete(&delete_source))
                .build(k),
        )
}

/// The month and year pickers above every month-scoped view, with a Today shortcut.
pub fn month_picker(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let months = model::month_names();
    let years: Vec<String> = s.year_options().iter().map(|y| y.to_string()).collect();

    let month = Select::new("picker-month", &months)
        .appearance(SelectAppearance::Field)
        .width(150.0)
        .label("Month")
        .current(Some(model::month_only(s.month_number())))
        .open(s.picker_open == Some(Picker::Month))
        .on_toggle(|s: &mut App| s.toggle_picker(Picker::Month))
        .on_close(|s: &mut App| s.close_picker())
        .on_choose(|s: &mut App, value, _| {
            if let Some(i) = model::month_names().iter().position(|m| *m == value) {
                s.set_month(s.month_year(), (i + 1) as u32);
            }
        })
        .build(k, layer);

    let current = format!("{}", s.month_year());
    let year = Select::new("picker-year", &years)
        .appearance(SelectAppearance::Field)
        .width(92.0)
        .label("Year")
        .current(Some(&current))
        .open(s.picker_open == Some(Picker::Year))
        .on_toggle(|s: &mut App| s.toggle_picker(Picker::Year))
        .on_close(|s: &mut App| s.close_picker())
        .on_choose(|s: &mut App, value, _| {
            if let Ok(year) = value.parse::<i32>() {
                s.set_month(year, s.month_number());
            }
        })
        .build(k, layer);

    row()
        .gap(8.0)
        .center_y()
        .child(month)
        .child(year)
        .child(
            Button::new("Today")
                .variant(ButtonVariant::Ghost)
                .control_size(ControlSize::Small)
                .on_press(|s: &mut App| s.go_today())
                .build(k),
        )
}

/// The year picker for the budgets and year overview.
pub fn year_picker(s: &App, k: &Kit, layer: &mut Vec<El>) -> El {
    let years: Vec<String> = s.year_options().iter().map(|y| y.to_string()).collect();
    let current = format!("{}", s.year);

    Select::new("year-picker", &years)
        .appearance(SelectAppearance::Field)
        .width(92.0)
        .label("Year")
        .current(Some(&current))
        .open(s.picker_open == Some(Picker::Year))
        .on_toggle(|s: &mut App| s.toggle_picker(Picker::Year))
        .on_close(|s: &mut App| s.close_picker())
        .on_choose(|s: &mut App, value, _| {
            if let Ok(year) = value.parse::<i32>() {
                s.set_year(year);
            }
        })
        .build(k, layer)
}

pub fn empty_state(k: &Kit, i: &'static rune::Icon, title: &str, hint: &str) -> El {
    col()
        .w_full()
        .grow(1.0)
        .min_h(160.0)
        .center()
        .gap(8.0)
        .padding(24.0)
        .child(icon(i, 26.0).color(k.c.icon_faint))
        .child(
            text(title.to_owned())
                .font(k.ty.ui_medium)
                .color(k.c.text_muted),
        )
        .child(
            text(hint.to_owned())
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .wrap(),
        )
}

/// A warning strip: the app's last failure, above the view it happened in.
pub fn notice_banner(k: &Kit, message: &str) -> El {
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .padding(10.0)
        .radius(k.radius.md)
        .fill(k.on(k.c.warning.fg.alpha(0.12), k.c.layer_01))
        .border(1.0, k.c.warning.fg.alpha(0.35))
        .child(icon(&icons::ALERT, 14.0).color(k.c.warning.fg))
        .child(
            text(message.to_owned())
                .font(k.ty.caption())
                .color(k.c.text_base)
                .wrap()
                .grow(1.0),
        )
}

/// A section heading with an optional action on the right.
pub fn section_header(k: &Kit, title: &str, subtitle: &str, action: Option<El>) -> El {
    let mut head = row().w_full().center_y().justify_between().gap(12.0);
    head = head.child(
        col()
            .gap(2.0)
            .child(
                text(title.to_owned())
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base),
            )
            .child(
                text(subtitle.to_owned())
                    .font(k.ty.caption())
                    .color(k.c.text_faint),
            ),
    );
    if let Some(action) = action {
        head = head.child(action);
    }
    head
}

/// One budget row: category, spend against the limit, and the editable annual limit.
pub fn budget_row(s: &App, display: &str, spent: f64, k: &Kit) -> El {
    let icon = icons::for_category(display, Kind::Expense);
    let budget = s.budgets.get(display).copied().unwrap_or(0.0);
    let ratio = if budget > 0.0 {
        (spent / budget) as f32
    } else {
        0.0
    };
    let ink = if budget <= 0.0 {
        k.c.text_faint
    } else if ratio >= 1.0 {
        k.c.danger.fg
    } else if ratio >= 0.8 {
        k.c.warning.fg
    } else {
        k.c.info.fg
    };
    let state_text = if budget <= 0.0 {
        "No limit".to_owned()
    } else if spent > budget {
        format!("{} over", model::euros(spent - budget))
    } else {
        format!("{} left", model::euros(budget - spent))
    };

    let key = display.to_owned();
    let submit_key = display.to_owned();
    let press_key = display.to_owned();
    let edit = s
        .budget_edits
        .get(display)
        .cloned()
        .unwrap_or_else(|| rune::TextEdit::new(""));

    panel(k)
        .w_full()
        .padding(14.0)
        .gap(10.0)
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(icon_chip(k, icon, ink, 30.0))
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(text(display.to_owned()).font(k.ty.ui).color(k.c.text_base))
                        .child(
                            text(format!("{} spent", model::euros(spent)))
                                .font(k.ty.caption())
                                .color(k.c.text_faint),
                        ),
                )
                .child(
                    row()
                        .gap(6.0)
                        .center_y()
                        .child(text("€").font(k.ty.small).color(k.c.text_faint))
                        .child(
                            TextInput::new(&edit)
                                .placeholder("0.00")
                                .width(Some(92.0))
                                .label(format!("Annual budget for {display}"))
                                .on_edit(move |s: &mut App| {
                                    s.budget_edits
                                        .entry(key.clone())
                                        .or_insert_with(|| rune::TextEdit::new(""))
                                })
                                .on_submit(move |s: &mut App, _| s.commit_budget(&submit_key))
                                .build(k)
                                .key(format!("budget-{display}")),
                        )
                        .child(
                            IconButton::new(&icons::CHECK)
                                .variant(IconButtonVariant::GhostMuted)
                                .control_size(ControlSize::Small)
                                .label("Save budget")
                                .on_press(move |s: &mut App| s.commit_budget(&press_key))
                                .build(k),
                        ),
                ),
        )
        .child(meter(k, ratio, ink, 6.0))
        .child(
            row()
                .w_full()
                .justify_between()
                .child(
                    text(if budget > 0.0 {
                        format!("{} of {}", model::euros(spent), model::euros(budget))
                    } else {
                        format!("{} spent this year", model::euros(spent))
                    })
                    .font(k.ty.caption())
                    .color(k.c.text_faint),
                )
                .child(text(state_text).font(k.ty.caption()).color(ink)),
        )
}

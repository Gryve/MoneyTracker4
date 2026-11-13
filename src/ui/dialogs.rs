//! Modals: add or edit a transaction, manage categories, confirm a deletion.
//!
//! Rune's [`Dialog`] does focus, Escape and background inertness; whether one is open is on
//! [`App::dialog`]. Everything is built into the kit's overlay layer, above the shell.

use rune::{col, el, icon, row, text, El, FileDialogOptions, Tx};
use rune_kit::{
    Button, ButtonVariant, Checkbox, ControlSize, Dialog, IconButton, IconButtonVariant, Kit,
    SegmentedControl, TextInput,
};

use crate::export;
use crate::icons;
use crate::model::{self, Kind};
use crate::state::{App, Dialog as Modal};

type Layer = Vec<El>;

pub fn mount(s: &App, k: &Kit, layer: &mut Layer) {
    match &s.dialog {
        None => {}
        Some(Modal::Transaction { edit_id, error }) => {
            transaction(s, k, layer, edit_id.is_some(), error.as_deref())
        }
        Some(Modal::Categories { error }) => categories(s, k, layer, error.as_deref()),
        Some(Modal::Export { error }) => export(s, k, layer, error.as_deref()),
        Some(Modal::ConfirmDelete { label, .. }) => confirm(k, layer, label),
    }
}

fn field(k: &Kit, label: &str, control: El) -> El {
    col()
        .w_full()
        .gap(4.0)
        .child(
            text(label.to_owned())
                .font(k.ty.caption())
                .color(k.c.text_muted),
        )
        .child(control)
}

fn error_line(k: &Kit, error: Option<&str>) -> Option<El> {
    error.map(|message| {
        text(message.to_owned())
            .font(k.ty.caption())
            .color(k.c.danger.fg)
            .wrap()
    })
}

fn buttons(cancel: El, confirm: El) -> El {
    row()
        .w_full()
        .gap(8.0)
        .justify_end()
        .child(cancel)
        .child(confirm)
}

fn transaction(s: &App, k: &Kit, layer: &mut Layer, editing: bool, error: Option<&str>) {
    let selected = if s.tx_form.kind == Kind::Expense {
        0
    } else {
        1
    };
    let kind = SegmentedControl::new(&["Expense", "Income"], Some(selected))
        .label("Type")
        .width(Some(240.0))
        .on_pick(|s: &mut App, i, _| {
            s.set_form_kind(if i == 0 { Kind::Expense } else { Kind::Income })
        })
        .build(k);

    let amount = TextInput::new(&s.tx_form.amount)
        .placeholder("0.00")
        .label("Amount")
        .leading_icon(&icons::COIN)
        .on_edit(|s: &mut App| &mut s.tx_form.amount)
        .on_submit(|s: &mut App, _| s.submit_transaction())
        .width(None)
        .build(k);

    let date = TextInput::new(&s.tx_form.date)
        .placeholder("YYYY-MM-DD")
        .label("Date")
        .leading_icon(&icons::CALENDAR)
        .on_edit(|s: &mut App| &mut s.tx_form.date)
        .on_submit(|s: &mut App, _| s.submit_transaction())
        .width(None)
        .build(k);

    let description = TextInput::new(&s.tx_form.description)
        .placeholder("What was it? (optional)")
        .label("Description")
        .leading_icon(&icons::PENCIL)
        .on_edit(|s: &mut App| &mut s.tx_form.description)
        .on_submit(|s: &mut App, _| s.submit_transaction())
        .width(None)
        .build(k);

    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::Ghost)
        .on_press(|s: &mut App| s.close_dialog())
        .build(k);
    let save = Button::new(if editing {
        "Save changes"
    } else {
        "Add transaction"
    })
    .variant(ButtonVariant::Contrast)
    .icon(&icons::CHECK)
    .on_press(|s: &mut App| s.submit_transaction())
    .build(k);

    let mut body = col().w_full().gap(12.0);
    body = body
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .justify_between()
                .child(text("Type").font(k.ty.caption()).color(k.c.text_muted))
                .child(kind),
        )
        .child(field(k, "Amount", amount))
        .child(category_picker(s, k))
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_start()
                .child(field(k, "Date", date).grow(1.0))
                .child(field(k, "Description", description).grow(2.0)),
        );
    if let Some(err) = error_line(k, error) {
        body = body.child(err);
    }
    body = body.child(buttons(cancel, save));

    Dialog::new(
        "transaction-dialog",
        if editing {
            "Edit transaction"
        } else {
            "New transaction"
        },
    )
    .description(if editing {
        "Change the record and save."
    } else {
        "Amount in euros; the date is YYYY-MM-DD."
    })
    .open(true)
    .width(520.0)
    .fit()
    .on_close(|s: &mut App| s.close_dialog())
    .child(body)
    .build(k, layer);
}

fn category_picker(s: &App, k: &Kit) -> El {
    let kind = s.tx_form.kind;
    let groups = model::display_groups(s.categories.of(kind));
    let selected = s.tx_form.category.clone();

    let mut rows = col().w_full().gap(6.0);
    if groups.is_empty() {
        rows = rows.child(
            text("No categories of this kind. Add some from Manage categories.")
                .font(k.ty.caption())
                .color(k.c.text_faint),
        );
    } else {
        for pair in groups.chunks(2) {
            let mut line = row().w_full().gap(6.0);
            for name in pair {
                line = line.child(chip(
                    name,
                    selected.as_deref() == Some(name.as_str()),
                    kind,
                    k,
                ));
            }
            if pair.len() == 1 {
                line = line.child(el().grow(1.0));
            }
            rows = rows.child(line);
        }
    }

    field(k, "Category", rows)
}

fn chip(name: &str, selected: bool, kind: Kind, k: &Kit) -> El {
    let shown = name.to_owned();
    let press = shown.clone();
    let ink = if selected {
        k.c.text_base
    } else {
        k.c.text_muted
    };
    row()
        .grow(1.0)
        .h(30.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(k.radius.md)
        .fill(if selected {
            k.on(k.c.info.fg.alpha(0.16), k.c.layer_01)
        } else {
            k.c.layer_01
        })
        .border(
            1.0,
            if selected {
                k.c.border_focus
            } else {
                k.c.border_muted
            },
        )
        .hover(k.c.hover)
        .press(k.c.pressed)
        .role("radio")
        .label(name)
        .on_press(move |s: &mut App| s.tx_form.category = Some(press.clone()))
        .child(
            icon(icons::for_category(name, kind), 14.0).color(if selected {
                k.c.info.fg
            } else {
                k.c.icon_faint
            }),
        )
        .child(text(shown).font(k.ty.small).color(ink).truncate())
}

/// One entry of the category manager: icon, display name, remove button.
fn category_item(stored: &str, kind: Kind, k: &Kit) -> El {
    let display = model::display_name(stored);
    let remove = stored.to_owned();
    row()
        .grow(1.0)
        .min_w(0.0)
        .h(30.0)
        .gap(6.0)
        .center_y()
        .px(8.0)
        .radius(k.radius.sm)
        .hover(k.c.hover)
        .child(icon(icons::for_category(&display, kind), 14.0).color(k.c.icon_faint))
        .child(
            text(display)
                .font(k.ty.small)
                .color(k.c.text_base)
                .grow(1.0)
                .truncate(),
        )
        .child(
            IconButton::new(&icons::TRASH)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Mini)
                .label(format!("Remove {stored}"))
                .on_press(move |s: &mut App| s.remove_category(kind, &remove))
                .build(k),
        )
}

fn categories(s: &App, k: &Kit, layer: &mut Layer, error: Option<&str>) {
    let kind = s.cats_form.kind;
    let selected = if kind == Kind::Expense { 0 } else { 1 };
    let switch = SegmentedControl::new(&["Expense", "Income"], Some(selected))
        .label("Category kind")
        .width(Some(240.0))
        .on_pick(|s: &mut App, i, _| {
            s.set_cats_kind(if i == 0 { Kind::Expense } else { Kind::Income })
        })
        .build(k);

    let names: Vec<String> = s.cats_form.draft.of(kind).clone();
    let mut list = col()
        .w_full()
        .gap(3.0)
        .max_h(260.0)
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        });
    if names.is_empty() {
        list = list.child(
            text("No categories yet.")
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .padding(8.0),
        );
    } else {
        // Two columns, so the seeded set fits without scrolling.
        for pair in names.chunks(2) {
            let mut line = row().w_full().gap(6.0);
            for stored in pair {
                line = line.child(category_item(stored, kind, k));
            }
            if pair.len() == 1 {
                line = line.child(el().grow(1.0));
            }
            list = list.child(line);
        }
    }

    let new_name = TextInput::new(&s.cats_form.new_name)
        .placeholder("New category name")
        .label("New category")
        .width(None)
        .on_edit(|s: &mut App| &mut s.cats_form.new_name)
        .on_submit(|s: &mut App, _| s.add_category())
        .build(k);
    let add = Button::new("Add")
        .variant(ButtonVariant::Neutral)
        .icon(&icons::PLUS)
        .on_press(|s: &mut App| s.add_category())
        .build(k);

    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::Ghost)
        .on_press(|s: &mut App| s.close_dialog())
        .build(k);
    let save = Button::new("Save categories")
        .variant(ButtonVariant::Contrast)
        .icon(&icons::CHECK)
        .on_press(|s: &mut App| s.save_categories())
        .build(k);

    let mut body = col().w_full().gap(12.0);
    body = body
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .justify_between()
                .child(text("Kind").font(k.ty.caption()).color(k.c.text_muted))
                .child(switch),
        )
        .child(field(k, &format!("Categories · {}", names.len()), list))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .items_end()
                .child(new_name.grow(1.0))
                .child(add),
        );
    if let Some(err) = error_line(k, error) {
        body = body.child(err);
    }
    body = body.child(buttons(cancel, save));

    Dialog::new("categories-dialog", "Manage categories")
        .description("Categories are stored by name; the icon comes from the name.")
        .open(true)
        .width(560.0)
        .fit()
        .on_close(|s: &mut App| s.close_dialog())
        .child(body)
        .build(k, layer);
}

fn export(s: &App, k: &Kit, layer: &mut Layer, error: Option<&str>) {
    let from = TextInput::new(&s.export_form.from_year)
        .placeholder("es. 2023")
        .label("Da anno")
        .leading_icon(&icons::CALENDAR)
        .on_edit(|s: &mut App| &mut s.export_form.from_year)
        .width(None)
        .build(k);

    let to = TextInput::new(&s.export_form.to_year)
        .placeholder("es. 2026")
        .label("A anno")
        .leading_icon(&icons::CALENDAR)
        .on_edit(|s: &mut App| &mut s.export_form.to_year)
        .width(None)
        .build(k);

    let all_months = s.export_form.all_months();
    let month_grid = {
        let months: Vec<u32> = (1..=12).collect();
        let mut grid = col().w_full().gap(8.0);
        for chunk in months.chunks(4) {
            let mut line = row().w_full().gap(12.0).items_center();
            for &month in chunk {
                line = line.child(
                    Checkbox::new(s.export_form.month_selected(month))
                        .label(export::month_name(month))
                        .on_toggle(move |s: &mut App| s.export_form.toggle_month(month))
                        .build(k)
                        .grow(1.0),
                );
            }
            grid = grid.child(line);
        }
        grid
    };
    let months = col()
        .w_full()
        .gap(10.0)
        .child(
            Checkbox::new(all_months)
                .label("Tutti")
                .indeterminate(!all_months && !s.export_form.no_months())
                .on_toggle(|s: &mut App| s.export_form.toggle_all_months())
                .build(k),
        )
        .child(month_grid);

    let cancel = Button::new("Annulla")
        .variant(ButtonVariant::Ghost)
        .on_press(|s: &mut App| s.close_dialog())
        .build(k);

    let confirm = Button::new("Esporta")
        .variant(ButtonVariant::Contrast)
        .icon(&icons::DOWNLOAD)
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            let period = match s.export_period() {
                Ok(period) => period,
                Err(message) => {
                    s.set_export_error(&message);
                    return;
                }
            };
            s.dialog = None;

            let options = FileDialogOptions {
                title: "Esporta in Excel".to_owned(),
                file_name: Some(App::default_export_name()),
                ..Default::default()
            }
            .filter("Excel", &["xlsx"]);

            tx.save_file(options, move |s: &mut App, result, _| match result {
                Ok(Some(mut path)) => {
                    if path.extension().is_none() {
                        path.set_extension("xlsx");
                    }
                    match s.export_to(&path, &period) {
                        Ok(count) => {
                            s.notice = Some(format!(
                                "Esportate {count} transazioni in {}",
                                path.display()
                            ));
                        }
                        Err(message) => {
                            s.notice = Some(format!("Esportazione non riuscita: {message}"));
                        }
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    s.notice = Some(format!(
                        "Impossibile aprire la finestra di salvataggio: {err}"
                    ));
                }
            });
        })
        .build(k);

    let mut body = col()
        .w_full()
        .gap(12.0)
        .child(
            row()
                .w_full()
                .gap(12.0)
                .items_start()
                .child(field(k, "Da anno", from).grow(1.0))
                .child(field(k, "A anno", to).grow(1.0)),
        )
        .child(field(k, "Mesi", months));
    if let Some(err) = error_line(k, error) {
        body = body.child(err);
    }
    body = body.child(buttons(cancel, confirm));

    Dialog::new("export-dialog", "Esporta in Excel")
        .description("Scegli il periodo e i mesi; lascia vuoti gli anni per tutta la cronologia.")
        .open(true)
        .width(560.0)
        .fit()
        .on_close(|s: &mut App| s.close_dialog())
        .child(body)
        .build(k, layer);
}

fn confirm(k: &Kit, layer: &mut Layer, label: &str) {
    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::Ghost)
        .on_press(|s: &mut App| s.close_dialog())
        .build(k);
    let delete = Button::new("Delete")
        .variant(ButtonVariant::Danger)
        .icon(&icons::TRASH)
        .on_press(|s: &mut App| s.delete_confirmed())
        .build(k);

    let body = col()
        .w_full()
        .gap(14.0)
        .child(
            text(label.to_owned())
                .font(k.ty.ui)
                .color(k.c.text_base)
                .wrap(),
        )
        .child(buttons(cancel, delete));

    Dialog::new("delete-dialog", "Delete this transaction?")
        .description("This cannot be undone.")
        .open(true)
        .width(420.0)
        .fit()
        .on_close(|s: &mut App| s.close_dialog())
        .child(body)
        .build(k, layer);
}

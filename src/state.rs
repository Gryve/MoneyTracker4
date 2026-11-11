//! Application state: what the view reads, and the handlers that change it.
//!
//! The data comes from [`crate::db`] and is refreshed after every mutation, mirroring the Python
//! app's `_refresh_all`. Views are pure functions of this state.

use std::collections::HashMap;

use chrono::{Datelike, Local, NaiveDate};
use rune::TextEdit;
use rune_kit::shell::PaneState;

use crate::db::{self, Db, TransactionChange};
use crate::model::{self, AnnualSummary, Balance, Categories, Kind, MonthlySummary, Transaction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Dashboard,
    Transactions,
    Calendar,
    Budgets,
    Year,
}

impl View {
    pub const ALL: [View; 5] = [
        View::Dashboard,
        View::Transactions,
        View::Calendar,
        View::Budgets,
        View::Year,
    ];

    pub fn label(self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Transactions => "Transactions",
            View::Calendar => "Calendar",
            View::Budgets => "Budgets",
            View::Year => "Year Overview",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            View::Dashboard => "This month at a glance",
            View::Transactions => "Everything you recorded this month",
            View::Calendar => "Day by day",
            View::Budgets => "Annual limits per category",
            View::Year => "Twelve months, one table",
        }
    }

    pub fn icon(self) -> &'static rune::Icon {
        use crate::icons as i;
        match self {
            View::Dashboard => &i::DASHBOARD,
            View::Transactions => &i::TRANSACTIONS,
            View::Calendar => &i::CALENDAR,
            View::Budgets => &i::BUDGETS,
            View::Year => &i::YEAR,
        }
    }
}

/// The header picker whose listbox is open; at most one at a time.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Picker {
    Month,
    Year,
}

/// Which modal is open. The form fields live on [`App`] so handlers can borrow them directly.
#[derive(Clone, Debug)]
pub enum Dialog {
    Transaction {
        edit_id: Option<String>,
        error: Option<String>,
    },
    Categories {
        error: Option<String>,
    },
    ConfirmDelete {
        id: String,
        label: String,
    },
}

pub struct TxForm {
    pub kind: Kind,
    pub amount: TextEdit,
    pub category: Option<String>,
    pub date: TextEdit,
    pub description: TextEdit,
}

impl TxForm {
    fn new() -> TxForm {
        TxForm {
            kind: Kind::Expense,
            amount: TextEdit::new(""),
            category: None,
            date: TextEdit::new(today_iso()),
            description: TextEdit::new(""),
        }
    }
}

pub struct CatsForm {
    pub kind: Kind,
    pub draft: Categories,
    pub new_name: TextEdit,
}

impl CatsForm {
    fn new(categories: &Categories) -> CatsForm {
        let mut draft = categories.clone();
        for kind in [Kind::Expense, Kind::Income] {
            draft.of_mut(kind).sort_by(|a, b| {
                model::display_name(a)
                    .to_lowercase()
                    .cmp(&model::display_name(b).to_lowercase())
            });
        }
        CatsForm {
            kind: Kind::Expense,
            draft,
            new_name: TextEdit::new(""),
        }
    }
}

pub struct App {
    pub db: Db,
    pub view: View,
    /// First day of the month the dashboard, transactions and calendar show.
    pub month: NaiveDate,
    /// The year the budgets and year overview show.
    pub year: i32,
    pub selected_day: Option<NaiveDate>,
    pub sidebar: PaneState,

    // Data
    pub categories: Categories,
    pub months: Vec<(i32, u32)>,
    pub summary: MonthlySummary,
    pub annual: AnnualSummary,
    /// The annual summary of the month's year, which the dashboard's budget card shows even
    /// when the budgets view is looking at another year.
    pub month_annual: AnnualSummary,
    pub month_budget_total: f64,
    /// The twelve months of `year`, in order, for the year overview.
    pub year_months: Vec<(u32, MonthlySummary)>,
    pub budgets: HashMap<String, f64>,
    pub all_time: Balance,
    pub transactions: Vec<Transaction>,
    pub day_transactions: Vec<Transaction>,

    // Currency, editable per year
    pub budget_edits: HashMap<String, TextEdit>,
    /// The inline budget field being edited, so the committed value isn't overwritten while typing.
    pub budget_focus: Option<String>,

    // Modals
    pub dialog: Option<Dialog>,
    /// The header picker whose listbox is open; at most one at a time.
    pub picker_open: Option<Picker>,
    pub tx_form: TxForm,
    pub cats_form: CatsForm,
    pub notice: Option<String>,
}

impl App {
    pub fn new() -> App {
        let db = Db::resolve();
        // `MT_SNAP_MONTH=YYYY-MM` (and `MT_SNAP_DAY=YYYY-MM-DD`) pin the month a headless
        // snapshot shows; the running app always opens on today.
        let snap_month = std::env::var("MT_SNAP_MONTH")
            .ok()
            .and_then(|raw| NaiveDate::parse_from_str(&format!("{raw}-01"), "%Y-%m-%d").ok());
        let open_month = snap_month
            .unwrap_or_else(|| today_iso_date().with_day(1).unwrap_or_else(today_iso_date));
        let open_day = std::env::var("MT_SNAP_DAY")
            .ok()
            .and_then(|raw| NaiveDate::parse_from_str(&raw, "%Y-%m-%d").ok())
            .or(snap_month)
            .unwrap_or_else(today_iso_date);
        let mut app = App::open(db, open_month, open_day);
        app.open_snap_dialog();
        app
    }

    /// A test handle on a database of its own, opened on today's month.
    pub fn with_db(db: Db) -> App {
        let today = today_iso_date();
        App::open(db, today.with_day(1).unwrap_or(today), today)
    }

    fn open(db: Db, month: NaiveDate, selected_day: NaiveDate) -> App {
        let mut app = App {
            view: App::initial_view(),
            db,
            month,
            year: month.year(),
            selected_day: Some(selected_day),
            sidebar: PaneState::new(268.0).range(220.0, 360.0),
            categories: Categories::default(),
            months: Vec::new(),
            summary: MonthlySummary::default(),
            annual: AnnualSummary::default(),
            month_annual: AnnualSummary::default(),
            month_budget_total: 0.0,
            year_months: Vec::new(),
            budgets: HashMap::new(),
            all_time: Balance::default(),
            transactions: Vec::new(),
            day_transactions: Vec::new(),
            budget_edits: HashMap::new(),
            budget_focus: None,
            dialog: None,
            picker_open: None,
            tx_form: TxForm::new(),
            cats_form: CatsForm::new(&Categories::default()),
            notice: None,
        };
        if let Err(err) = app.db.init() {
            app.notice = Some(format!("Database error: {err}"));
        }
        app.reload();
        app
    }

    /// `MT_SNAP_DIALOG=add|categories|edit|delete` opens a modal for a headless snapshot.
    fn open_snap_dialog(&mut self) {
        let Ok(which) = std::env::var("MT_SNAP_DIALOG") else {
            return;
        };
        match which.as_str() {
            "add" => self.open_add(),
            "categories" => self.open_categories(),
            "edit" => {
                if let Some(t) = self.transactions.first().cloned() {
                    self.open_edit(&t);
                }
            }
            "delete" => {
                if let Some(t) = self.transactions.first().cloned() {
                    self.ask_delete(&t);
                }
            }
            _ => {}
        }
    }

    /// `--view dashboard|transactions|calendar|budgets|year` picks the first screen, which the
    /// headless snapshots use.
    fn initial_view() -> View {
        let args = rune::extra_args();
        let mut view = View::Dashboard;
        if let Some(i) = args.iter().position(|a| a == "--view") {
            if let Some(name) = args.get(i + 1) {
                view = match name.as_str() {
                    "transactions" => View::Transactions,
                    "calendar" => View::Calendar,
                    "budgets" => View::Budgets,
                    "year" => View::Year,
                    _ => View::Dashboard,
                };
            }
        }
        view
    }

    pub fn month_number(&self) -> u32 {
        self.month.month()
    }

    pub fn month_year(&self) -> i32 {
        self.month.year()
    }

    pub fn month_label(&self) -> String {
        model::month_name(self.month.year(), self.month.month())
    }

    /// Re-reads everything the views show.
    pub fn reload(&mut self) {
        match self.load() {
            Ok(()) => self.notice = None,
            Err(err) => {
                self.notice = Some(format!("Could not read the database: {err}"));
            }
        }
    }

    fn load(&mut self) -> db::DbResult<()> {
        self.categories = self.db.load_categories()?;
        self.months = self.db.get_months_with_data()?;
        self.summary = self
            .db
            .get_monthly_summary(self.month.year(), self.month.month())?;
        self.annual = self.db.get_annual_summary(self.year)?;
        let year_months = (1..=12)
            .map(|month| {
                self.db
                    .get_monthly_summary(self.year, month)
                    .map(|summary| (month, summary))
            })
            .collect::<db::DbResult<Vec<_>>>()?;
        self.year_months = year_months;
        self.budgets = self.db.load_budgets_by_display(self.year)?;
        self.all_time = self.db.get_all_time_balance()?;
        // The dashboard's budget card follows the month, not the budgets view's year.
        let month_year = self.month.year();
        if month_year == self.year {
            self.month_annual = self.annual.clone();
            self.month_budget_total = self.total_budget();
        } else {
            self.month_annual = self.db.get_annual_summary(month_year)?;
            let budgets = self.db.load_budgets_by_display(month_year)?;
            self.month_budget_total = budgets.values().copied().filter(|v| *v > 0.0).sum();
        }
        self.transactions = self
            .db
            .get_transactions_by_month(self.month.year(), self.month.month())?;
        self.day_transactions = match self.selected_day {
            Some(day) => self
                .db
                .get_transactions_by_date(&day.format("%Y-%m-%d").to_string())?,
            None => Vec::new(),
        };

        // Seed the inline budget fields for categories that have no field yet; keep whatever is
        // already typed in the others.
        for display in model::display_groups(&self.categories.expense) {
            let value = self.budgets.get(&display).copied().unwrap_or(0.0);
            self.budget_edits.entry(display).or_insert_with(|| {
                TextEdit::new(if value > 0.0 {
                    format!("{value:.2}")
                } else {
                    String::new()
                })
            });
        }
        Ok(())
    }

    // ── Navigation ──────────────────────────────────────────────────────────

    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    /// Jumps the month-scoped views to `year`-`month`; a no-op when it is already shown.
    pub fn set_month(&mut self, year: i32, month: u32) {
        self.picker_open = None;
        if (year, month) == (self.month.year(), self.month.month()) {
            return;
        }
        let first = NaiveDate::from_ymd_opt(year, month.clamp(1, 12), 1).unwrap_or(self.month);
        self.month = first;
        self.selected_day = Some(first);
        self.reload();
    }

    /// The years the pickers offer: every year with data plus the current one, newest first.
    pub fn year_options(&self) -> Vec<i32> {
        let mut years: Vec<i32> = self.months.iter().map(|(year, _)| *year).collect();
        years.push(self.year);
        years.push(self.month.year());
        years.sort_unstable_by(|a, b| b.cmp(a));
        years.dedup();
        years
    }

    /// Opens or closes the header picker's listbox.
    pub fn toggle_picker(&mut self, picker: Picker) {
        self.picker_open = if self.picker_open == Some(picker) { None } else { Some(picker) };
    }

    /// Closes any open header picker (Escape or a press outside the listbox).
    pub fn close_picker(&mut self) {
        self.picker_open = None;
    }

    pub fn go_today(&mut self) {
        let today = today_iso_date();
        self.month = today.with_day(1).unwrap_or(today);
        self.selected_day = Some(today);
        self.year = today.year();
        self.reload();
    }

    /// Jumps the budgets and year overview to `year`; a no-op when it is already shown.
    pub fn set_year(&mut self, year: i32) {
        self.picker_open = None;
        if year == self.year {
            return;
        }
        self.year = year;
        self.reload();
    }

    pub fn select_day(&mut self, day: NaiveDate) {
        self.selected_day = Some(day);
        self.day_transactions = self
            .db
            .get_transactions_by_date(&day.format("%Y-%m-%d").to_string())
            .unwrap_or_default();
    }

    // ── Transactions ────────────────────────────────────────────────────────

    pub fn open_add(&mut self) {
        self.tx_form = TxForm::new();
        self.open_add_of_kind(Kind::Expense);
    }

    pub fn open_add_of_kind(&mut self, kind: Kind) {
        self.tx_form = TxForm::new();
        self.tx_form.kind = kind;
        self.tx_form.category = model::display_groups(self.categories.of(kind))
            .first()
            .cloned();
        if let Some(day) = self.selected_day {
            self.tx_form.date = TextEdit::new(day.format("%Y-%m-%d").to_string());
        }
        self.dialog = Some(Dialog::Transaction {
            edit_id: None,
            error: None,
        });
    }

    /// Switches the open form between expense and income, keeping a category of the new kind.
    pub fn set_form_kind(&mut self, kind: Kind) {
        self.tx_form.kind = kind;
        let valid = self.tx_form.category.as_deref().is_some_and(|c| {
            self.categories
                .of(kind)
                .iter()
                .any(|n| model::display_name(n) == c)
        });
        if !valid {
            self.tx_form.category = self
                .categories
                .of(kind)
                .first()
                .map(|n| model::display_name(n));
        }
    }

    /// Switches the category manager between the expense and the income list.
    pub fn set_cats_kind(&mut self, kind: Kind) {
        self.cats_form.kind = kind;
    }

    pub fn open_edit(&mut self, transaction: &Transaction) {
        self.tx_form = TxForm {
            kind: transaction.kind,
            amount: TextEdit::new(format!("{:.2}", transaction.amount)),
            category: Some(model::display_name(&transaction.category)),
            date: TextEdit::new(transaction.date.clone()),
            description: TextEdit::new(transaction.description.clone()),
        };
        self.dialog = Some(Dialog::Transaction {
            edit_id: Some(transaction.id.clone()),
            error: None,
        });
    }

    pub fn close_dialog(&mut self) {
        self.dialog = None;
    }

    pub fn submit_transaction(&mut self) {
        let Some(Dialog::Transaction { edit_id, .. }) = self.dialog.clone() else {
            return;
        };

        let amount_text = self.tx_form.amount.text.replace(',', ".");
        let amount = match amount_text.trim().parse::<f64>() {
            Ok(v) if v > 0.0 && v.is_finite() => db::round2(v),
            _ => {
                self.set_transaction_error("Enter an amount greater than zero");
                return;
            }
        };
        let Some(display_category) = self.tx_form.category.clone() else {
            self.set_transaction_error("Choose a category");
            return;
        };
        let stored = self
            .stored_category(self.tx_form.kind, &display_category)
            .unwrap_or(display_category);

        let date_text = self.tx_form.date.text.trim().to_owned();
        let date = if date_text.is_empty() {
            today_iso()
        } else if NaiveDate::parse_from_str(&date_text, "%Y-%m-%d").is_ok() {
            date_text
        } else {
            self.set_transaction_error("Use YYYY-MM-DD for the date");
            return;
        };

        let description = self.tx_form.description.text.clone();
        let result = match edit_id {
            Some(id) => self
                .db
                .update_transaction(
                    &id,
                    TransactionChange {
                        kind: Some(self.tx_form.kind),
                        amount: Some(amount),
                        category: Some(stored),
                        description: Some(description),
                        date: Some(date),
                    },
                )
                .map(|_| ()),
            None => self
                .db
                .add_transaction(
                    self.tx_form.kind,
                    amount,
                    &stored,
                    &description,
                    Some(&date),
                )
                .map(|_| ()),
        };
        match result {
            Ok(()) => {
                self.dialog = None;
                self.reload();
            }
            Err(err) => self.set_transaction_error(&format!("Could not save: {err}")),
        }
    }

    fn set_transaction_error(&mut self, message: &str) {
        if let Some(Dialog::Transaction { error, .. }) = &mut self.dialog {
            *error = Some(message.to_owned());
        }
    }

    /// The stored spelling of a displayed category: the one that already has transactions that
    /// most recently used it, else the first stored spelling, else the display name itself.
    fn stored_category(&self, kind: Kind, display: &str) -> Option<String> {
        let names = model::stored_names(self.categories.of(kind), display);
        names
            .iter()
            .find(|n| {
                self.transactions
                    .iter()
                    .chain(&self.day_transactions)
                    .any(|t| &t.category == *n)
            })
            .or_else(|| names.first())
            .cloned()
    }

    pub fn ask_delete(&mut self, transaction: &Transaction) {
        let label = format!(
            "{} · {}",
            model::display_name(&transaction.category),
            model::euros(transaction.amount)
        );
        self.dialog = Some(Dialog::ConfirmDelete {
            id: transaction.id.clone(),
            label,
        });
    }

    pub fn delete_confirmed(&mut self) {
        let Some(Dialog::ConfirmDelete { id, .. }) = self.dialog.clone() else {
            return;
        };
        if let Err(err) = self.db.delete_transaction(&id) {
            self.notice = Some(format!("Could not delete: {err}"));
        }
        self.dialog = None;
        self.reload();
    }

    // ── Categories ──────────────────────────────────────────────────────────

    pub fn open_categories(&mut self) {
        self.cats_form = CatsForm::new(&self.categories);
        self.dialog = Some(Dialog::Categories { error: None });
    }

    pub fn add_category(&mut self) {
        let name = self.cats_form.new_name.text.trim().to_owned();
        if name.is_empty() {
            self.set_categories_error("Type a category name");
            return;
        }
        let kind = self.cats_form.kind;
        let list = self.cats_form.draft.of_mut(kind);
        if list.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
            self.set_categories_error("That category already exists");
            return;
        }
        list.push(name);
        self.cats_form.of_sort(kind);
        self.cats_form.new_name = TextEdit::new("");
        self.set_categories_error("");
    }

    pub fn remove_category(&mut self, kind: Kind, name: &str) {
        self.cats_form.draft.of_mut(kind).retain(|c| c != name);
    }

    fn set_categories_error(&mut self, message: &str) {
        if let Some(Dialog::Categories { error }) = &mut self.dialog {
            *error = if message.is_empty() {
                None
            } else {
                Some(message.to_owned())
            };
        }
    }

    pub fn save_categories(&mut self) {
        match self.db.save_categories(&self.cats_form.draft) {
            Ok(()) => {
                self.dialog = None;
                self.reload();
            }
            Err(err) => self.set_categories_error(&format!("Could not save: {err}")),
        }
    }

    // ── Budgets ─────────────────────────────────────────────────────────────

    /// Stores the typed value of one inline budget field.
    pub fn commit_budget(&mut self, display: &str) {
        let Some(edit) = self.budget_edits.get(display) else {
            return;
        };
        let raw = edit.text.replace(',', ".").trim().to_owned();
        let amount = if raw.is_empty() {
            0.0
        } else {
            match raw.parse::<f64>() {
                Ok(v) if v.is_finite() && v >= 0.0 => db::round2(v),
                _ => {
                    self.notice = Some(format!("“{raw}” is not a valid budget"));
                    return;
                }
            }
        };
        self.budgets.insert(display.to_owned(), amount);
        let budgets = self.budgets.clone();
        if let Err(err) = self
            .db
            .save_budgets_by_display(self.year, &self.categories, &budgets)
        {
            self.notice = Some(format!("Could not save the budget: {err}"));
        }
        self.budget_focus = None;
        self.reload();
    }

    /// Spent this year per display name, for the budget rows.
    pub fn annual_spend_by_display(&self) -> HashMap<String, f64> {
        model::merge_by_display(&self.annual.category_expenses)
    }

    pub fn total_budget(&self) -> f64 {
        self.budgets.values().copied().filter(|v| *v > 0.0).sum()
    }
}

impl CatsForm {
    fn of_sort(&mut self, kind: Kind) {
        self.draft.of_mut(kind).sort_by(|a, b| {
            model::display_name(a)
                .to_lowercase()
                .cmp(&model::display_name(b).to_lowercase())
        });
    }
}

pub fn today_iso_date() -> NaiveDate {
    Local::now().date_naive()
}

pub fn today_iso() -> String {
    today_iso_date().format("%Y-%m-%d").to_string()
}

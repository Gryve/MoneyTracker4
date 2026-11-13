//! Application state: what the view reads, and the handlers that change it.
//!
//! The data comes from [`crate::db`] and is refreshed after every mutation, mirroring the Python
//! app's `_refresh_all`. Views are pure functions of this state.

use std::collections::HashMap;
use std::path::Path;

use chrono::{Datelike, Local, NaiveDate};
use rune::TextEdit;
use rune_kit::shell::PaneState;

use crate::db::{self, Db, TransactionChange};
use crate::export::{self, Period};
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
    DashMonths,
    DashYears,
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
    Export {
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

/// The Excel export dialog's period fields: empty years mean "all years", every month starts
/// selected.
pub struct ExportForm {
    pub from_year: TextEdit,
    pub to_year: TextEdit,
    /// One entry per month, January first.
    pub months: [bool; 12],
}

impl ExportForm {
    fn new() -> ExportForm {
        ExportForm {
            from_year: TextEdit::new(""),
            to_year: TextEdit::new(""),
            months: [true; 12],
        }
    }

    pub fn month_selected(&self, month: u32) -> bool {
        self.months[month.clamp(1, 12) as usize - 1]
    }

    /// Flips one month, 1-based.
    pub fn toggle_month(&mut self, month: u32) {
        let index = month.clamp(1, 12) as usize - 1;
        self.months[index] = !self.months[index];
    }

    pub fn all_months(&self) -> bool {
        self.months.iter().all(|selected| *selected)
    }

    pub fn no_months(&self) -> bool {
        self.months.iter().all(|selected| !*selected)
    }

    /// The "Tutti" checkbox: selects every month unless they all already are.
    pub fn toggle_all_months(&mut self) {
        self.months = [!self.all_months(); 12];
    }

    pub fn selected_months(&self) -> export::MonthSet {
        export::MonthSet::from_months((1..=12).filter(|month| self.month_selected(*month)))
    }
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

#[derive(Clone, Debug, PartialEq)]
pub enum TimeBucket {
    Day(NaiveDate),
    Month(i32, u32),
}

impl TimeBucket {
    pub fn label(&self) -> String {
        match self {
            TimeBucket::Day(d) => format!("{:02}", d.day()),
            TimeBucket::Month(y, m) => {
                let name = model::month_only(*m);
                let short_name = if name.len() >= 3 { &name[..3] } else { name };
                format!("{} '{:02}", short_name, y % 100)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DashboardFilter {
    pub is_all: bool,
    pub years: Vec<i32>,
    pub months: [bool; 12],
    pub visible_expense_categories: HashMap<String, bool>,
    pub visible_income_categories: HashMap<String, bool>,
}

impl DashboardFilter {
    pub fn new(current_year: i32, current_month: u32) -> Self {
        let mut months = [false; 12];
        if (1..=12).contains(&current_month) {
            months[current_month as usize - 1] = true;
        }
        Self {
            is_all: false,
            years: vec![current_year],
            months,
            visible_expense_categories: HashMap::new(),
            visible_income_categories: HashMap::new(),
        }
    }

    pub fn month_selected(&self, month: u32) -> bool {
        if self.is_all {
            return true;
        }
        self.months[month.clamp(1, 12) as usize - 1]
    }

    pub fn year_selected(&self, year: i32) -> bool {
        if self.is_all {
            return true;
        }
        self.years.contains(&year)
    }

    pub fn selected_months_count(&self) -> usize {
        if self.is_all {
            12
        } else {
            self.months.iter().filter(|&&m| m).count()
        }
    }

    pub fn all_months_selected(&self) -> bool {
        self.months.iter().all(|&m| m)
    }

    pub fn no_months_selected(&self) -> bool {
        self.months.iter().all(|&m| !m)
    }

    pub fn is_category_visible(&self, kind: Kind, category: &str) -> bool {
        let map = match kind {
            Kind::Expense => &self.visible_expense_categories,
            Kind::Income => &self.visible_income_categories,
        };
        map.get(category).copied().unwrap_or(true)
    }

    pub fn toggle_category(&mut self, kind: Kind, category: &str) {
        let map = match kind {
            Kind::Expense => &mut self.visible_expense_categories,
            Kind::Income => &mut self.visible_income_categories,
        };
        let current = map.get(category).copied().unwrap_or(true);
        map.insert(category.to_string(), !current);
    }

    pub fn set_all_categories(&mut self, kind: Kind, categories: &[String], visible: bool) {
        let map = match kind {
            Kind::Expense => &mut self.visible_expense_categories,
            Kind::Income => &mut self.visible_income_categories,
        };
        for cat in categories {
            map.insert(cat.clone(), visible);
        }
    }

    pub fn matches(&self, t: &Transaction) -> bool {
        if self.is_all {
            return true;
        }
        let parts: Vec<&str> = t.date.split('-').collect();
        if parts.len() < 2 {
            return false;
        }
        let Ok(year) = parts[0].parse::<i32>() else { return false };
        let Ok(month) = parts[1].parse::<u32>() else { return false };
        if !self.years.contains(&year) {
            return false;
        }
        if !self.month_selected(month) {
            return false;
        }
        true
    }

    pub fn time_buckets(&self, transactions: &[Transaction]) -> (Vec<TimeBucket>, bool) {
        if !self.is_all && self.years.len() == 1 && self.selected_months_count() == 1 {
            let year = self.years[0];
            let month = (1..=12).find(|&m| self.month_selected(m)).unwrap_or(1);
            let days = db::month_days(year, month);
            let buckets = days.into_iter().map(TimeBucket::Day).collect();
            (buckets, true)
        } else {
            let mut pairs = Vec::new();
            if self.is_all {
                for t in transactions {
                    let parts: Vec<&str> = t.date.split('-').collect();
                    if parts.len() >= 2 {
                        if let (Ok(y), Ok(m)) = (parts[0].parse::<i32>(), parts[1].parse::<u32>()) {
                            pairs.push((y, m));
                        }
                    }
                }
                pairs.sort_unstable();
                pairs.dedup();
                if pairs.is_empty() {
                    let today = Local::now().date_naive();
                    pairs.push((today.year(), today.month()));
                }
            } else {
                let mut sorted_years = self.years.clone();
                sorted_years.sort_unstable();
                for &y in &sorted_years {
                    for m in 1..=12 {
                        if self.month_selected(m) {
                            pairs.push((y, m));
                        }
                    }
                }
            }
            let buckets = pairs.into_iter().map(|(y, m)| TimeBucket::Month(y, m)).collect();
            (buckets, false)
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
    pub all_transactions: Vec<Transaction>,
    pub dash_filter: DashboardFilter,
    pub dash_transactions: Vec<Transaction>,
    pub dash_summary: MonthlySummary,

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
    pub export_form: ExportForm,
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
        let dash_filter = DashboardFilter::new(month.year(), month.month());
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
            all_transactions: Vec::new(),
            dash_filter,
            dash_transactions: Vec::new(),
            dash_summary: MonthlySummary::default(),
            budget_edits: HashMap::new(),
            budget_focus: None,
            dialog: None,
            picker_open: None,
            tx_form: TxForm::new(),
            cats_form: CatsForm::new(&Categories::default()),
            export_form: ExportForm::new(),
            notice: None,
        };
        if let Err(err) = app.db.init() {
            app.notice = Some(format!("Database error: {err}"));
        }
        app.reload();
        app
    }

    /// `MT_SNAP_DIALOG=add|categories|edit|delete|export` opens a modal for a headless snapshot.
    fn open_snap_dialog(&mut self) {
        let Ok(which) = std::env::var("MT_SNAP_DIALOG") else {
            return;
        };
        match which.as_str() {
            "add" => self.open_add(),
            "categories" => self.open_categories(),
            "export" => self.open_export(),
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
        self.all_transactions = self.db.load_transactions()?;
        self.update_dashboard_data();
        Ok(())
    }

    pub fn update_dashboard_data(&mut self) {
        self.dash_transactions = self
            .all_transactions
            .iter()
            .filter(|t| self.dash_filter.matches(t))
            .cloned()
            .collect();
        self.dash_summary = App::summarize_transactions(&self.dash_transactions);
    }

    pub fn summarize_transactions(transactions: &[Transaction]) -> MonthlySummary {
        let mut summary = MonthlySummary {
            transaction_count: transactions.len(),
            ..Default::default()
        };
        for t in transactions {
            match t.kind {
                Kind::Income => summary.total_income += t.amount,
                Kind::Expense => summary.total_expense += t.amount,
            }
            let day = summary.daily.entry(t.date.clone()).or_insert((0.0, 0.0));
            match t.kind {
                Kind::Income => day.0 += t.amount,
                Kind::Expense => day.1 += t.amount,
            }
            match t.kind {
                Kind::Expense => {
                    *summary
                        .category_expenses
                        .entry(t.category.clone())
                        .or_insert(0.0) += t.amount
                }
                Kind::Income => {
                    *summary
                        .category_income
                        .entry(t.category.clone())
                        .or_insert(0.0) += t.amount
                }
            }
        }
        summary.total_income = db::round2(summary.total_income);
        summary.total_expense = db::round2(summary.total_expense);
        summary.balance = db::round2(summary.total_income - summary.total_expense);
        summary
    }

    pub fn dash_filter_label(&self) -> String {
        if self.dash_filter.is_all {
            return "All time".to_owned();
        }
        let years = &self.dash_filter.years;
        let months_count = self.dash_filter.selected_months_count();
        if years.is_empty() || months_count == 0 {
            return "No period selected".to_owned();
        }
        if years.len() == 1 {
            let year = years[0];
            if months_count == 1 {
                let m = (1..=12).find(|&m| self.dash_filter.month_selected(m)).unwrap_or(1);
                return model::month_name(year, m);
            } else if months_count == 12 {
                return format!("Year {year}");
            } else {
                return format!("{months_count} months · {year}");
            }
        } else {
            let mut sorted_years = years.clone();
            sorted_years.sort_unstable();
            let years_str = sorted_years.iter().map(|y| y.to_string()).collect::<Vec<_>>().join(", ");
            if months_count == 12 {
                return format!("Years {years_str}");
            } else if months_count == 1 {
                let m = (1..=12).find(|&m| self.dash_filter.month_selected(m)).unwrap_or(1);
                let m_name = model::month_only(m);
                return format!("{m_name} · {years_str}");
            } else {
                return format!("{months_count} months · {years_str}");
            }
        }
    }

    pub fn dash_today(&mut self) {
        self.picker_open = None;
        let today = today_iso_date();
        self.month = today.with_day(1).unwrap_or(today);
        self.selected_day = Some(today);
        self.year = today.year();
        self.dash_filter.is_all = false;
        self.dash_filter.years = vec![today.year()];
        self.dash_filter.months = [false; 12];
        if (1..=12).contains(&today.month()) {
            self.dash_filter.months[today.month() as usize - 1] = true;
        }
        self.reload();
    }

    pub fn dash_all(&mut self) {
        self.picker_open = None;
        self.dash_filter.is_all = true;
        self.dash_filter.years = self.year_options();
        self.dash_filter.months = [true; 12];
        self.update_dashboard_data();
    }

    pub fn toggle_dash_month(&mut self, month: u32) {
        self.dash_filter.is_all = false;
        let idx = month.clamp(1, 12) as usize - 1;
        self.dash_filter.months[idx] = !self.dash_filter.months[idx];
        self.update_dashboard_data();
    }

    pub fn dash_select_all_months(&mut self) {
        self.dash_filter.is_all = false;
        self.dash_filter.months = [true; 12];
        self.update_dashboard_data();
    }

    pub fn dash_deselect_all_months(&mut self) {
        self.dash_filter.is_all = false;
        self.dash_filter.months = [false; 12];
        self.update_dashboard_data();
    }

    pub fn toggle_dash_year(&mut self, year: i32) {
        self.dash_filter.is_all = false;
        if let Some(pos) = self.dash_filter.years.iter().position(|&y| y == year) {
            self.dash_filter.years.remove(pos);
        } else {
            self.dash_filter.years.push(year);
        }
        self.update_dashboard_data();
    }

    pub fn dash_select_all_years(&mut self) {
        self.dash_filter.is_all = false;
        self.dash_filter.years = self.year_options();
        self.update_dashboard_data();
    }

    pub fn dash_deselect_all_years(&mut self) {
        self.dash_filter.is_all = false;
        self.dash_filter.years.clear();
        self.update_dashboard_data();
    }

    pub fn toggle_dash_category(&mut self, kind: Kind, category: &str) {
        self.dash_filter.toggle_category(kind, category);
    }

    pub fn dash_select_all_categories(&mut self, kind: Kind) {
        let cats: Vec<String> = model::display_groups(self.categories.of(kind));
        self.dash_filter.set_all_categories(kind, &cats, true);
    }

    pub fn dash_deselect_all_categories(&mut self, kind: Kind) {
        let cats: Vec<String> = model::display_groups(self.categories.of(kind));
        self.dash_filter.set_all_categories(kind, &cats, false);
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
        self.picker_open = if self.picker_open == Some(picker) {
            None
        } else {
            Some(picker)
        };
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

    // ── Excel export ────────────────────────────────────────────────────────

    /// The default name the save dialog proposes.
    pub fn default_export_name() -> String {
        format!("MoneyTracker-{}.xlsx", today_iso_date().format("%Y-%m-%d"))
    }

    pub fn open_export(&mut self) {
        self.export_form = ExportForm::new();
        self.notice = None;
        self.dialog = Some(Dialog::Export { error: None });
    }

    pub fn set_export_error(&mut self, message: &str) {
        if let Some(Dialog::Export { error }) = &mut self.dialog {
            *error = Some(message.to_owned());
        }
    }

    /// Reads the dialog's fields into a [`Period`]; empty years mean "everything".
    pub fn export_period(&self) -> Result<Period, String> {
        fn year(text: &str, label: &str) -> Result<Option<i32>, String> {
            let raw = text.trim();
            if raw.is_empty() {
                return Ok(None);
            }
            match raw.parse::<i32>() {
                Ok(value) if (1900..=2200).contains(&value) => Ok(Some(value)),
                _ => Err(format!("{label}: scrivi un anno tra 1900 e 2200")),
            }
        }

        let from_year = year(&self.export_form.from_year.text, "Da anno")?;
        let to_year = year(&self.export_form.to_year.text, "A anno")?;
        if let (Some(from), Some(to)) = (from_year, to_year) {
            if from > to {
                return Err("Da anno è successivo ad A anno".to_owned());
            }
        }

        let months = self.export_form.selected_months();
        if months.is_empty() {
            return Err("Mese: seleziona almeno un mese".to_owned());
        }

        Ok(Period {
            from_year,
            to_year,
            months,
        })
    }

    /// Writes the workbook to `path`; returns how many transactions it contains.
    pub fn export_to(&self, path: &Path, period: &Period) -> Result<usize, String> {
        let transactions = self
            .db
            .load_transactions()
            .map_err(|err| format!("lettura del database: {err}"))?;
        let categories = self
            .db
            .load_categories()
            .map_err(|err| format!("lettura delle categorie: {err}"))?;
        let bytes = export::build_workbook(&transactions, &categories, period)
            .map_err(|err| format!("creazione del file: {err}"))?;
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .map_err(|err| format!("creazione della cartella {}: {err}", dir.display()))?;
        }
        std::fs::write(path, bytes)
            .map_err(|err| format!("scrittura di {}: {err}", path.display()))?;
        Ok(transactions.len())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_filter_defaults_to_single_month() {
        let filter = DashboardFilter::new(2026, 11);
        assert!(!filter.is_all);
        assert_eq!(filter.years, vec![2026]);
        assert_eq!(filter.selected_months_count(), 1);
        assert!(filter.month_selected(11));
        assert!(!filter.month_selected(10));

        let t_nov = Transaction {
            id: "1".into(),
            kind: Kind::Expense,
            amount: 50.0,
            category: "Food".into(),
            description: "".into(),
            date: "2026-11-15".into(),
            created_at: "".into(),
        };
        let t_oct = Transaction {
            id: "2".into(),
            kind: Kind::Expense,
            amount: 30.0,
            category: "Food".into(),
            description: "".into(),
            date: "2026-10-15".into(),
            created_at: "".into(),
        };
        assert!(filter.matches(&t_nov));
        assert!(!filter.matches(&t_oct));
    }

    #[test]
    fn dashboard_filter_multi_selection() {
        let mut filter = DashboardFilter::new(2026, 11);
        filter.months[9] = true; // October (index 9)
        filter.years.push(2025);

        assert_eq!(filter.selected_months_count(), 2);
        assert!(filter.year_selected(2025));
        assert!(filter.year_selected(2026));

        let t1 = Transaction {
            id: "1".into(),
            kind: Kind::Expense,
            amount: 10.0,
            category: "Food".into(),
            description: "".into(),
            date: "2025-10-01".into(),
            created_at: "".into(),
        };
        let t2 = Transaction {
            id: "2".into(),
            kind: Kind::Expense,
            amount: 20.0,
            category: "Food".into(),
            description: "".into(),
            date: "2026-11-20".into(),
            created_at: "".into(),
        };
        let t3 = Transaction {
            id: "3".into(),
            kind: Kind::Expense,
            amount: 30.0,
            category: "Food".into(),
            description: "".into(),
            date: "2024-10-01".into(),
            created_at: "".into(),
        };
        assert!(filter.matches(&t1));
        assert!(filter.matches(&t2));
        assert!(!filter.matches(&t3));
    }

    #[test]
    fn dashboard_filter_all_matches_everything() {
        let mut filter = DashboardFilter::new(2026, 11);
        filter.is_all = true;
        let t_old = Transaction {
            id: "1".into(),
            kind: Kind::Income,
            amount: 1000.0,
            category: "Salary".into(),
            description: "".into(),
            date: "2020-01-01".into(),
            created_at: "".into(),
        };
        assert!(filter.matches(&t_old));
    }

    #[test]
    fn dashboard_filter_category_visibility() {
        let mut filter = DashboardFilter::new(2026, 11);
        assert!(filter.is_category_visible(Kind::Expense, "Food & Dining"));
        filter.toggle_category(Kind::Expense, "Food & Dining");
        assert!(!filter.is_category_visible(Kind::Expense, "Food & Dining"));
        filter.toggle_category(Kind::Expense, "Food & Dining");
        assert!(filter.is_category_visible(Kind::Expense, "Food & Dining"));
    }

    #[test]
    fn dashboard_time_buckets_daily_and_monthly() {
        let filter_single = DashboardFilter::new(2026, 11);
        let (buckets_day, is_daily) = filter_single.time_buckets(&[]);
        assert!(is_daily);
        assert_eq!(buckets_day.len(), 30); // November has 30 days

        let mut filter_multi = DashboardFilter::new(2026, 11);
        filter_multi.months[9] = true; // October
        let (buckets_month, is_daily_m) = filter_multi.time_buckets(&[]);
        assert!(!is_daily_m);
        assert_eq!(buckets_month.len(), 2);
    }
}

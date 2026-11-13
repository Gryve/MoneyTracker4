//! Excel export: charts, a filterable transactions table, and hidden aggregates.
//!
//! The workbook has three sheets, in this order:
//!
//! - `Grafici` — six charts with Italian titles, the first thing the user sees;
//! - `Transazioni` — a real Excel Table with autofilter, frozen header and a `SUBTOTAL` row;
//! - `Dati` — the pre-computed aggregates the charts read, hidden.
//!
//! Aggregate values are written both as formulas (so Excel recalculates them) and as cached
//! results (so charts render immediately in other readers, which don't calculate on open).

use std::collections::BTreeMap;

use rust_xlsxwriter::{
    Chart, ChartEmptyCells, ChartLegendPosition, ChartType, Format, Formula, Table, TableColumn,
    TableFunction, TableStyle, Workbook, Worksheet, XlsxError,
};

use crate::db::round2;
use crate::model::{self, Categories, Kind, Transaction};

pub const SHEET_CHARTS: &str = "Grafici";
pub const SHEET_TRANSACTIONS: &str = "Transazioni";
pub const SHEET_DATA: &str = "Dati";

const MONTHS: [&str; 12] = [
    "Gennaio",
    "Febbraio",
    "Marzo",
    "Aprile",
    "Maggio",
    "Giugno",
    "Luglio",
    "Agosto",
    "Settembre",
    "Ottobre",
    "Novembre",
    "Dicembre",
];

const MONTHS_SHORT: [&str; 12] = [
    "Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic",
];

/// Palette for the line charts: enough distinct colours for the seeded categories.
const SERIES_COLORS: [&str; 16] = [
    "#2563EB", "#DC2626", "#16A34A", "#D97706", "#7C3AED", "#0891B2", "#DB2777", "#65A30D",
    "#EA580C", "#4F46E5", "#0D9488", "#B91C1C", "#9333EA", "#CA8A04", "#0284C7", "#BE185D",
];

pub fn month_name(month: u32) -> &'static str {
    MONTHS[(month.clamp(1, 12) - 1) as usize]
}

pub fn short_month_label(year: i32, month: u32) -> String {
    format!("{} {year}", MONTHS_SHORT[(month.clamp(1, 12) - 1) as usize])
}

/// The export period: a year range and a set of months, everything by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Period {
    pub from_year: Option<i32>,
    pub to_year: Option<i32>,
    pub months: MonthSet,
}

impl Period {
    pub fn includes(&self, year: i32, month: u32) -> bool {
        self.includes_year(year) && self.months.contains(month)
    }

    /// Whether the year range covers `year`; the month selection is ignored.
    pub fn includes_year(&self, year: i32) -> bool {
        self.from_year.is_none_or(|from| year >= from) && self.to_year.is_none_or(|to| year <= to)
    }

    pub fn label(&self) -> String {
        let years = match (self.from_year, self.to_year) {
            (None, None) => "Tutti gli anni".to_owned(),
            (Some(from), Some(to)) if from == to => format!("{from}"),
            (Some(from), None) => format!("Dal {from}"),
            (None, Some(to)) => format!("Fino al {to}"),
            (Some(from), Some(to)) => format!("{from}–{to}"),
        };
        format!("{years} · {}", self.months.label())
    }
}

/// A set of calendar months, one bit each: bit 0 is January. Defaults to every month.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthSet(u16);

impl Default for MonthSet {
    fn default() -> MonthSet {
        MonthSet::ALL
    }
}

impl MonthSet {
    pub const ALL: MonthSet = MonthSet(0x0FFF);
    pub const NONE: MonthSet = MonthSet(0);

    /// A set holding only `month` (1-based; out-of-range values clamp like [`month_name`]).
    pub fn single(month: u32) -> MonthSet {
        MonthSet(1 << (month.clamp(1, 12) - 1))
    }

    pub fn from_months(months: impl IntoIterator<Item = u32>) -> MonthSet {
        months.into_iter().fold(MonthSet::NONE, |set, month| {
            MonthSet(set.0 | MonthSet::single(month).0)
        })
    }

    pub fn contains(self, month: u32) -> bool {
        self.0 & MonthSet::single(month).0 != 0
    }

    pub fn is_all(self) -> bool {
        self == MonthSet::ALL
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub fn iter(self) -> impl Iterator<Item = u32> {
        (1..=12).filter(move |month| self.contains(*month))
    }

    /// `Tutti i mesi`, or the selected names, comma-separated.
    pub fn label(self) -> String {
        if self.is_all() {
            return "Tutti i mesi".to_owned();
        }
        self.iter().map(month_name).collect::<Vec<_>>().join(", ")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MonthTotals {
    pub year: i32,
    pub month: u32,
    pub income: f64,
    pub expense: f64,
}

impl MonthTotals {
    pub fn balance(&self) -> f64 {
        round2(self.income - self.expense)
    }

    pub fn label(&self) -> String {
        if self.month == 0 {
            format!("{}", self.year)
        } else {
            short_month_label(self.year, self.month)
        }
    }
}

/// Everything the workbook needs, computed once from the transactions.
pub struct Aggregates {
    /// All months with data, oldest first.
    pub months: Vec<MonthTotals>,
    /// All years with data, oldest first (`month` is 0).
    pub years: Vec<MonthTotals>,
    /// Display category names, biggest total first.
    pub expense_categories: Vec<String>,
    pub income_categories: Vec<String>,
    /// `months × expense_categories`, whole history.
    pub expense_matrix: Vec<Vec<f64>>,
    /// `months × income_categories`, whole history.
    pub income_matrix: Vec<Vec<f64>>,
}

pub fn parse_month(date: &str) -> Option<(i32, u32)> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    (1..=12).contains(&month).then_some((year, month))
}

pub fn aggregate(transactions: &[Transaction], categories: &Categories) -> Aggregates {
    let mut monthly: BTreeMap<(i32, u32), (f64, f64)> = BTreeMap::new();
    let mut yearly: BTreeMap<i32, (f64, f64)> = BTreeMap::new();
    let mut expense_totals: BTreeMap<String, f64> = BTreeMap::new();
    let mut income_totals: BTreeMap<String, f64> = BTreeMap::new();

    for transaction in transactions {
        let Some((year, month)) = parse_month(&transaction.date) else {
            continue;
        };
        let display = model::display_name(&transaction.category);
        let entry = monthly.entry((year, month)).or_insert((0.0, 0.0));
        let year_entry = yearly.entry(year).or_insert((0.0, 0.0));
        match transaction.kind {
            Kind::Income => {
                entry.0 += transaction.amount;
                year_entry.0 += transaction.amount;
                *income_totals.entry(display).or_insert(0.0) += transaction.amount;
            }
            Kind::Expense => {
                entry.1 += transaction.amount;
                year_entry.1 += transaction.amount;
                *expense_totals.entry(display).or_insert(0.0) += transaction.amount;
            }
        }
    }

    // Every category the user has, plus any name only seen on an old transaction.
    for name in model::display_groups(&categories.expense) {
        expense_totals.entry(name).or_insert(0.0);
    }
    for name in model::display_groups(&categories.income) {
        income_totals.entry(name).or_insert(0.0);
    }

    let expense_categories = sorted_categories(expense_totals);
    let income_categories = sorted_categories(income_totals);

    let months: Vec<MonthTotals> = monthly
        .into_iter()
        .map(|((year, month), (income, expense))| MonthTotals {
            year,
            month,
            income: round2(income),
            expense: round2(expense),
        })
        .collect();
    let years: Vec<MonthTotals> = yearly
        .into_iter()
        .map(|(year, (income, expense))| MonthTotals {
            year,
            month: 0,
            income: round2(income),
            expense: round2(expense),
        })
        .collect();

    let expense_matrix = matrix(transactions, &months, &expense_categories, Kind::Expense);
    let income_matrix = matrix(transactions, &months, &income_categories, Kind::Income);

    Aggregates {
        months,
        years,
        expense_categories,
        income_categories,
        expense_matrix,
        income_matrix,
    }
}

fn sorted_categories(totals: BTreeMap<String, f64>) -> Vec<String> {
    let mut entries: Vec<(String, f64)> = totals.into_iter().collect();
    entries.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    entries.into_iter().map(|(name, _)| name).collect()
}

fn matrix(
    transactions: &[Transaction],
    months: &[MonthTotals],
    categories: &[String],
    kind: Kind,
) -> Vec<Vec<f64>> {
    let index: BTreeMap<(i32, u32), usize> = months
        .iter()
        .enumerate()
        .map(|(i, m)| ((m.year, m.month), i))
        .collect();
    let category_index: BTreeMap<&str, usize> = categories
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();
    let mut out = vec![vec![0.0; categories.len()]; months.len()];
    for transaction in transactions {
        if transaction.kind != kind {
            continue;
        }
        let Some((year, month)) = parse_month(&transaction.date) else {
            continue;
        };
        let display = model::display_name(&transaction.category);
        if let (Some(&row), Some(&col)) = (
            index.get(&(year, month)),
            category_index.get(display.as_str()),
        ) {
            out[row][col] += transaction.amount;
        }
    }
    for row in &mut out {
        for value in row.iter_mut() {
            *value = round2(*value);
        }
    }
    out
}

/// Totals of the transactions inside the period.
pub fn period_totals<'a>(
    transactions: impl IntoIterator<Item = &'a Transaction>,
    period: &Period,
) -> (f64, f64, usize) {
    let mut income = 0.0;
    let mut expense = 0.0;
    let mut count = 0;
    for transaction in transactions {
        let Some((year, month)) = parse_month(&transaction.date) else {
            continue;
        };
        if !period.includes(year, month) {
            continue;
        }
        count += 1;
        match transaction.kind {
            Kind::Income => income += transaction.amount,
            Kind::Expense => expense += transaction.amount,
        }
    }
    (round2(income), round2(expense), count)
}

/// Per-category totals inside the period, biggest first, with the empty categories dropped.
pub fn period_categories<'a>(
    transactions: impl IntoIterator<Item = &'a Transaction>,
    period: &Period,
    kind: Kind,
) -> Vec<(String, f64)> {
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    for transaction in transactions {
        if transaction.kind != kind {
            continue;
        }
        let Some((year, month)) = parse_month(&transaction.date) else {
            continue;
        };
        if !period.includes(year, month) {
            continue;
        }
        *totals
            .entry(model::display_name(&transaction.category))
            .or_insert(0.0) += transaction.amount;
    }
    let mut entries: Vec<(String, f64)> = totals
        .into_iter()
        .map(|(name, amount)| (name, round2(amount)))
        .filter(|(_, amount)| *amount > 0.0)
        .collect();
    entries.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
    });
    entries
}

/// Builds the workbook and returns its bytes.
pub fn build_workbook(
    transactions: &[Transaction],
    categories: &Categories,
    period: &Period,
) -> Result<Vec<u8>, XlsxError> {
    let aggregates = aggregate(transactions, categories);

    let mut workbook = Workbook::new();
    workbook.add_worksheet().set_name(SHEET_CHARTS)?;
    workbook.add_worksheet().set_name(SHEET_TRANSACTIONS)?;
    workbook.add_worksheet().set_name(SHEET_DATA)?;

    let rows = sorted_for_grid(transactions);
    write_transactions_sheet(workbook.worksheet_from_index(1)?, &rows)?;

    let layout = {
        let data = workbook.worksheet_from_index(2)?;
        let layout = write_data_sheet(data, &rows, &aggregates, period)?;
        data.set_hidden(true);
        layout
    };

    write_charts_sheet(workbook.worksheet_from_index(0)?, &aggregates, &layout)?;

    workbook.save_to_buffer()
}

/// The grid reads oldest first; the sort is stable so `created_at` breaks date ties.
fn sorted_for_grid(transactions: &[Transaction]) -> Vec<&Transaction> {
    let mut rows: Vec<&Transaction> = transactions.iter().collect();
    rows.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    rows
}

fn euro_format() -> Format {
    Format::new().set_num_format("€ #,##0.00")
}

fn integer_format() -> Format {
    Format::new().set_num_format("#,##0")
}

fn write_transactions_sheet(
    sheet: &mut Worksheet,
    transactions: &[&Transaction],
) -> Result<(), XlsxError> {
    let headers = [
        "Data",
        "Anno",
        "Mese",
        "Tipo",
        "Categoria",
        "Descrizione",
        "Entrate",
        "Uscite",
        "Saldo",
        "ID",
        "Creato il",
    ];
    let money = euro_format();
    let year = integer_format();
    let bold = Format::new()
        .set_bold()
        .set_background_color("#1F2937")
        .set_font_color("#FFFFFF");

    // The headers, also used as a plain row when there is nothing to put under them.
    for (col, header) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *header, &bold)?;
    }

    for (row, transaction) in transactions.iter().enumerate() {
        let row = row as u32 + 1;
        let (year_value, month_value) = parse_month(&transaction.date)
            .map(|(y, m)| (y, month_name(m)))
            .unwrap_or((0, ""));
        let display = model::display_name(&transaction.category);
        let (entrate, uscite, saldo) = match transaction.kind {
            Kind::Income => (transaction.amount, 0.0, transaction.amount),
            Kind::Expense => (0.0, transaction.amount, -transaction.amount),
        };

        sheet.write_string(row, 0, &transaction.date)?;
        sheet.write_number_with_format(row, 1, year_value as f64, &year)?;
        sheet.write_string(row, 2, month_value)?;
        sheet.write_string(
            row,
            3,
            match transaction.kind {
                Kind::Income => "Entrata",
                Kind::Expense => "Uscita",
            },
        )?;
        sheet.write_string(row, 4, &display)?;
        sheet.write_string(row, 5, &transaction.description)?;
        sheet.write_number_with_format(row, 6, entrate, &money)?;
        sheet.write_number_with_format(row, 7, uscite, &money)?;
        sheet.write_number_with_format(row, 8, saldo, &money)?;
        sheet.write_string(row, 9, &transaction.id)?;
        sheet.write_string(row, 10, &transaction.created_at)?;
    }

    let widths = [
        12.0, 8.0, 12.0, 10.0, 24.0, 44.0, 14.0, 14.0, 14.0, 40.0, 24.0,
    ];
    for (col, width) in widths.iter().enumerate() {
        sheet.set_column_width(col as u16, *width)?;
    }
    sheet.set_freeze_panes(1, 0)?;

    if transactions.is_empty() {
        let empty = Format::new().set_italic().set_font_color("#6B7280");
        sheet.merge_range(1, 0, 1, 10, "Nessun dato da esportare", &empty)?;
        return Ok(());
    }

    let total_row = TableColumn::new().set_total_label("Totale");
    let plain = || TableColumn::new();
    let sum = || TableColumn::new().set_total_function(TableFunction::Sum);

    let columns = vec![
        total_row,
        plain(),
        plain(),
        plain(),
        plain(),
        plain(),
        sum(),
        sum(),
        sum(),
        plain(),
        plain(),
    ];

    let table = Table::new()
        .set_name("Transazioni")
        .set_style(TableStyle::Light1)
        .set_total_row(true)
        .set_columns(&columns);

    // Rust_xlsxwriter's range includes the totals row, so the table ends one row below the data.
    let last_row = transactions.len() as u32 + 1;
    sheet.add_table(0, 0, last_row, 10, &table)?;

    Ok(())
}

/// Where the aggregate blocks ended up, so the charts can point at them.
struct DataLayout {
    monthly_label_col: u16,
    monthly_first_row: u32,
    monthly_last_row: u32,
    monthly_count: usize,
    yearly_label_col: u16,
    yearly_first_row: u32,
    yearly_last_row: u32,
    expense_donut_first: u32,
    expense_donut_last: u32,
    expense_donut_col: u16,
    expense_donut_count: usize,
    income_donut_first: u32,
    income_donut_last: u32,
    income_donut_col: u16,
    income_donut_count: usize,
    expense_line_col: u16,
    income_line_col: u16,
    line_first: u32,
    line_last: u32,
}

/// Column letter for an Excel column number: 0 → A, 25 → Z, 26 → AA.
fn column_name(mut col: u16) -> String {
    let mut out = Vec::new();
    loop {
        out.push((b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    out.iter().rev().collect()
}

/// An absolute range on `sheet`; rows are 1-based, as Excel writes them.
fn range(sheet: &str, first_row: u32, first_col: u16, last_row: u32, last_col: u16) -> String {
    format!(
        "{}!${}${}:${}${}",
        sheet,
        column_name(first_col),
        first_row,
        column_name(last_col),
        last_row
    )
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// A sum over the transactions table, restricted to the period and optionally one category.
///
/// Excel's `SUMIFS` can only test one month at a time, so a selection of months turns into
/// one `SUMIFS` per month added together. With every month selected it is a single `SUMIFS`
/// (or a plain `SUM` when the years are open too).
fn period_sum(
    value_col: char,
    period: &Period,
    category: Option<&str>,
    last_row: u32,
) -> Option<String> {
    if last_row < 2 {
        return None;
    }
    let joiner = |col: char| format!("{SHEET_TRANSACTIONS}!${col}$2:${col}${last_row}");
    let value = joiner(value_col);
    let mut criteria: Vec<String> = Vec::new();
    if let Some(from) = period.from_year {
        criteria.push(joiner('B'));
        criteria.push(quote(&format!(">={from}")));
    }
    if let Some(to) = period.to_year {
        criteria.push(joiner('B'));
        criteria.push(quote(&format!("<={to}")));
    }
    if let Some(category) = category {
        criteria.push(joiner('E'));
        criteria.push(quote(category));
    }

    if period.months.is_all() {
        return Some(if criteria.is_empty() {
            format!("SUM({value})")
        } else {
            format!("SUMIFS({value},{})", criteria.join(","))
        });
    }

    let terms: Vec<String> = period
        .months
        .iter()
        .map(|month| {
            let mut month_criteria = criteria.clone();
            month_criteria.push(joiner('C'));
            month_criteria.push(quote(month_name(month)));
            format!("SUMIFS({value},{})", month_criteria.join(","))
        })
        .collect();
    Some(terms.join("+"))
}

/// `SUMIFS`/`SUM` over the transactions table, restricted to the period.
fn total_formula(value_col: char, period: &Period, last_row: u32) -> Option<String> {
    period_sum(value_col, period, None, last_row)
}

/// A sum for one month, optionally narrowed to one category.
fn month_formula(
    value_col: char,
    year: i32,
    month: u32,
    category: Option<&str>,
    last_row: u32,
) -> Option<String> {
    if last_row < 2 {
        return None;
    }
    let joiner = |col: char| format!("{SHEET_TRANSACTIONS}!${col}$2:${col}${last_row}");
    let mut parts = vec![joiner(value_col), joiner('B'), format!("\"{year}\"")];
    parts.push(joiner('C'));
    parts.push(quote(month_name(month)));
    if let Some(category) = category {
        parts.push(joiner('E'));
        parts.push(quote(category));
    }
    Some(format!("SUMIFS({},{})", parts[0], parts[1..].join(",")))
}

fn year_formula(value_col: char, year: i32, last_row: u32) -> Option<String> {
    if last_row < 2 {
        return None;
    }
    let joiner = |col: char| format!("{SHEET_TRANSACTIONS}!${col}$2:${col}${last_row}");
    Some(format!(
        "SUMIFS({}, {B}, \"{year}\")",
        joiner(value_col),
        B = joiner('B')
    ))
}

fn write_formula_number(
    sheet: &mut Worksheet,
    row: u32,
    col: u16,
    formula: Option<String>,
    cached: f64,
    format: &Format,
) -> Result<(), XlsxError> {
    match formula {
        Some(formula) => {
            sheet.write_formula_with_format(
                row,
                col,
                Formula::new(formula).set_result(format!("{cached:.2}")),
                format,
            )?;
        }
        None => {
            sheet.write_number_with_format(row, col, cached, format)?;
        }
    }
    Ok(())
}

/// A sum for one category, restricted to the period.
fn category_total_formula(
    value_col: char,
    category: &str,
    period: &Period,
    last_row: u32,
) -> Option<String> {
    period_sum(value_col, period, Some(category), last_row)
}

/// One donut block: a name/amount column pair, one row per category.
#[allow(clippy::too_many_arguments)]
fn write_category_block(
    sheet: &mut Worksheet,
    col: u16,
    title: &str,
    cats: &[(String, f64)],
    value_col: char,
    period: &Period,
    last_row: u32,
    bold: &Format,
    money: &Format,
) -> Result<(u32, u32), XlsxError> {
    sheet.write_string_with_format(0, col, title, bold)?;
    sheet.write_string_with_format(0, col + 1, "Importo", bold)?;
    for (i, (name, amount)) in cats.iter().enumerate() {
        let row = i as u32 + 1;
        sheet.write_string(row, col, name)?;
        write_formula_number(
            sheet,
            row,
            col + 1,
            category_total_formula(value_col, name, period, last_row),
            *amount,
            money,
        )?;
    }
    Ok((2, cats.len() as u32 + 1))
}

fn write_data_sheet(
    sheet: &mut Worksheet,
    transactions: &[&Transaction],
    aggregates: &Aggregates,
    period: &Period,
) -> Result<DataLayout, XlsxError> {
    let bold = Format::new().set_bold();
    let money = euro_format();
    let last_row = transactions.len() as u32 + 1;

    // ── Summary (KPIs) ─────────────────────────────────────────────────────
    let (income, expense, count) = period_totals(transactions.iter().copied(), period);
    sheet.write_string_with_format(0, 0, "Riepilogo", &bold)?;
    sheet.write_string(1, 0, "Periodo")?;
    sheet.write_string(1, 1, period.label())?;
    sheet.write_string(2, 0, "Mese")?;
    sheet.write_string(2, 1, period.months.label())?;
    let lines = [
        ("Entrate", income, Some('G')),
        ("Uscite", expense, Some('H')),
        ("Saldo", round2(income - expense), Some('I')),
        ("Transazioni", count as f64, None),
    ];
    for (i, (label, value, column)) in lines.iter().enumerate() {
        let row = i as u32 + 3;
        sheet.write_string(row, 0, *label)?;
        match column {
            // The Saldo column is signed per row, so its SUMIFS total is already the balance.
            Some(col) => {
                let formula = total_formula(*col, period, last_row);
                write_formula_number(sheet, row, 1, formula, *value, &money)?;
            }
            None => {
                sheet.write_number_with_format(row, 1, *value, &integer_format())?;
            }
        }
    }

    // ── Donuts: per-category totals inside the period ──────────────────────
    let expense_cats = period_categories(transactions.iter().copied(), period, Kind::Expense);
    let income_cats = period_categories(transactions.iter().copied(), period, Kind::Income);

    let expense_donut_col = 4; // E
    let income_donut_col = 7; // H
    let (expense_donut_first, expense_donut_last) = write_category_block(
        sheet,
        expense_donut_col,
        "Uscite per categoria",
        &expense_cats,
        'H',
        period,
        last_row,
        &bold,
        &money,
    )?;
    let (income_donut_first, income_donut_last) = write_category_block(
        sheet,
        income_donut_col,
        "Entrate per categoria",
        &income_cats,
        'G',
        period,
        last_row,
        &bold,
        &money,
    )?;

    // ── Monthly block ──────────────────────────────────────────────────────
    // The monthly chart follows the year range only; the month picker narrows the KPIs and
    // the donuts, not this history.
    let monthly_months: Vec<&MonthTotals> = aggregates
        .months
        .iter()
        .filter(|month| period.includes_year(month.year))
        .collect();
    let monthly_col = 10; // K
    sheet.write_string_with_format(0, monthly_col, "Mese", &bold)?;
    sheet.write_string_with_format(0, monthly_col + 1, "Entrate", &bold)?;
    sheet.write_string_with_format(0, monthly_col + 2, "Uscite", &bold)?;
    sheet.write_string_with_format(0, monthly_col + 3, "Saldo", &bold)?;
    for (i, month) in monthly_months.iter().enumerate() {
        let row = i as u32 + 1;
        sheet.write_string(row, monthly_col, month.label())?;
        write_formula_number(
            sheet,
            row,
            monthly_col + 1,
            month_formula('G', month.year, month.month, None, last_row),
            month.income,
            &money,
        )?;
        write_formula_number(
            sheet,
            row,
            monthly_col + 2,
            month_formula('H', month.year, month.month, None, last_row),
            month.expense,
            &money,
        )?;
        write_formula_number(sheet, row, monthly_col + 3, None, month.balance(), &money)?;
    }

    // ── Yearly block ───────────────────────────────────────────────────────
    let yearly_col = 16; // Q
    sheet.write_string_with_format(0, yearly_col, "Anno", &bold)?;
    sheet.write_string_with_format(0, yearly_col + 1, "Entrate", &bold)?;
    sheet.write_string_with_format(0, yearly_col + 2, "Uscite", &bold)?;
    sheet.write_string_with_format(0, yearly_col + 3, "Saldo", &bold)?;
    for (i, year) in aggregates.years.iter().enumerate() {
        let row = i as u32 + 1;
        sheet.write_number(row, yearly_col, year.year as f64)?;
        write_formula_number(
            sheet,
            row,
            yearly_col + 1,
            year_formula('G', year.year, last_row),
            year.income,
            &money,
        )?;
        write_formula_number(
            sheet,
            row,
            yearly_col + 2,
            year_formula('H', year.year, last_row),
            year.expense,
            &money,
        )?;
        write_formula_number(sheet, row, yearly_col + 3, None, year.balance(), &money)?;
    }

    // ── Line-chart matrices, months × category ─────────────────────────────
    let write_matrix = |sheet: &mut Worksheet,
                        col: u16,
                        categories: &[String],
                        matrix: &[Vec<f64>],
                        value_col: char|
     -> Result<(), XlsxError> {
        sheet.write_string_with_format(0, col, "Mese", &bold)?;
        for (i, category) in categories.iter().enumerate() {
            sheet.write_string_with_format(0, col + 1 + i as u16, category, &bold)?;
        }
        for (row, month) in aggregates.months.iter().enumerate() {
            let excel_row = row as u32 + 1;
            sheet.write_string(excel_row, col, month.label())?;
            let inside = period.includes(month.year, month.month);
            for (i, category) in categories.iter().enumerate() {
                let value = matrix
                    .get(row)
                    .and_then(|r| r.get(i))
                    .copied()
                    .unwrap_or(0.0);
                let cell_col = col + 1 + i as u16;
                if !inside {
                    // Outside the period: leave the cell empty so the line has a gap.
                    continue;
                }
                write_formula_number(
                    sheet,
                    excel_row,
                    cell_col,
                    month_formula(value_col, month.year, month.month, Some(category), last_row),
                    value,
                    &money,
                )?;
            }
        }
        Ok(())
    };

    let expense_line_col = 22; // W
    write_matrix(
        sheet,
        expense_line_col,
        &aggregates.expense_categories,
        &aggregates.expense_matrix,
        'H',
    )?;
    let income_line_col = expense_line_col + aggregates.expense_categories.len() as u16 + 2;
    write_matrix(
        sheet,
        income_line_col,
        &aggregates.income_categories,
        &aggregates.income_matrix,
        'G',
    )?;

    Ok(DataLayout {
        monthly_label_col: monthly_col,
        monthly_first_row: 2,
        monthly_last_row: monthly_months.len() as u32 + 1,
        monthly_count: monthly_months.len(),
        yearly_label_col: yearly_col,
        yearly_first_row: 2,
        yearly_last_row: aggregates.years.len() as u32 + 1,
        expense_donut_first,
        expense_donut_last,
        expense_donut_col,
        expense_donut_count: expense_cats.len(),
        income_donut_first,
        income_donut_last,
        income_donut_col,
        income_donut_count: income_cats.len(),
        expense_line_col,
        income_line_col,
        line_first: 2,
        line_last: aggregates.months.len() as u32 + 1,
    })
}

fn write_charts_sheet(
    sheet: &mut Worksheet,
    aggregates: &Aggregates,
    layout: &DataLayout,
) -> Result<(), XlsxError> {
    sheet.set_tab_color("#2563EB");

    if aggregates.months.is_empty() {
        sheet.write_string_with_format(
            0,
            0,
            "Nessun dato da esportare",
            &Format::new().set_bold().set_font_size(16),
        )?;
        return Ok(());
    }

    let monthly = (layout.monthly_count > 0).then(|| monthly_chart(layout));
    let yearly = yearly_chart(layout);
    let expense_donut = (layout.expense_donut_count > 0).then(|| {
        donut_chart(
            "Uscite per categoria",
            layout.expense_donut_first,
            layout.expense_donut_last,
            layout.expense_donut_col,
            "Uscite",
        )
    });
    let income_donut = (layout.income_donut_count > 0).then(|| {
        donut_chart(
            "Entrate per categoria",
            layout.income_donut_first,
            layout.income_donut_last,
            layout.income_donut_col,
            "Entrate",
        )
    });
    let expense_lines = (!aggregates.expense_categories.is_empty()).then(|| {
        line_chart(
            "Andamento mensile delle uscite per categoria",
            aggregates,
            layout,
            false,
        )
    });
    let income_lines = (!aggregates.income_categories.is_empty()).then(|| {
        line_chart(
            "Andamento mensile delle entrate per categoria",
            aggregates,
            layout,
            true,
        )
    });

    let mut placements: Vec<(u32, u16, &Chart)> = Vec::new();
    if let Some(chart) = &monthly {
        placements.push((0, 0, chart));
    }
    placements.push((0, 14, &yearly));
    if let Some(chart) = &expense_donut {
        placements.push((18, 0, chart));
    }
    if let Some(chart) = &income_donut {
        placements.push((18, 14, chart));
    }
    if let Some(chart) = &expense_lines {
        placements.push((36, 0, chart));
    }
    if let Some(chart) = &income_lines {
        placements.push((36, 14, chart));
    }

    for (row, col, chart) in placements {
        sheet.insert_chart_with_offset(row, col, chart, 0, 0)?;
    }
    Ok(())
}

fn monthly_chart(layout: &DataLayout) -> Chart {
    let mut chart = Chart::new(ChartType::Column);
    chart.title().set_name("Entrate, uscite e saldo per mese");
    chart.set_width(860).set_height(320);
    chart.legend().set_position(ChartLegendPosition::Bottom);
    chart.y_axis().set_num_format("€ #,##0");

    let categories = range(
        SHEET_DATA,
        layout.monthly_first_row,
        layout.monthly_label_col,
        layout.monthly_last_row,
        layout.monthly_label_col,
    );
    let labels = ["Entrate", "Uscite", "Saldo"];
    let colors = ["#16A34A", "#DC2626", "#2563EB"];
    for (i, (label, color)) in labels.iter().zip(colors).enumerate() {
        let col = layout.monthly_label_col + 1 + i as u16;
        let values = range(
            SHEET_DATA,
            layout.monthly_first_row,
            col,
            layout.monthly_last_row,
            col,
        );
        chart
            .add_series()
            .set_name(*label)
            .set_categories(categories.as_str())
            .set_values(values.as_str())
            .set_format(rust_xlsxwriter::ChartSolidFill::new().set_color(color));
    }
    chart
}

fn yearly_chart(layout: &DataLayout) -> Chart {
    let mut chart = Chart::new(ChartType::Column);
    chart.title().set_name("Entrate, uscite e saldo per anno");
    chart.set_width(860).set_height(320);
    chart.legend().set_position(ChartLegendPosition::Bottom);
    chart.y_axis().set_num_format("€ #,##0");

    let categories = range(
        SHEET_DATA,
        layout.yearly_first_row,
        layout.yearly_label_col,
        layout.yearly_last_row,
        layout.yearly_label_col,
    );
    let labels = ["Entrate", "Uscite", "Saldo"];
    let colors = ["#16A34A", "#DC2626", "#2563EB"];
    for (i, (label, color)) in labels.iter().zip(colors).enumerate() {
        let col = layout.yearly_label_col + 1 + i as u16;
        let values = range(
            SHEET_DATA,
            layout.yearly_first_row,
            col,
            layout.yearly_last_row,
            col,
        );
        chart
            .add_series()
            .set_name(*label)
            .set_categories(categories.as_str())
            .set_values(values.as_str())
            .set_format(rust_xlsxwriter::ChartSolidFill::new().set_color(color));
    }
    chart
}

fn donut_chart(
    title: &str,
    first_row: u32,
    last_row: u32,
    category_col: u16,
    series_name: &str,
) -> Chart {
    let mut chart = Chart::new(ChartType::Doughnut);
    chart.title().set_name(title);
    chart.set_width(520).set_height(320);
    chart.legend().set_position(ChartLegendPosition::Right);
    chart.set_hole_size(55);

    let categories = range(SHEET_DATA, first_row, category_col, last_row, category_col);
    let values = range(
        SHEET_DATA,
        first_row,
        category_col + 1,
        last_row,
        category_col + 1,
    );
    chart
        .add_series()
        .set_name(series_name)
        .set_categories(categories.as_str())
        .set_values(values.as_str());
    chart
}

fn line_chart(title: &str, aggregates: &Aggregates, layout: &DataLayout, income: bool) -> Chart {
    let (matrix_col, categories) = if income {
        (layout.income_line_col, &aggregates.income_categories)
    } else {
        (layout.expense_line_col, &aggregates.expense_categories)
    };
    let mut chart = Chart::new(ChartType::Line);
    chart.title().set_name(title);
    chart.set_width(860).set_height(360);
    chart.legend().set_position(ChartLegendPosition::Bottom);
    chart.show_empty_cells_as(ChartEmptyCells::Gaps);
    chart.y_axis().set_num_format("€ #,##0");
    let interval = if aggregates.months.len() > 14 { 2 } else { 1 };
    chart.x_axis().set_label_interval(interval);

    let labels = range(
        SHEET_DATA,
        layout.line_first,
        matrix_col,
        layout.line_last,
        matrix_col,
    );
    for (i, category) in categories.iter().enumerate() {
        let col = matrix_col + 1 + i as u16;
        let values = range(SHEET_DATA, layout.line_first, col, layout.line_last, col);
        chart
            .add_series()
            .set_name(category.as_str())
            .set_categories(labels.as_str())
            .set_values(values.as_str())
            .set_format(
                rust_xlsxwriter::ChartLine::new()
                    .set_color(SERIES_COLORS[i % SERIES_COLORS.len()])
                    .set_width(1.75),
            );
    }
    chart
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(kind: Kind, amount: f64, category: &str, date: &str) -> Transaction {
        Transaction {
            id: format!("id-{date}-{amount}"),
            kind,
            amount,
            category: category.to_owned(),
            description: String::new(),
            date: date.to_owned(),
            created_at: format!("{date}T10:00:00.000000"),
        }
    }

    fn sample() -> (Vec<Transaction>, Categories) {
        let transactions = vec![
            transaction(Kind::Income, 1500.0, "💰 Salary", "2023-01-10"),
            transaction(Kind::Expense, 200.0, "🍔 Food & Dining", "2023-01-15"),
            transaction(Kind::Expense, 100.0, " Food & Dining", "2023-02-03"),
            transaction(Kind::Income, 1600.0, "💰 Salary", "2024-01-09"),
            transaction(Kind::Expense, 300.0, "🏠 Housing & Rent", "2024-01-20"),
        ];
        let categories = Categories {
            expense: vec!["🍔 Food & Dining".into(), "🏠 Housing & Rent".into()],
            income: vec!["💰 Salary".into()],
        };
        (transactions, categories)
    }

    #[test]
    fn aggregation_groups_months_and_categories() {
        let (transactions, categories) = sample();
        let aggregates = aggregate(&transactions, &categories);

        assert_eq!(aggregates.months.len(), 3);
        assert_eq!(aggregates.months[0].year, 2023);
        assert_eq!(aggregates.months[0].income, 1500.0);
        assert_eq!(aggregates.months[0].expense, 200.0);
        assert_eq!(aggregates.months[1].expense, 100.0);

        // Legacy spellings merge into one display category.
        assert_eq!(aggregates.expense_categories[0], "Food & Dining");
        assert_eq!(aggregates.expense_matrix[0][0], 200.0);
        assert_eq!(aggregates.expense_matrix[1][0], 100.0);

        assert_eq!(aggregates.years.len(), 2);
        assert_eq!(aggregates.years[0].income, 1500.0);
        assert_eq!(aggregates.years[1].expense, 300.0);
    }

    #[test]
    fn period_filters_years_and_months() {
        let (transactions, _) = sample();
        let all = Period::default();
        assert_eq!(period_totals(&transactions, &all), (3100.0, 600.0, 5));

        let january = Period {
            from_year: Some(2023),
            to_year: Some(2024),
            months: MonthSet::single(1),
        };
        assert_eq!(period_totals(&transactions, &january), (3100.0, 500.0, 4));

        // February only: the January income and the 2024 expenses drop out.
        let february = Period {
            months: MonthSet::single(2),
            ..Period::default()
        };
        assert_eq!(period_totals(&transactions, &february), (0.0, 100.0, 1));

        // January and February together cover every sample transaction.
        let jan_feb = Period {
            months: MonthSet::from_months([1, 2]),
            ..Period::default()
        };
        assert_eq!(period_totals(&transactions, &jan_feb), (3100.0, 600.0, 5));

        let food = period_categories(&transactions, &all, Kind::Expense);
        assert_eq!(food[0], ("Food & Dining".to_owned(), 300.0));
        assert_eq!(food[1], ("Housing & Rent".to_owned(), 300.0));
    }

    #[test]
    fn month_set_labels_and_membership() {
        assert!(MonthSet::default().is_all());
        assert_eq!(MonthSet::default().label(), "Tutti i mesi");
        assert_eq!(MonthSet::NONE.len(), 0);
        assert!(MonthSet::NONE.is_empty());

        let chosen = MonthSet::from_months([1, 3, 12]);
        assert_eq!(chosen.len(), 3);
        assert!(chosen.contains(3));
        assert!(!chosen.contains(2));
        assert_eq!(chosen.iter().collect::<Vec<_>>(), vec![1, 3, 12]);
        assert_eq!(chosen.label(), "Gennaio, Marzo, Dicembre");
        assert_eq!(MonthSet::single(1).label(), "Gennaio");
    }

    #[test]
    fn multi_month_formulas_sum_one_sumifs_per_month() {
        let all = Period::default();
        assert_eq!(
            total_formula('G', &all, 10).unwrap(),
            "SUM(Transazioni!$G$2:$G$10)"
        );

        let chosen = Period {
            from_year: Some(2023),
            months: MonthSet::from_months([1, 6]),
            ..Period::default()
        };
        let formula = total_formula('G', &chosen, 10).unwrap();
        assert_eq!(formula.matches("SUMIFS(").count(), 2);
        assert!(formula.contains('+'));
        assert!(formula.contains("\"Gennaio\""));
        assert!(formula.contains("\"Giugno\""));
        assert!(formula.contains("\">=2023\""));

        // Every month selected collapses to a single SUMIFS with just the year filter.
        let year_only = Period {
            from_year: Some(2023),
            ..Period::default()
        };
        let formula = total_formula('G', &year_only, 10).unwrap();
        assert_eq!(formula.matches("SUMIFS(").count(), 1);
        assert!(formula.contains("\">=2023\""));
    }

    #[test]
    fn a_workbook_has_the_three_sheets_and_the_table() {
        let (transactions, categories) = sample();
        let bytes = build_workbook(&transactions, &categories, &Period::default()).unwrap();

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        assert!(names.iter().any(|n| n == "xl/charts/chart1.xml"));
        assert!(names.iter().any(|n| n == "xl/charts/chart6.xml"));
        assert!(names.iter().any(|n| n == "xl/tables/table1.xml"));

        let mut workbook = String::new();
        use std::io::Read;
        zip.by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut workbook)
            .unwrap();
        assert!(workbook.contains(SHEET_CHARTS));
        assert!(workbook.contains(SHEET_TRANSACTIONS));
        assert!(workbook.contains(SHEET_DATA));

        let mut table = String::new();
        zip.by_name("xl/tables/table1.xml")
            .unwrap()
            .read_to_string(&mut table)
            .unwrap();
        assert!(table.contains("totalsRowFunction=\"sum\""));
        assert!(table.contains("autoFilter"));

        let mut sheet = String::new();
        zip.by_name("xl/worksheets/sheet2.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains("SUBTOTAL(109"));
    }

    #[test]
    fn the_monthly_chart_follows_the_year_range() {
        let (transactions, categories) = sample();
        let period = Period {
            from_year: Some(2024),
            ..Period::default()
        };
        let bytes = build_workbook(&transactions, &categories, &period).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();

        let mut chart = String::new();
        use std::io::Read;
        zip.by_name("xl/charts/chart1.xml")
            .unwrap()
            .read_to_string(&mut chart)
            .unwrap();
        // Only January 2024 survives the filter, so the series span a single row.
        assert!(chart.contains("Dati!$K$2"));
        assert!(chart.contains("Dati!$L$2"));
        assert!(!chart.contains("$K$3"));
    }

    #[test]
    fn an_empty_dataset_still_builds() {
        let bytes = build_workbook(&[], &Categories::default(), &Period::default()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();

        // Headers and a note, but no half-drawn chart and no table without rows.
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        assert!(!names.iter().any(|name| name.starts_with("xl/charts/")));
        assert!(!names.iter().any(|name| name.starts_with("xl/tables/")));

        let mut strings = String::new();
        use std::io::Read;
        zip.by_name("xl/sharedStrings.xml")
            .unwrap()
            .read_to_string(&mut strings)
            .unwrap();
        assert!(strings.contains("Nessun dato da esportare"));
        assert!(strings.contains("Data"));
        assert!(strings.contains("Entrate"));
    }
}

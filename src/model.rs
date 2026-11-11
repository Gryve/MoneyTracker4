//! Domain types shared by the database layer and the views.

use std::collections::HashMap;

/// Whether a transaction is money coming in or going out. Stored in SQLite as `'income'` or
/// `'expense'`, exactly as the Python application writes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    Income,
    Expense,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Income => "income",
            Kind::Expense => "expense",
        }
    }

    /// The inverse of [`Kind::as_str`]. Any unexpected value is read as an expense, which the
    /// schema's `CHECK` makes unreachable.
    pub fn parse(s: &str) -> Kind {
        if s == "income" {
            Kind::Income
        } else {
            Kind::Expense
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Income => "Income",
            Kind::Expense => "Expense",
        }
    }

    pub fn sign(self) -> &'static str {
        match self {
            Kind::Income => "+",
            Kind::Expense => "−",
        }
    }
}

/// One row of `transactions`, in the same shape `data_manager.py` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub id: String,
    pub kind: Kind,
    pub amount: f64,
    pub category: String,
    pub description: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Local ISO timestamp, as Python's `datetime.now().isoformat()` writes it.
    pub created_at: String,
}

/// `load_categories()`'s result: names per kind, sorted as the query returns them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Categories {
    pub expense: Vec<String>,
    pub income: Vec<String>,
}

impl Categories {
    pub fn of(&self, kind: Kind) -> &Vec<String> {
        match kind {
            Kind::Expense => &self.expense,
            Kind::Income => &self.income,
        }
    }

    pub fn of_mut(&mut self, kind: Kind) -> &mut Vec<String> {
        match kind {
            Kind::Expense => &mut self.expense,
            Kind::Income => &mut self.income,
        }
    }
}

/// `get_monthly_summary()`'s result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonthlySummary {
    pub total_income: f64,
    pub total_expense: f64,
    pub balance: f64,
    /// `YYYY-MM-DD` → (income, expense).
    pub daily: HashMap<String, (f64, f64)>,
    pub category_expenses: HashMap<String, f64>,
    pub category_income: HashMap<String, f64>,
    pub transaction_count: usize,
}

/// `get_all_time_balance()`'s result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Balance {
    pub total_income: f64,
    pub total_expense: f64,
    pub balance: f64,
}

/// `get_annual_summary()`'s result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnualSummary {
    pub total_expense: f64,
    pub category_expenses: HashMap<String, f64>,
}

// ── Formatting ───────────────────────────────────────────────────────────────

/// `€1.234,56`, as the Python app shows amounts, with the sign kept separate.
pub fn money(amount: f64) -> String {
    let cents_total = (amount.abs() * 100.0).round() as i64;
    let euros = (cents_total / 100).to_string();
    let cents = cents_total % 100;
    let mut grouped = String::with_capacity(euros.len() + euros.len() / 3);
    for (i, ch) in euros.chars().enumerate() {
        if i > 0 && (euros.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    format!(
        "{}{},{:02}",
        if amount < 0.0 { "−" } else { "" },
        grouped,
        cents
    )
}

/// Signed amount with the euro sign: `+€1.234,56`, `−€12,00`.
pub fn signed_money(amount: f64) -> String {
    format!(
        "{}€{}",
        if amount < 0.0 { "−" } else { "+" },
        money(amount.abs())
    )
}

/// `€1.234,56`.
pub fn euros(amount: f64) -> String {
    format!("€{}", money(amount))
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// `October 2026`, as `strftime("%B %Y")` gives it.
pub fn month_name(year: i32, month: u32) -> String {
    format!("{} {}", MONTHS[(month.clamp(1, 12) - 1) as usize], year)
}

/// Just the month's English name, for tables that carry the year in their header.
pub fn month_only(month: u32) -> &'static str {
    MONTHS[(month.clamp(1, 12) - 1) as usize]
}

/// The twelve month names, January to December, for the pickers' option lists.
pub fn month_names() -> Vec<&'static str> {
    MONTHS.to_vec()
}

/// `Sunday, October 25`, as `strftime("%A, %B %d")` gives it.
pub fn long_date(date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    format!(
        "{}, {} {:02}",
        WEEKDAYS[date.weekday().num_days_from_monday() as usize],
        MONTHS[(date.month0()) as usize],
        date.day()
    )
}

/// The emoji the Python seed data puts at the front of a category name.
pub fn strip_emoji(name: &str) -> &str {
    let rest = name.trim_start();
    let cut = rest
        .char_indices()
        .find(|(_, ch)| {
            !(ch.is_whitespace()
                || matches!(*ch,
                    '\u{1F000}'..='\u{1FAFF}'
                    | '\u{2600}'..='\u{27BF}'
                    | '\u{2190}'..='\u{21FF}'
                    | '\u{2B00}'..='\u{2BFF}'
                    | '\u{FE0F}'
                    | '\u{200D}'
                    | '\u{20E3}'))
        })
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    let stripped = rest[cut..].trim_start();
    if stripped.is_empty() {
        name.trim()
    } else {
        stripped
    }
}

/// The name shown in the UI: the stored category without its emoji and stray spaces.
///
/// The database holds a few legacy spellings of the same category (`"🍔 Food & Dining"` and
/// `" Food & Dining"`), so views group by this instead of the raw string.
pub fn display_name(category: &str) -> String {
    strip_emoji(category).to_owned()
}

/// Merges any `{category: amount}` map (stored names) into `{display name: amount}`, preserving
/// nothing about the order: callers sort as they need.
pub fn merge_by_display(map: &HashMap<String, f64>) -> HashMap<String, f64> {
    let mut out: HashMap<String, f64> = HashMap::new();
    for (category, amount) in map {
        *out.entry(display_name(category)).or_insert(0.0) += *amount;
    }
    for value in out.values_mut() {
        *value = (*value * 100.0).round() / 100.0;
    }
    out
}

/// Distinct display names among `categories`, in alphabetical order (case-insensitive, so a
/// lowercase name the user typed sorts with the seeded ones).
pub fn display_groups(categories: &[String]) -> Vec<String> {
    let mut names: Vec<String> = categories.iter().map(|c| display_name(c)).collect();
    names.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    names.dedup();
    names
}

/// The stored names that show up as `display`, alphabetically.
pub fn stored_names(categories: &[String], display: &str) -> Vec<String> {
    let mut names: Vec<String> = categories
        .iter()
        .filter(|c| display_name(c) == display)
        .cloned()
        .collect();
    names.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_matches_the_python_format() {
        assert_eq!(euros(1234.5), "€1.234,50");
        assert_eq!(euros(0.0), "€0,00");
        assert_eq!(euros(999.999), "€1.000,00");
        assert_eq!(money(12.0), "12,00");
        assert_eq!(signed_money(-42.4), "−€42,40");
    }

    #[test]
    fn categories_lose_their_emoji() {
        assert_eq!(strip_emoji("🍔 Food & Dining"), "Food & Dining");
        assert_eq!(strip_emoji("🏠 Housing & Rent"), "Housing & Rent");
        assert_eq!(strip_emoji("📈 Investments"), "Investments");
        assert_eq!(strip_emoji("Other"), "Other");
    }

    #[test]
    fn month_names_are_english() {
        assert_eq!(month_name(2026, 10), "October 2026");
        assert_eq!(
            long_date(chrono::NaiveDate::from_ymd_opt(2026, 10, 25).unwrap()),
            "Sunday, October 25"
        );
    }

    #[test]
    fn legacy_spellings_merge_into_one_display_name() {
        let mut raw = HashMap::new();
        raw.insert("🍔 Food & Dining".to_owned(), 1078.5);
        raw.insert(" Food & Dining".to_owned(), 221.5);
        let merged = merge_by_display(&raw);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged["Food & Dining"], 1300.0);

        let cats = [
            "🍔 Food & Dining".to_owned(),
            " Food & Dining".to_owned(),
            "✈️ Travel".to_owned(),
        ];
        assert_eq!(display_groups(&cats), vec!["Food & Dining", "Travel"]);
        assert_eq!(
            stored_names(&cats, "Food & Dining"),
            vec![" Food & Dining", "🍔 Food & Dining"]
        );
    }
}

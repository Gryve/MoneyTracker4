//! Persistence layer: a 1:1 port of `data_manager.py`.
//!
//! Same file (`data/moneytracker.db`), same schema, same SQL strings, same semantics: amounts are
//! rounded to two decimals, `created_at` is a local ISO timestamp with microseconds, ids are
//! UUID v4, and `save_transactions` still replaces the whole table. Keeping the queries verbatim
//! means the Rust app and the Python app can share the database.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::model::{AnnualSummary, Balance, Categories, Kind, MonthlySummary, Transaction};

pub const DEFAULT_EXPENSE_CATEGORIES: [&str; 14] = [
    "🍔 Food & Dining",
    "🏠 Housing & Rent",
    "🚗 Transportation",
    "🛒 Groceries",
    "💊 Health & Medical",
    "🎭 Entertainment",
    "👕 Clothing",
    "📚 Education",
    "💡 Utilities",
    "📱 Subscriptions",
    "🎁 Gifts",
    "✈️ Travel",
    "🔧 Maintenance",
    "💼 Other Expenses",
];

pub const DEFAULT_INCOME_CATEGORIES: [&str; 8] = [
    "💰 Salary",
    "💵 Freelance",
    "📈 Investments",
    "🏦 Interest",
    "🎁 Gifts Received",
    "💼 Business Income",
    "📦 Refunds",
    "💸 Other Income",
];

pub type DbResult<T> = rusqlite::Result<T>;

const TX_COLUMNS: &str = "id, type, amount, category, description, date, created_at";

/// A handle on the SQLite file. Every call opens its own connection, as the Python layer does.
#[derive(Clone, Debug)]
pub struct Db {
    file: PathBuf,
}

impl Db {
    /// `MT_DB_FILE` wins; otherwise `data/moneytracker.db` beside the executable. A debug build
    /// launched by `cargo run` (binary under `target/debug`) falls back to the project's own
    /// `data/`, so development never creates a stray database under `target/`.
    pub fn resolve() -> Db {
        if let Ok(custom) = std::env::var("MT_DB_FILE") {
            if !custom.trim().is_empty() {
                return Db {
                    file: PathBuf::from(custom),
                };
            }
        }
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
        {
            let beside = dir.join("data").join("moneytracker.db");
            let in_target = dir
                .file_name()
                .is_some_and(|n| n == "debug" || n == "release")
                && dir
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|n| n == "target");
            if !in_target {
                return Db { file: beside };
            }
        }
        Db {
            file: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("data")
                .join("moneytracker.db"),
        }
    }

    pub fn at(path: impl Into<PathBuf>) -> Db {
        Db { file: path.into() }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    fn conn(&self) -> DbResult<Connection> {
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Connection::open(&self.file)
    }

    /// `_init_db()` + `_migrate_from_json()`: creates the schema, seeds the default categories
    /// when the table is empty, and imports legacy JSON once.
    pub fn init(&self) -> DbResult<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS categories (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL UNIQUE,
                 type TEXT NOT NULL CHECK(type IN ('income', 'expense'))
             );
             CREATE TABLE IF NOT EXISTS transactions (
                 id TEXT PRIMARY KEY,
                 type TEXT NOT NULL CHECK(type IN ('income', 'expense')),
                 amount REAL NOT NULL,
                 category TEXT NOT NULL,
                 description TEXT DEFAULT '',
                 date TEXT NOT NULL,
                 created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
             CREATE INDEX IF NOT EXISTS idx_transactions_type ON transactions(type);
             CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category);
             CREATE TABLE IF NOT EXISTS budgets (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 year INTEGER NOT NULL,
                 category TEXT NOT NULL,
                 amount REAL NOT NULL CHECK(amount >= 0),
                 UNIQUE(year, category)
             );",
        )?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
        if count == 0 {
            for cat in DEFAULT_EXPENSE_CATEGORIES {
                conn.execute(
                    "INSERT INTO categories (name, type) VALUES (?1, 'expense')",
                    params![cat],
                )?;
            }
            for cat in DEFAULT_INCOME_CATEGORIES {
                conn.execute(
                    "INSERT INTO categories (name, type) VALUES (?1, 'income')",
                    params![cat],
                )?;
            }
        }
        drop(conn);
        self.migrate_from_json()
    }

    fn migrate_from_json(&self) -> DbResult<()> {
        let dir = match self.file.parent() {
            Some(d) => d.to_path_buf(),
            None => return Ok(()),
        };
        let json_transactions = dir.join("transactions.json");
        let json_categories = dir.join("categories.json");
        if !json_transactions.exists() && !json_categories.exists() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }

        let tx = conn.transaction()?;
        if json_categories.exists() {
            if let Ok(src) = std::fs::read_to_string(&json_categories) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&src) {
                    for kind in ["expense", "income"] {
                        if let Some(list) = value.get(kind).and_then(|v| v.as_array()) {
                            for name in list.iter().filter_map(|v| v.as_str()) {
                                let _ = tx.execute(
                                    "INSERT OR IGNORE INTO categories (name, type) VALUES (?1, ?2)",
                                    params![name, kind],
                                );
                            }
                        }
                    }
                }
            }
        }

        if json_transactions.exists() {
            if let Ok(src) = std::fs::read_to_string(&json_transactions) {
                if let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&src) {
                    for t in list {
                        let field = |k: &str| {
                            t.get(k)
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_owned()
                        };
                        let amount = t.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let created = t
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_owned())
                            .unwrap_or_else(now_iso);
                        let _ = tx.execute(
                            "INSERT INTO transactions (id, type, amount, category, description, date, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            params![
                                field("id"),
                                field("type"),
                                amount,
                                field("category"),
                                field("description"),
                                field("date"),
                                created
                            ],
                        );
                    }
                }
            }
        }
        tx.commit()
    }

    // ── Categories ──────────────────────────────────────────────────────────

    pub fn load_categories(&self) -> DbResult<Categories> {
        let conn = self.conn()?;
        let mut out = Categories::default();
        let mut stmt =
            conn.prepare("SELECT name FROM categories WHERE type = 'expense' ORDER BY name")?;
        out.expense = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let mut stmt =
            conn.prepare("SELECT name FROM categories WHERE type = 'income' ORDER BY name")?;
        out.income = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(out)
    }

    pub fn save_categories(&self, categories: &Categories) -> DbResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM categories", [])?;
        for cat in &categories.expense {
            tx.execute(
                "INSERT INTO categories (name, type) VALUES (?1, 'expense')",
                params![cat],
            )?;
        }
        for cat in &categories.income {
            tx.execute(
                "INSERT INTO categories (name, type) VALUES (?1, 'income')",
                params![cat],
            )?;
        }
        tx.execute(
            "DELETE FROM budgets WHERE category NOT IN (SELECT name FROM categories)",
            [],
        )?;
        tx.commit()
    }

    // ── Budgets ─────────────────────────────────────────────────────────────

    pub fn load_budgets(&self, year: i32) -> DbResult<HashMap<String, f64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT category, amount FROM budgets WHERE year = ?1")?;
        let rows = stmt.query_map(params![year], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
    }

    pub fn save_year_budgets(&self, year: i32, budgets: &HashMap<String, f64>) -> DbResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM budgets WHERE year = ?1", params![year])?;
        for (category, amount) in budgets {
            if *amount <= 0.0 {
                continue;
            }
            tx.execute(
                "INSERT INTO budgets (year, category, amount) VALUES (?1, ?2, ?3)",
                params![year, category, round2(*amount)],
            )?;
        }
        tx.commit()
    }

    /// Budgets keyed by display name (the category without its emoji). Legacy spellings of the
    /// same category are summed, so the UI never shows "Food & Dining" twice.
    pub fn load_budgets_by_display(&self, year: i32) -> DbResult<HashMap<String, f64>> {
        Ok(crate::model::merge_by_display(&self.load_budgets(year)?))
    }

    /// Writes budgets keyed by display name. Each display group keeps a single stored row: the
    /// spelling that already had a budget, else the first one alphabetically. Clearing a budget
    /// removes it, and `save_year_budgets` drops the group's other rows on the way.
    pub fn save_budgets_by_display(
        &self,
        year: i32,
        categories: &Categories,
        budgets: &HashMap<String, f64>,
    ) -> DbResult<()> {
        let existing = self.load_budgets(year)?;
        let mut raw = HashMap::new();
        let mut displays: Vec<&String> = budgets.keys().collect();
        displays.sort();
        for display in displays {
            let amount = budgets[display];
            if amount <= 0.0 {
                continue;
            }
            let names = crate::model::stored_names(&categories.expense, display);
            let canonical = names
                .iter()
                .find(|n| existing.get(*n).is_some_and(|v| *v > 0.0))
                .or_else(|| names.first())
                .cloned();
            if let Some(name) = canonical {
                raw.insert(name, amount);
            }
        }
        self.save_year_budgets(year, &raw)
    }

    // ── Summaries ───────────────────────────────────────────────────────────

    pub fn get_annual_summary(&self, year: i32) -> DbResult<AnnualSummary> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT category, SUM(amount) FROM transactions
             WHERE type = 'expense' AND substr(date, 1, 4) = ?1
             GROUP BY category",
        )?;
        let prefix = format!("{year:04}");
        let rows = stmt.query_map(params![prefix], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        let mut category_expenses = HashMap::new();
        for row in rows {
            let (category, amount) = row?;
            category_expenses.insert(category, round2(amount));
        }
        let total = round2(category_expenses.values().sum());
        Ok(AnnualSummary {
            total_expense: total,
            category_expenses,
        })
    }

    pub fn get_monthly_summary(&self, year: i32, month: u32) -> DbResult<MonthlySummary> {
        let transactions = self.get_transactions_by_month(year, month)?;
        let mut summary = MonthlySummary {
            transaction_count: transactions.len(),
            ..Default::default()
        };
        for t in &transactions {
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
        summary.total_income = round2(summary.total_income);
        summary.total_expense = round2(summary.total_expense);
        summary.balance = round2(summary.total_income - summary.total_expense);
        Ok(summary)
    }

    pub fn get_all_time_balance(&self) -> DbResult<Balance> {
        let conn = self.conn()?;
        let total_income: f64 = conn
            .query_row(
                "SELECT SUM(amount) FROM transactions WHERE type = 'income'",
                [],
                |r| r.get::<_, Option<f64>>(0),
            )?
            .unwrap_or(0.0);
        let total_expense: f64 = conn
            .query_row(
                "SELECT SUM(amount) FROM transactions WHERE type = 'expense'",
                [],
                |r| r.get::<_, Option<f64>>(0),
            )?
            .unwrap_or(0.0);
        Ok(Balance {
            total_income: round2(total_income),
            total_expense: round2(total_expense),
            balance: round2(total_income - total_expense),
        })
    }

    // ── Transactions ────────────────────────────────────────────────────────

    pub fn load_transactions(&self) -> DbResult<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TX_COLUMNS} FROM transactions ORDER BY date DESC, created_at DESC"
        ))?;
        let rows = stmt.query_map([], tx_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// `save_transactions()`: replaces every row, as the Python layer does.
    pub fn save_transactions(&self, transactions: &[Transaction]) -> DbResult<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM transactions", [])?;
        for t in transactions {
            tx.execute(
                "INSERT INTO transactions (id, type, amount, category, description, date, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![t.id, t.kind.as_str(), t.amount, t.category, t.description, t.date, t.created_at],
            )?;
        }
        tx.commit()
    }

    pub fn add_transaction(
        &self,
        kind: Kind,
        amount: f64,
        category: &str,
        description: &str,
        date: Option<&str>,
    ) -> DbResult<Transaction> {
        let transaction = Transaction {
            id: Uuid::new_v4().to_string(),
            kind,
            amount: round2(amount),
            category: category.to_owned(),
            description: description.to_owned(),
            date: date.map(|d| d.to_owned()).unwrap_or_else(today),
            created_at: now_iso(),
        };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO transactions (id, type, amount, category, description, date, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                transaction.id,
                transaction.kind.as_str(),
                transaction.amount,
                transaction.category,
                transaction.description,
                transaction.date,
                transaction.created_at
            ],
        )?;
        Ok(transaction)
    }

    pub fn delete_transaction(&self, id: &str) -> DbResult<bool> {
        let conn = self.conn()?;
        let deleted = conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])? > 0;
        Ok(deleted)
    }

    /// `update_transaction(**fields)`: only the mutable fields can change, and the row is read
    /// first so unspecified fields keep their value.
    pub fn update_transaction(
        &self,
        id: &str,
        change: TransactionChange,
    ) -> DbResult<Option<Transaction>> {
        let conn = self.conn()?;
        let current: Option<Transaction> = conn
            .query_row(
                &format!("SELECT {TX_COLUMNS} FROM transactions WHERE id = ?1"),
                params![id],
                tx_from_row,
            )
            .optional()?;
        let Some(mut current) = current else {
            return Ok(None);
        };
        if let Some(kind) = change.kind {
            current.kind = kind;
        }
        if let Some(amount) = change.amount {
            current.amount = amount;
        }
        if let Some(category) = change.category {
            current.category = category;
        }
        if let Some(description) = change.description {
            current.description = description;
        }
        if let Some(date) = change.date {
            current.date = date;
        }
        conn.execute(
            "UPDATE transactions SET type = ?1, amount = ?2, category = ?3, description = ?4, date = ?5
             WHERE id = ?6",
            params![
                current.kind.as_str(),
                current.amount,
                current.category,
                current.description,
                current.date,
                id
            ],
        )?;
        Ok(Some(current))
    }

    pub fn get_transactions_by_date(&self, target_date: &str) -> DbResult<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TX_COLUMNS} FROM transactions WHERE date = ?1 ORDER BY created_at DESC"
        ))?;
        let rows = stmt.query_map(params![target_date], tx_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_transactions_by_month(&self, year: i32, month: u32) -> DbResult<Vec<Transaction>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TX_COLUMNS} FROM transactions WHERE date LIKE ?1 ORDER BY date DESC, created_at DESC"
        ))?;
        let prefix = format!("{year:04}-{month:02}%");
        let rows = stmt.query_map(params![prefix], tx_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Sorted `(year, month)` pairs that have transactions, newest first.
    pub fn get_months_with_data(&self) -> DbResult<Vec<(i32, u32)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT substr(date, 1, 7) as ym FROM transactions ORDER BY ym DESC",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            let ym = row?;
            if let (Ok(year), Ok(month)) = (ym[0..4].parse::<i32>(), ym[5..7].parse::<u32>()) {
                out.push((year, month));
            }
        }
        Ok(out)
    }
}

/// The fields `update_transaction` may change; `None` keeps the stored value.
#[derive(Clone, Debug, Default)]
pub struct TransactionChange {
    pub kind: Option<Kind>,
    pub amount: Option<f64>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub date: Option<String>,
}

fn tx_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transaction> {
    Ok(Transaction {
        id: row.get(0)?,
        kind: Kind::parse(&row.get::<_, String>(1)?),
        amount: row.get(2)?,
        category: row.get(3)?,
        description: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        date: row.get(5)?,
        created_at: row.get(6)?,
    })
}

/// Python's `round(x, 2)`. `f64::round` on the scaled value matches for the amounts this app
/// handles (and avoids the binary noise `format!` would introduce).
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn now_iso() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}

fn today() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

/// `2026-10-01` for the first day of a month, and the month's last day, for the calendar.
pub fn month_days(year: i32, month: u32) -> Vec<chrono::NaiveDate> {
    use chrono::NaiveDate;
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let mut days = Vec::with_capacity(31);
    let mut day = first;
    while day.month() == month {
        days.push(day);
        day = day.succ_opt().expect("valid date");
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDb {
        db: Db,
        path: PathBuf,
    }

    impl TempDb {
        fn new(name: &str) -> TempDb {
            let path = std::env::temp_dir().join(format!("mt4-{}-{}.db", name, Uuid::new_v4()));
            let db = Db::at(&path);
            db.init().expect("init");
            TempDb { db, path }
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn a_fresh_database_is_seeded_like_the_python_one() {
        let t = TempDb::new("seed");
        let cats = t.db.load_categories().unwrap();
        assert_eq!(cats.expense.len(), DEFAULT_EXPENSE_CATEGORIES.len());
        assert_eq!(cats.income.len(), DEFAULT_INCOME_CATEGORIES.len());
        // ORDER BY name: both lists come back sorted.
        let mut sorted = cats.expense.clone();
        sorted.sort();
        assert_eq!(cats.expense, sorted);
    }

    #[test]
    fn transactions_round_trip_and_summarise() {
        let t = TempDb::new("transactions");
        let a =
            t.db.add_transaction(
                Kind::Expense,
                10.005,
                "🍔 Food & Dining",
                "cena",
                Some("2026-10-25"),
            )
            .unwrap();
        let b =
            t.db.add_transaction(Kind::Income, 1500.0, "💰 Salary", "", Some("2026-10-01"))
                .unwrap();
        assert_eq!(a.amount, 10.01, "amounts are rounded to two decimals");
        assert!(!a.id.is_empty() && a.id != b.id);

        let summary = t.db.get_monthly_summary(2026, 10).unwrap();
        assert_eq!(summary.total_income, 1500.0);
        assert_eq!(summary.total_expense, 10.01);
        assert_eq!(summary.balance, 1489.99);
        assert_eq!(summary.transaction_count, 2);
        assert_eq!(summary.category_expenses["🍔 Food & Dining"], 10.01);
        assert_eq!(summary.daily["2026-10-25"].1, 10.01);

        let all = t.db.get_all_time_balance().unwrap();
        assert_eq!(all.balance, 1489.99);
        assert_eq!(t.db.get_months_with_data().unwrap(), vec![(2026, 10)]);

        let day = t.db.get_transactions_by_date("2026-10-25").unwrap();
        assert_eq!(day.len(), 1);

        let updated =
            t.db.update_transaction(
                &a.id,
                TransactionChange {
                    amount: Some(12.5),
                    ..Default::default()
                },
            )
            .unwrap()
            .expect("row exists");
        assert_eq!(updated.amount, 12.5);
        assert_eq!(
            updated.category, "🍔 Food & Dining",
            "unspecified fields keep their value"
        );

        assert!(t.db.delete_transaction(&b.id).unwrap());
        assert!(!t.db.delete_transaction(&b.id).unwrap());
        assert_eq!(t.db.load_transactions().unwrap().len(), 1);
    }

    #[test]
    fn budgets_are_replaced_per_year_and_drop_with_their_category() {
        let t = TempDb::new("budgets");
        let mut budgets = HashMap::new();
        budgets.insert("🍔 Food & Dining".to_owned(), 1200.0);
        budgets.insert("🎭 Entertainment".to_owned(), 0.0);
        t.db.save_year_budgets(2026, &budgets).unwrap();

        let loaded = t.db.load_budgets(2026).unwrap();
        assert_eq!(loaded.len(), 1, "zero budgets are not stored");
        assert_eq!(loaded["🍔 Food & Dining"], 1200.0);

        let annual = t.db.get_annual_summary(2026).unwrap();
        assert_eq!(annual.total_expense, 0.0);

        let mut cats = t.db.load_categories().unwrap();
        cats.expense.retain(|c| c != "🍔 Food & Dining");
        t.db.save_categories(&cats).unwrap();
        assert!(
            t.db.load_budgets(2026).unwrap().is_empty(),
            "budgets of deleted categories go"
        );
    }
}

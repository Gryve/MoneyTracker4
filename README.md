# MoneyTracker 4

A personal-finance desktop app written in Rust with [Rune](https://github.com/anomalyco/rune), opencode's native UI toolkit. It reads and writes the same SQLite database as the original Python version, so the two apps can take turns on the same file — no migration, no lock-in.

## Screenshots

| Dashboard | Transactions |
| --- | --- |
| ![Dashboard](assets/dashboard.png) | ![Transactions](assets/transactions.png) |

| Calendar | Budgets | Year overview |
| --- | --- | --- |
| ![Calendar](assets/calendar.png) | ![Budgets](assets/budgets.png) | ![Year overview](assets/year.png) |

*Rendered headlessly against a demo database with fictional data — no personal figures ever leave the machine.*

## Features

- **Dashboard** – monthly income, expenses and balance; annual budget progress; expense/income breakdowns as donuts with a legend.
- **Transactions** – every record of the selected month; click a row to edit it, the bin deletes it.
- **Calendar** – the month grid with each day's totals and the selected day's records.
- **Budgets** – one annual limit per expense category, edited inline and saved with Enter or the check.
- **Year overview** – the twelve months with income, expenses and balance, plus a year total.

Every time-scoped screen has dropdown pickers for **month** and **year** (a single year picker on the annual views) that update the page immediately; the default is always the current month/year, and a *Today* button jumps back to it. The toolbar adds a transaction (<kbd>Ctrl</kbd>+<kbd>N</kbd>) and opens the category manager; <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>B</kbd> shows/hides the sidebar.

## Requirements

- Windows (developed and packaged for Windows)
- Rust — the exact toolchain is pinned in `rust-toolchain.toml` (1.99.1); rustup installs it automatically on first build
- No system dependencies: SQLite ships bundled with `rusqlite`, so there is nothing else to install

## Quick start (development)

```powershell
cargo run            # debug build, opens the project's data\moneytracker.db
cargo run --release  # optimized build
```

The first build compiles Rune from its pinned git revision (`Cargo.toml`) and takes a few minutes; later builds are incremental.

### Where the database lives

1. `MT_DB_FILE` if set;
2. `data/moneytracker.db` beside the executable;
3. for a `cargo run` build (binary under `target/debug|release`), the project's own `data/moneytracker.db`, so development never writes into `target/`.

A missing database is created and seeded on first launch with 22 default categories (14 expense, 8 income).

## Building the release exe

```powershell
.\package.ps1                  # build release, run the tests, refresh dist\
.\package.ps1 -SkipTests       # same without cargo test
.\package.ps1 -ForceDb         # also replace dist\data\moneytracker.db with data\moneytracker.db
.\package.ps1 -DbSource <path> # publish a specific database file instead (implies -ForceDb)
.\package.ps1 -SkipSmokeTest   # do not launch the published binary once
```

`dist\` ends up with `MoneyTracker4.exe` and `data\moneytracker.db` side by side — that is all the app needs: double-click the exe and it opens the database next to it.

If PowerShell refuses to run the script because of the execution policy, call it explicitly:

```powershell
powershell -ExecutionPolicy Bypass -File .\package.ps1
```

The script never overwrites an existing `dist\data\moneytracker.db`, since that one holds live data; on the first run it seeds the folder from the development copy. After copying, it launches the published exe once with `--snap` and checks that it renders a frame, so a broken binary or a missing database does not go unnoticed.

## Database

One SQLite file shared with the Python original — same schema, same SQL strings, same semantics (amounts rounded to two decimals, UUID v4 ids, local ISO timestamps):

| Table | Columns |
| --- | --- |
| `categories` | `id`, `name` (unique), `type` (`income`/`expense`) |
| `transactions` | `id` (UUID), `type`, `amount`, `category`, `description`, `date`, `created_at`; indexed on date, type and category |
| `budgets` | `year`, `category`, `amount` — unique per `(year, category)` |

Categories carry the emoji in their stored name (`"🍔 Food & Dining"`); a few legacy rows have an emoji-less duplicate. The UI groups by display name (emoji stripped) and never rewrites those rows; budgets are stored on one canonical spelling per group.

**Your data stays yours.** `data/*.db` and `dist/` are git-ignored, so the repository contains no personal figures — a fresh clone gets an empty seeded database on first launch.

## Layout of the code

| File | Contents |
| --- | --- |
| `src/db.rs` | A 1:1 port of `data_manager.py`: schema, seed, queries, summaries, budgets. |
| `src/model.rs` | Domain types, money formatting, category display names (emoji stripped). |
| `src/state.rs` | `App`: what the views read and the handlers that change it. |
| `src/ui/` | The window shell and one module per view, plus `widgets.rs` and `dialogs.rs`. |
| `src/icons.rs` | The SVG icon set and the category → icon mapping. |
| `src/main.rs` | `AppSpec`: window, fonts, theme, shortcuts. |

Views are plain functions of `App`; Rune re-renders them every frame and the element handlers mutate the state. Modals use `rune_kit::Dialog`, including focus trapping and Escape.

## Tests

```powershell
cargo test
```

Seven inline unit tests in `src/db.rs` and `src/model.rs`: throwaway-database round trips, fresh-DB seeding, budget lifecycle, money formatting, emoji stripping, legacy-spelling merge and month names. They never touch the live database.

## Headless snapshots

Rune can render a single frame without a window; the app understands a few environment variables so snapshots can show real data (this is also how the screenshots above are produced):

```powershell
$env:MT_DB_FILE="C:\path\to\db"               # which database to open
$env:MT_SNAP_MONTH="2026-10"                  # month to open
$env:MT_SNAP_DAY="2026-10-25"                 # selected calendar day
$env:MT_SNAP_DIALOG="add|categories|edit|delete"
cargo run -- --snap shot.png --theme dark --size 1180x760 --view dashboard
```

`--view` accepts `dashboard`, `transactions`, `calendar`, `budgets`, `year`.

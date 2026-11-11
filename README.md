# MoneyTracker 4

The MoneyTracker desktop app rebuilt in Rust with [Rune](https://github.com/anomalyco/rune), the
native UI toolkit from opencode. It reads and writes the same SQLite database as the Python
version, so the two apps can take turns on the same file.

## Running it

```powershell
cargo run            # debug
cargo run --release  # release
```

The toolchain is pinned in `rust-toolchain.toml` (Rust 1.99.1). The first build compiles Rune from
its pinned git revision (`Cargo.toml`) and takes a few minutes; later builds are incremental.

### Where the database lives

1. `MT_DB_FILE` if set;
2. `data/moneytracker.db` beside the executable;
3. in a `cargo run` build (`target/debug` or `target/release`), the project's own
   `data/moneytracker.db`, so development never writes into `target/`.

The file next to this README is a copy of the real database for development; it is ignored by git
(`data/*.db`).

## Packaging

```powershell
.\package.ps1                  # build release, run the tests, refresh dist\
.\package.ps1 -SkipTests       # same without cargo test
.\package.ps1 -SkipSmokeTest   # do not launch the published binary once
```

`dist\` ends up with `MoneyTracker4.exe` and `data\moneytracker.db` side by side, which is all the
app needs: double-click the exe and it opens the database next to it.

If PowerShell refuses to run the script because of the execution policy, call it explicitly:

```powershell
powershell -ExecutionPolicy Bypass -File .\package.ps1
```

The script never overwrites an existing `dist\data\moneytracker.db`, since that one holds live
data. On the first run it seeds the folder from the development copy; afterwards use
`-ForceDb` to seed it again or `-DbSource <path>` to publish a specific file.

After copying, the script starts the published exe once with `--snap` and checks that it renders a
frame, so a broken binary or a missing database does not go unnoticed. `dist\` is ignored by git.

## What it does

- **Dashboard** – the month's income, expenses and balance, the annual budget progress, and the
  expense/income breakdowns as donuts with a legend.
- **Transactions** – every record of the selected month, click a row to edit it, the bin deletes
  it.
- **Calendar** – the month grid with each day's totals and the selected day's records.
- **Budgets** – one annual limit per expense category, edited inline and saved with Enter or the
  check.
- **Year overview** – the twelve months with income, expenses and balance, plus a year total.

The toolbar adds a transaction (<kbd>Ctrl</kbd>+<kbd>N</kbd>) and opens the category manager;
<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>B</kbd> shows/hides the sidebar. Month-scoped views have
chevrons and a *Today* button.

## Layout of the code

| File | Contents |
| --- | --- |
| `src/db.rs` | A 1:1 port of `data_manager.py`: schema, seed, queries, summaries, budgets. |
| `src/model.rs` | Domain types, money formatting, category display names (emoji stripped). |
| `src/state.rs` | `App`: what the views read and the handlers that change it. |
| `src/ui/` | The window shell and one module per view, plus `widgets.rs` and `dialogs.rs`. |
| `src/icons.rs` | The SVG icon set and the category → icon mapping. |
| `src/main.rs` | `AppSpec`: window, fonts, theme, shortcuts. |

Views are plain functions of `App`; Rune re-renders them every frame and the element handlers
mutate the state. Modals use `rune_kit::Dialog`, including focus trapping and Escape.

Categories in the database carry the emoji the Python seed put in front of their names
(`"🍔 Food & Dining"`), and a few legacy rows have an emoji-less duplicate. The UI groups by the
display name and never rewrites those rows; budgets are stored on one canonical spelling per
group.

## Tests

```powershell
cargo test
```

- `src/db.rs` unit tests build a throwaway database and check the round trips.
- `tests/parity.rs` asserts the aggregates the Python layer reports for the development copy
  (all-time balance, October 2026, the annual summary, the seeded budget).
- `tests/state.rs` drives the handlers against a temp copy: add/edit/delete a transaction,
  validation errors, category editing, and writing a budget for a display name.

## Headless snapshots

Rune can render a single frame without a window. The app understands a few environment variables
so the snapshots can show real data:

```powershell
$env:MT_SNAP_MONTH="2026-10"                  # month to open
$env:MT_SNAP_DAY="2026-10-25"                 # selected calendar day
$env:MT_SNAP_DIALOG="add|categories|edit|delete"
cargo run -- --snap shot.png --theme dark --size 1180x760 --view dashboard
```

`--view` accepts `dashboard`, `transactions`, `calendar`, `budgets`, `year`.

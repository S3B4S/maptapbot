# leaderboard_daily — Optional Date Parameters

## Overview

`/leaderboard_daily` currently always shows *today's* scores (using UTC server time). This spec adds optional `day`, `month`, and `year` parameters so users can look up any past (or current) day's leaderboard.

## Discord timestamp note

Discord message text does **not** automatically communicate the sender's timezone. However, Discord supports inline timestamp formatting (`<t:UNIX:FLAG>`) which renders in each reader's local timezone. For leaderboard dates, however, we intentionally display the **UTC date** the scores were recorded against — so the embed header will append `(UTC)` to the formatted date to make the reference frame explicit, e.g.:

```
Sunday, April 13 (UTC) · 5 players submitted · https://maptap.gg
```

When users look up today's leaderboard (no params), this keeps the existing header format; when they supply a historical date, it makes clear which UTC day is shown.

## Command signature

```
/leaderboard_daily [day] [month] [year]
```

| Parameter | Type    | Required | Default        | Constraints            |
|-----------|---------|----------|----------------|------------------------|
| `day`     | integer | No       | Current UTC day   | 1–31                |
| `month`   | integer | No       | Current UTC month | 1–12               |
| `year`    | integer | No       | Current UTC year  | 2020–2100          |

All three parameters are independent. Any subset may be provided; missing ones fall back to the current UTC value. For example:

- `/leaderboard_daily` → today (UTC)
- `/leaderboard_daily day:13` → the 13th of the current UTC month/year
- `/leaderboard_daily day:13 month:4` → April 13 of the current UTC year
- `/leaderboard_daily day:13 month:4 year:2026` → April 13, 2026

## Validation

- The resolved date must be a **valid calendar date** (e.g. day 31 in a 30-day month is rejected).
- The resolved date must **not be in the future** relative to UTC server time (same rule as the existing score parser; see [setup.md](./setup.md#generic-date-parsing)).
- Out-of-range values (e.g. `month:13`, `day:0`) are rejected with a user-facing error.

On validation failure, the bot replies **ephemerally** (visible only to the invoker) with a descriptive error message.

## Embed changes

The embed description header for the daily leaderboard gains a `(UTC)` suffix on the date portion:

- **Before**: `Sunday, April 13 · 5 players submitted · https://maptap.gg`
- **After**: `Sunday, April 13 (UTC) · 5 players submitted · https://maptap.gg`

This applies to both the summary and full-leaderboard embeds.

## Internal function changes

### `db.rs` — `get_daily_leaderboard` / `get_daily_challenge_leaderboard`

No signature changes required. Both already accept a `date: &str` (`"YYYY-MM-DD"`). The caller (handler) is responsible for building the date string from the optional parameters.

### `embed.rs` — `build_description`

Currently calls `chrono::Utc::now()` internally to derive the display date. This must be changed to accept an explicit `date: NaiveDate` parameter so historical dates render correctly.

```rust
// Before
fn build_description(count: usize, is_permanent: bool, is_challenge: bool) -> String

// After
fn build_description(count: usize, is_permanent: bool, is_challenge: bool, date: Option<chrono::NaiveDate>) -> String
```

- When `date` is `Some(d)`, format `d` as `"Weekday, Month Day (UTC)"`.
- When `date` is `None` (permanent leaderboard), behaviour is unchanged (`"All-time · …"`).
- `build_summary_embed` and `build_full_embed` are updated accordingly to accept and thread through the date.

### `handler.rs` — command registration and dispatch

**Registration** — add three optional integer options to `leaderboard_daily`:

```rust
CreateCommand::new("leaderboard_daily")
    .description("Show a day's leaderboard for this server")
    .add_option(CreateCommandOption::new(CommandOptionType::Integer, "day", "Day (1–31), defaults to today (UTC)").required(false).min_int_value(1).max_int_value(31))
    .add_option(CreateCommandOption::new(CommandOptionType::Integer, "month", "Month (1–12), defaults to current UTC month").required(false).min_int_value(1).max_int_value(12))
    .add_option(CreateCommandOption::new(CommandOptionType::Integer, "year", "Year, defaults to current UTC year").required(false).min_int_value(2020).max_int_value(2100))
```

**Dispatch** — in `build_leaderboard_embed` (or equivalent), read the options, resolve defaults, validate, and build the `"YYYY-MM-DD"` string before the DB call:

```
let now = Utc::now().date_naive();
let day   = options.get("day")   as u32 ?? now.day();
let month = options.get("month") as u32 ?? now.month();
let year  = options.get("year")  as i32 ?? now.year();

let date = NaiveDate::from_ymd_opt(year, month, day)
    .ok_or("Invalid date — check day/month/year values.")?;

if date > now {
    return Err("That date is in the future.".to_string());
}
```

## Files to modify

| File | Change |
|------|--------|
| `src/handler.rs` | Register 3 optional options; extract + validate in dispatch; pass resolved `NaiveDate` to embed builders |
| `src/embed.rs` | `build_description` accepts `Option<NaiveDate>`; add `(UTC)` suffix; thread date through `build_summary_embed` / `build_full_embed` |

## Files unchanged

| File | Reason |
|------|--------|
| `src/db.rs` | Already accepts `date: &str`; no change needed |
| `src/parser.rs` | Date parsing for score ingestion is unrelated |

## Testing

1. `/leaderboard_daily` → shows today's leaderboard (header: `Weekday, Month Day (UTC) · …`)
2. `/leaderboard_daily day:13` → shows the 13th of the current month (or error if future)
3. `/leaderboard_daily day:13 month:4 year:2026` → shows April 13 2026's leaderboard
4. Invalid date (e.g. `day:31 month:2`) → ephemeral error
5. Future date → ephemeral error
6. Date with no scores → existing empty-state message: `"No scores recorded for today yet!"`

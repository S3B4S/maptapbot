# leaderboard_daily — Optional Date Parameter

## Overview

`/leaderboard_daily` currently always shows *today's* scores (using UTC server time). This spec adds an optional `date` string parameter so users can look up any past (or nearby) day's leaderboard by typing a compact date string or a relative keyword.

The previous approach used three separate integer options (`day`, `month`, `year`). These were replaced with a single string argument because Discord's integer pickers are awkward on mobile.

## Discord timestamp note

Discord message text does **not** automatically communicate the sender's timezone. For leaderboard dates we intentionally display the **UTC date** the scores were recorded against — so the embed header appends `(UTC)` to the formatted date, e.g.:

```
Sunday, April 13 (UTC) · 5 players submitted · https://maptap.gg
```

## Command signature

```
/leaderboard_daily [date]
/leaderboard_challenge_daily [date]
```

| Parameter | Type   | Required | Default      |
|-----------|--------|----------|--------------|
| `date`    | string | No       | Today (UTC)  |

Both `leaderboard_daily` and `leaderboard_challenge_daily` receive the same change.

## Supported formats

| Input | Resolves to |
|-------|-------------|
| _(no argument)_ | Today (UTC) |
| `"DD"` | Day DD, current UTC month + year |
| `"DD-MM"` | Day DD, month MM, current UTC year |
| `"DD-MM-YYYY"` | Exact date |
| `"yesterday"` / `"yest"` / `"y"` | Yesterday (UTC) |
| `"tomorrow"` / `"tmro"` / `"t"` | Tomorrow (UTC) |

Missing parts (month, year) default to the current UTC value.

**Examples:**

- `/leaderboard_daily` → today (UTC)
- `/leaderboard_daily date:13` → the 13th of the current UTC month/year
- `/leaderboard_daily date:13-04` → April 13 of the current UTC year
- `/leaderboard_daily date:13-04-2026` → April 13, 2026
- `/leaderboard_daily date:y` → yesterday
- `/leaderboard_daily date:t` → tomorrow

## Validation

- The parsed date must be a **valid calendar date** (e.g. `"31-02"` is rejected).
- An unrecognised string → ephemeral error: `"Unrecognised date format. Try DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, or tomorrow/tmro/t."`
- Future dates more than 1 week out → ephemeral error: `"That date is too far in the future."`

On validation failure the bot replies **ephemerally** (visible only to the invoker).

## Embed changes

The embed description header for the daily leaderboard gains a `(UTC)` suffix on the date portion:

- **Before**: `Sunday, April 13 · 5 players submitted · https://maptap.gg`
- **After**: `Sunday, April 13 (UTC) · 5 players submitted · https://maptap.gg`

This applies to both the summary and full-leaderboard embeds, unchanged from the prior spec.

## Internal function changes

### `handler.rs` — command registration

Replace the three integer options with a single optional string option on both daily commands:

```rust
CreateCommand::new("leaderboard_daily")
    .description("Show a day's leaderboard for this server")
    .add_option(
        CreateCommandOption::new(
            CommandOptionType::String,
            "date",
            "DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, tomorrow/tmro/t — defaults to today",
        )
        .required(false),
    )
```

### `handler.rs` — dispatch

Replace the per-integer extraction + fallback with a single string parser:

```
let today = Utc::now().date_naive();
let raw   = options.get("date");   // Option<&str>

let date = match raw {
    None                                        => today,
    Some("yesterday" | "yest" | "y")            => today - Duration::days(1),
    Some("tomorrow"  | "tmro" | "t")            => today + Duration::days(1),
    Some(s)                                     => parse_date_str(s, today)?,
};

if date > today + Duration::weeks(1) {
    return ephemeral_error("That date is too far in the future.");
}
```

`parse_date_str(s, today)` is a small helper that tries `DD`, `DD-MM`, and `DD-MM-YYYY` in order and fills missing parts from `today`, returning an error on unrecognised input or an invalid calendar date.

### `embed.rs` — no signature changes

`build_description` / `build_summary_embed` / `build_full_embed` already accept `Option<NaiveDate>` — no further changes needed.

### `db.rs` — no changes

Both `get_daily_leaderboard` and `get_daily_challenge_leaderboard` already accept `date: &str` (`"YYYY-MM-DD"`).

## Files to modify

| File | Change |
|------|--------|
| `src/handler.rs` | Replace 3 integer options with 1 string option on both daily commands; replace integer extraction with `parse_date_str` helper |

## Files unchanged

| File | Reason |
|------|--------|
| `src/embed.rs` | Already accepts `Option<NaiveDate>`; `(UTC)` suffix already present |
| `src/db.rs` | Already accepts `date: &str` |
| `src/parser.rs` | Score ingestion date parsing is unrelated |

## Testing

1. `/leaderboard_daily` (no arg) → today's leaderboard
2. `/leaderboard_daily date:13` → 13th of current month
3. `/leaderboard_daily date:13-04` → April 13 of current year
4. `/leaderboard_daily date:13-04-2025` → April 13, 2025
5. `/leaderboard_daily date:y` and `date:yesterday` and `date:yest` → all show yesterday
6. `/leaderboard_daily date:t` and `date:tomorrow` and `date:tmro` → all show tomorrow
7. Invalid string (e.g. `date:abc`) → ephemeral error with format hint
8. Invalid calendar date (e.g. `date:31-02`) → ephemeral error
9. Date > 1 week in future → ephemeral error
10. Date with no scores → existing empty-state: `"No scores recorded for today yet!"`
11. Same cases for `/leaderboard_challenge_daily`

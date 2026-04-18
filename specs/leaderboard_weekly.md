# Weekly Leaderboard

## Command

```
/leaderboard_weekly [week] [scoring]
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `week` | string | no | Week to display (see formats below). Defaults to current ISO week. |
| `scoring` | choice | no | `avg` (default) or `sum` — how to aggregate scores across the week. |

### `week` parameter formats

| Input | Meaning |
|-------|---------|
| omitted | Current ISO week |
| `last` or `l` | Previous ISO week |
| `N` (e.g. `16`) | Week N of the current ISO year |
| `N-YYYY` (e.g. `16-2026`) | Week N of year YYYY |

Invalid week strings → ephemeral error: `"Unrecognised week format. Try a week number (16), N-YYYY (16-2026), or last/l."`

### `scoring` parameter

| Value | Behaviour |
|-------|-----------|
| `avg` (default) | Average final score per player across submitted days; formatted to 1 decimal place |
| `sum` | Sum of final scores per player across submitted days; formatted as integer |

## Behaviour

- Guild-only command. Invoking from a DM → ephemeral: `"This command can only be used in a server."`
- The week is bounded to days that have already occurred: if the target week includes future days, only days up to and including today are included.
- Empty state → ephemeral: `"No scores recorded for that week yet!"`
- On success: posts a **public summary embed**, then sends an ephemeral follow-up with **"Full leaderboard"** and **"Remove"** buttons (same pattern as `/leaderboard_daily`).

## Response format

**Embed — summary view**

| Property | Value |
|---|---|
| Title | `Weekly Leaderboard` (avg) / `Weekly Leaderboard (Sum)` (sum) |
| Color | Gold — `#FFD700` |
| Description | Header line (see below) |
| Field: `Top 3` | Medal entries |
| Field: `Bottom 3` | Skull entries; omitted if total entries ≤ 3 |

**Description (header line)**

`Week {N} of {YYYY} ({Mon d} – {Mon d}[, in progress]) · {N} players · https://maptap.gg`

- Date range uses abbreviated month + day, e.g. `Apr 14 – Apr 20`.
- `, in progress` is appended when the week is the current ISO week (i.e. today falls within it).

**Full leaderboard embed** (posted in a thread, or directly if invoked inside a thread)

- Title: `Weekly Leaderboard — Full` / `Weekly Leaderboard (Sum) — Full`
- Single `description` field listing all entries ranked: `{rank}. {username} — {score}`
- Truncated to Discord's 4096-character embed description limit with `\n... (truncated)` if needed.

## Buttons

Same button pattern as `/leaderboard_daily`: ephemeral follow-up with **"Full leaderboard"** and **"Remove"** buttons. After clicking **"Full leaderboard"**, the message updates to include a **"Remove full leaderboard"** button as well. See [daily_mode.md](./daily_mode.md) for the full button spec.

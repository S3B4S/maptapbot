# Frontier mode

A third MapTap variant played at `https://maptap.gg/frontier`. Unlike daily/challenge, Frontier is an endless-run mode: the player keeps going until they fail, and the message reports how far they got rather than five discrete tile scores.

The message format is:

```
MapTap Frontier
Level <N> · <rounds> rounds · <M>:<SS>
Fell at <location> · <pts> pts
www.maptap.gg/frontier
```

Example:
```
MapTap Frontier
Level 8 · 31 rounds · 3:17
Fell at Kurnool, Andhra Pradesh, India · 2,829 pts
www.maptap.gg/frontier
```

Notes:
- The URL line is **last**, not first (this is the key structural difference from daily and challenge).
- `pts` is comma-formatted in the message (`2,829`); strip commas before parsing.
- `pts` can exceed 1000 — the daily/challenge 1000 cap does not apply.
- `<M>:<SS>` is mm:ss — the run took 3 minutes 17 seconds. Stored as milliseconds.
- `<location>` is free-text, extracted as everything between `Fell at ` and ` · ` on line 3.

## Parsing rules

See [generic message-parsing rules](./setup.md#message-parsing-rules) — text before line 1 and after line 4 is allowed (including on the same line); the four lines themselves cannot be interrupted.

See [generic date parsing](./setup.md#generic-date-parsing) — but **Frontier messages do not contain a date**. The date is derived from the Discord message's `posted_at` timestamp (UTC). No future-date validation applies.

### Field parsing

| Field | Source | Notes |
|---|---|---|
| `level` | `Level <N>` on line 2 | Integer ≥ 1 |
| `rounds` | `<N> rounds` on line 2 | Integer ≥ 0 |
| `time_spent_ms` | `<M>:<SS>` on line 2 | `3:17` → 197000 |
| `location` | between `Fell at ` and ` · ` on line 3 | Free-text, kept verbatim |
| `final_score` | `<N> pts` on line 3 | Strip commas; integer; **no upper cap** |

The ` · ` separator is U+00B7 MIDDLE DOT — use the exact character.

## Database

The `scores` table `mode` column value for this mode is `frontier`.

Frontier rows reuse existing columns:

| Column | Frontier value |
|---|---|
| `mode` | `'frontier'` |
| `score1`–`score5` | `NULL` (no per-tile scores) |
| `final_score` | pts |
| `time_spent_ms` | run duration in ms |
| `date` | derived from `posted_at`, formatted `YYYY-MM-DD` |

Three new nullable columns are added (migration 7), only populated for Frontier rows:

| Column | Type | Description |
|---|---|---|
| `frontier_level` | INTEGER | Level reached |
| `frontier_rounds` | INTEGER | Rounds played |
| `frontier_location` | TEXT | "Fell at" location, free-text |

### Cadence

Frontier runs are not deterministic across users on a given day — every run produces unique data and every valid submission is recorded. There is **no** one-row-per-day partitioning for Frontier:

- A user may submit multiple runs in the same day; all valid submissions are stored as separate rows.
- The `UNIQUE(user_id, guild_id, date, mode)` constraint was already dropped in migration 5, so the append-only model applies naturally.
- The "effective row per (user, guild, date, mode)" logic used by daily/challenge leaderboards is **not** applied to Frontier.

## Validation

`MaptapScore::validate()` is mode-aware:

- For `daily_default` and `daily_challenge`: existing rules unchanged (s1–s5 in 0–100, final ≤ 1000, formula match).
- For `frontier`:
  - `score1`–`score5` MUST be `None`; the per-tile scoring formula is not checked.
  - `final_score` has no upper cap.
  - `frontier_level` MUST be present and ≥ 1.
  - `frontier_rounds` MUST be present and ≥ 0.
  - `frontier_location` MUST be present and non-empty.
  - `time_spent_ms` MUST be present.

## Failure behavior

Same as [setup.md failure behavior](./setup.md#failure-behavior). A message that contains the Frontier block but fails validation reacts with ❌; a valid stored Frontier score reacts with 🗺️.

## Commands

```
/leaderboard_frontier
```

Shows the all-time Frontier leaderboard, scoped to the current guild. **There is no daily or weekly variant** — Frontier is permanent-leaderboard-only.

Ranking: each user contributes their single best run (highest `final_score`). Ties on `final_score` are broken by earliest `posted_at` (the earlier submission wins). Banned users are excluded. Empty state: `"No Frontier scores recorded yet!"`

### Response format

Public Discord embed.

**Embed — summary view**

| Property | Value |
|---|---|
| Title | `Frontier Leaderboard` |
| Color | Deep orange — `#FF6B35` |
| Description | `All-time · <N> players · https://maptap.gg/frontier` |
| Field: `Top 3` | Medal entries (see below) |
| Field: `Bottom 3` | Skull entries (see below); omitted if total entries ≤ 3 |

**Top 3 / Bottom 3 entry format**

```
🥇 alice (2,829 pts · L8 · 31r · 3:17) — Kurnool, Andhra Pradesh, India
```

Format: `<medal> <username> (<pts> pts · L<level> · <rounds>r · <M>:<SS>) — <location>`

- `pts` displayed with thousands separators
- Level prefixed with `L`
- Rounds suffixed with `r`
- Time formatted as `M:SS` (one or two digit minutes, zero-padded seconds)
- Bottom 3 only shown if total entries > 3, no overlap with Top 3.

### Buttons (ephemeral, invoker-only)

Same as [daily mode buttons](./daily_mode.md#buttons-ephemeral-invoker-only): "Full leaderboard" / "Remove" / (after expansion) "Remove full leaderboard".

# Challenge mode

There's another kind of maptap that people can do, the challenge mode. The message format is:

```
⚡ MapTap Challenge Round - <month> <day>
www.maptap.gg/challenge
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Score: <final-score> in <time>s (<spare>s to spare!)
```

Example (completed):
```
⚡ MapTap Challenge Round - Apr 12
www.maptap.gg/challenge
89🎉 82✨ 94🏆 88🎓 97🏅
Score: 914 in 21.1s (4.0s to spare!)
```

Example (timed out — failed tile shows `--`):
```
⚡ MapTap Challenge Round - Apr 13
www.maptap.gg/challenge
96🏅 4🤮 68🙂 91🎉 --
Score: 509 in 25.0s (TIME UP!)
```

## Scores format

See [shared score rules](./setup.md#shared-score-rules) in setup.md.

See [generic date parsing](./setup.md#generic-date-parsing) in setup.md for date validation rules.

A score value is either:
- A digit string (`0`–`100`) — normal score
- `--` — the tile was not completed in time; stored as `NULL` in the DB

Only digits or `--` are valid score values. Anything else is a parse failure.

When calculating averages for leaderboard purposes, `NULL` scores are treated as `0`.

- `time_spent`: integer in milliseconds — `21.1s` => `21100`
- When the time runs out the suffix is `(TIME UP!)` instead of `(X.Xs to spare!)`; both are ignored when storing
- The `(TIME UP!)`/`(X.Xs to spare!)` part can be ignored; it's just `25s - time_spent` and can be derived

## Database

The `scores` table `mode` column value for this mode is `daily_challenge`.

An extra column `time_spent` (integer, milliseconds) is added for this mode.

We need a migration for the DB as it is, as I don't want to destroy the current existing data.

## Commands

```
/leaderboard_challenge_daily
```

Shows today's challenge scores only, scoped to the current guild. Sorted descendingly by total score. Empty state: `"No challenge scores recorded for today yet!"`

```
/leaderboard_challenge_permanent
```

Shows all-time challenge scores, scoped to the current guild. Averages each score column across all entries. Sorted descendingly by total score. Empty state: `"No challenge scores recorded yet!"`

### Response format

Same structure as the [daily mode response format](./daily_mode.md#response-format), with these differences:

**Embed — summary view**

| Property | Value |
|---|---|
| Title | `Daily Challenge Leaderboard` / `Permanent Challenge Leaderboard` |
| Color | Electric blue — `#4A90E2` |
| Description | Header line (see below) |
| Field: `Top 3` | Medal entries with time (see below) |
| Field: `Bottom 3` | Skull entries with time (see below); omitted if total entries ≤ 3 |

**Description (header line)**
- Daily: `<Weekday, Month Day> · <N> players submitted · https://maptap.gg/challenge`
- Permanent: `All-time · <N> players · https://maptap.gg/challenge`

**Fields: Top 3 / Bottom 3**

Challenge entries also display `time_spent` alongside the total score:
```
🥇 alice (914, 21.1s)  🥈 bob (891, 19.4s)  🥉 charlie (876, 22.0s)
```
- Format per entry: `<medal> <username> (<total_score>, <time_spent_s>)`
- `time_spent` formatted as `<seconds_with_1_decimal>s` e.g. `21.1s`
- If `time_spent` is `NULL`, omit it: `<medal> <username> (<total_score>)`

### Buttons (ephemeral, invoker-only)

Same as [daily mode buttons](./daily_mode.md#buttons-ephemeral-invoker-only).

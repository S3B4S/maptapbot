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

### Table format

Same as the [daily mode table](./daily_mode.md#table-format), with one additional column:

| Column | Width | Notes |
|---|---|---|
| `#` | 4, left-aligned | Rank |
| `User` | 20, left-aligned | Truncated to 18 chars + `..` if over limit |
| `S1`–`S5` | 5, right-aligned | Individual scores |
| `Total` | 7, right-aligned | Daily: integer; Permanent: 1 decimal average |
| `Time` | 7, right-aligned | Formatted as `21.1s`; `-` if absent |

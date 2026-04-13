# Daily mode (default)

The default mode played when a user visits `https://maptap.gg`. The message format is:

```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

Example:
```
www.maptap.gg April 13
93🏆 90👑 83😁 61🫢 97🔥
Final score: 823
```

## Scores format

See [shared score rules](./setup.md#shared-score-rules) in setup.md.

See [generic date parsing](./setup.md#generic-date-parsing) in setup.md for date validation rules.

## Database

The `scores` table `mode` column value for this mode is `daily_default`.

`time_spent` is `NULL` for this mode (no timer in daily default).

## Commands

```
/leaderboard_daily
```

Shows today's scores only, scoped to the current guild. Sorted descendingly by total score. Empty state: `"No scores recorded for today yet!"`

```
/leaderboard_permanent
```

Shows all-time scores, scoped to the current guild. Averages each score column across all entries. Sorted descendingly by total score. Empty state: `"No scores recorded yet!"`

### Table format

Both commands render a fixed-width table in a Discord code block:

| Column | Width | Notes |
|---|---|---|
| `#` | 4, left-aligned | Rank |
| `User` | 20, left-aligned | Truncated to 18 chars + `..` if over limit |
| `S1`–`S5` | 5, right-aligned | Individual scores |
| `Total` | 7, right-aligned | Daily: integer; Permanent: 1 decimal average |

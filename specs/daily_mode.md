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

Shows today's scores only (or the date specified via the `date` parameter), scoped to the current guild. Sorted descendingly by total score. Empty state: `"No scores recorded for that day yet!"`

```
/leaderboard_permanent
```

Shows all-time scores, scoped to the current guild. Averages each score column across all entries. Sorted descendingly by total score. Empty state: `"No scores recorded yet!"`

### Response format

Both commands post a **public Discord embed**:

**Embed — summary view**

| Property | Value |
|---|---|
| Title | `Daily Leaderboard` / `Permanent Leaderboard` |
| Color | Gold — `#FFD700` |
| Description | Header line (see below) |
| Field: `Top 3` | Medal entries (see below) |
| Field: `Bottom 3` | Skull entries (see below); omitted if total entries ≤ 3 |

**Description (header line)**
- Daily: `<Weekday, Month Day> · <N> players submitted · https://maptap.gg`
  - e.g. `Tuesday, April 15 · 8 players submitted · https://maptap.gg`
- Permanent: `All-time · <N> players · https://maptap.gg`

**Field: Top 3** (sorted descending, left to right)
```
🥇 alice (823)  🥈 bob (812)  🥉 charlie (799)
```
- Format per entry: `<medal> <username> (<total_score>)`
- If fewer than 3 entries, show only however many exist.

**Field: Bottom 3** (sorted descending, left to right: rank N-2, N-1, N)
```
💀 dave (341)  💀 eve (298)  💀 frank (201)
```
- Only shown if total entries > 3.
- If entries > 3 but < 6, show however many entries are not already in the top 3 (no overlap).

### Buttons (ephemeral, invoker-only)

After posting the public embed, the bot sends a **private ephemeral follow-up** to the command invoker containing two buttons:

- **"Full leaderboard"**: Creates a public thread on the summary message and posts a full embed (same color and title, all entries listed in ranked order) there.
  - **Thread fallback**: If the command was invoked inside a thread (where creating sub-threads is not possible), the full leaderboard embed is posted directly into the same thread instead.
- **"Remove"**: Deletes the public summary embed.

#### After "Full leaderboard" is clicked

The original 2-button ephemeral message is deleted and replaced with a new ephemeral message containing **three** buttons:

- **"Full leaderboard"**: Same as above (posts another full leaderboard).
- **"Remove"**: Same as above (deletes the public summary embed).
- **"Remove full leaderboard"**: Deletes the full leaderboard message that was posted (whether in a newly created thread or directly in an existing thread).

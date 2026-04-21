# /self — Personal Stats

## Command

```
/self
```

No parameters. Stats are scoped to the invoking user across all guilds (scores are stored globally per user, not per server).

## Behaviour

- Available in guilds and DMs.
- Always responds ephemerally — the user sees their stats privately first.
- The embed includes a **"Share"** button to post the stats publicly in the current channel.
- Once shared publicly, a **"Remove"** button lets the user delete the public post.

## Embed format

| Property | Value |
|---|---|
| Author | User's Discord avatar + username |
| Title | `🗺️ Your MapTap Stats` |
| Color | Indigo — `#5865F2` |
| Fields | See below |
| Footer | A rotating flavour line (see below) |

### Fields

| Emoji | Field name | Content |
|---|---|---|
| 🎯 | **Scores submitted** | `{N} daily · {N} challenge · {N} total` |
| 💯 | **Perfect 100s** | `{N} tiles scored 100 ({pct}% of all tiles)` |
| 😬 | **Zero tiles** | `{N} tiles scored 0 ({pct}% of all tiles)` |
| 🔥 | **Current streak** | `{N} days in a row` — or `No active streak` if they missed yesterday |
| 🏆 | **Best streak** | `{N} days` |
| ⭐ | **Average score** | `{avg} daily · {avg} challenge` (1 decimal place each; omit a mode if no scores) |
| 🚀 | **Personal best** | `{score} on {Month Day, YYYY}` (highest ever final score, daily mode) |
| 📅 | **Playing since** | `{Month Day, YYYY}` (date of their very first recorded score) |
| 🏅 | **Server rank** | `#{rank} of {total} on the permanent leaderboard` (guild-only; omitted in DMs) |

#### Streak definition

A streak is the number of consecutive calendar days (UTC) on which the user has at least one valid score recorded, ending on either today or yesterday. The streak is considered active if the most recent score was on today or yesterday. Scores from any mode count toward the streak.

#### Server rank

Only included when the command is used inside a guild. Rank is based on the permanent daily leaderboard for that guild (average daily score, same ordering as `/leaderboard_permanent`). Shows `Not ranked yet` if the user has no daily scores in this guild.

### Footer flavour lines (rotate randomly)

- `Keep tapping those maps! 🌍`
- `Every tile is a new opportunity. 🗺️`
- `The world won't map itself. 📍`
- `Geography nerd? Absolutely. 🧭`
- `One day you'll get that 1000. 💪`

## Button flow

**Step 1 — ephemeral response:**

Bot replies with the stats embed (ephemeral) and a single button:

- **"Share"** — posts the embed publicly in the current channel.

**Step 2 — after "Share" is clicked:**

- The public embed is posted.
- The ephemeral message updates to show a single button:
  - **"Remove"** — deletes the public post.

**Step 3 — after "Remove" is clicked:**

- The public post is deleted.
- The ephemeral updates to plain text: `"Stats removed."`

## Empty state

If the user has no recorded scores at all, the bot responds ephemerally:

> `You haven't submitted any scores yet! Play at https://maptap.gg and share your results here.`

No embed, no buttons.

## Ideas for future additions

- **Percentile rank** — "You're in the top 12% of players this week."
- **Heatmap by day-of-week** — which days they play most often (e.g. `Mon ▓▓▓ Tue ░ Wed ▓▓ …`)
- **Score trend** — "📈 Your last 7 scores average {N}, up from {N} the week before."
- **Challenge mode stats block** — separate section with avg time, fastest round, most timed-out tiles.
- **Head-to-head record** — optional `/self vs @user` variant comparing two players' averages.
- **Milestone callouts** — "🎉 You just hit 100 scores submitted!" surfaced in the footer.

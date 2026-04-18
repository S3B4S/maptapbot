# RF maptap leaderboard

## Description
We are creating a discord bot that will scan messages from RF, #random channel.

It needs to activate if it parses a recognized message format. See individual mode specs for exact formats:
- [Daily mode (default)](./daily_mode.md)
- [Challenge mode](./challenge_mode.md)

## Message parsing rules

A parseable message must contain the recognized pattern for a given mode, but it does _not_ need to be the sole text in the message.

Text before it is allowed:
```
this is horrible
<parseable message line 1>
<parseable message line 2>
<parseable message line 3>
```

Even on the same line as the first line:
```
this is horrible <parseable message line 1>
<parseable message line 2>
<parseable message line 3>
```

Text after it is allowed:
```
<parseable message line 1>
<parseable message line 2>
<parseable message line 3>
this is amazing
```

And also on the same line as the last line:
```
<parseable message line 1>
<parseable message line 2>
<parseable message line 3> this is amazing
```

But the lines of the parseable message cannot be interrupted. These are all invalid:
```
<parseable message line 1> this sucks
<parseable message line 2>
<parseable message line 3>
```

```
<parseable message line 1>
<parseable message line 2> wow I did so well today
<parseable message line 3>
```

```
<parseable message line 1>
nahh I'm embarrassed <parseable message line 2>
<parseable message line 3>
```

## Shared score rules

These apply to all modes:

- Each individual `score` must be between `0-100` (inclusive both ends)
- `final-score` cannot exceed `1000`
- Final score is calculated as: `(s1 + s2) * 1 + s3 * 2 + (s4 + s5) * 3`
- The reported `final-score` in the message must match this formula exactly

## Generic date parsing

- The date in the message cannot be more than 1 day in the future; it must be today, tomorrow, or an earlier date (tomorrow is allowed to accommodate users in timezones ahead of the server)
- If a date more than 1 day in the future is detected, treat it as a validation failure with reason: `"Date cannot be in the future"`

## Failure behavior

| Scenario | Bot action |
|---|---|
| Message contains no recognizable maptap block | Silent — no reply, no reaction |
| Maptap block found but validation fails | Reacts with ❌ emoji; no reply |
| Valid score but DB save fails | Replies: `"Internal error saving your score."` |
| Valid score saved successfully | Reacts with 🗺️ emoji, no reply |

## Database

Through discord you can get an ID for an user. Use this ID to match the user to a score they posted.
A user can only post 1 score for each day. If an user posts multiple (valid) scores for 1 day, assume that the latest post wins and overwrites the previous score.

The guild, channel, and message ID of the source Discord message are stored per score. This enables direct linking to the original message via:
`https://discord.com/channels/{guild_id}/{channel_id}/{message_id}`

2 tables, 1 for scores, and 1 for user info

Scores:
- `message_id` (Discord message snowflake)
- `channel_id` (Discord channel snowflake)
- `user_id`
- `guild_id`
- `date`
- `mode` (e.g. `daily_default`, `daily_challenge`)
- `time_spent_ms` (milliseconds, challenge mode only)
- `score1`
- `score2`
- `score3`
- `score4`
- `score5`
- `final_score`
- `raw_message` (parsed & sanitized)
- `created_at`

Key is `message_id`

UNIQUE constraint on (`user_id`, `guild_id`, `date`, `mode`) — enforces the "one score per user per guild per day per mode" rule. On conflict (upsert), the latest post wins and overwrites the existing row (including its `message_id` and `channel_id`).

Users:
- `user_id`
- `username`

Key is `user_id`

Let's start by just storing these scores in a small local DB.

## Tech stack
- Rust
- Discord API library in Rust: https://github.com/serenity-rs/serenity
- SQLite

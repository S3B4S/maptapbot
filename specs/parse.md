# `/parse` Command

Admin-only command that re-processes an existing Discord message through the normal score pipeline — as if the bot had just seen it for the first time. Useful for recovering scores that were missed due to downtime or bugs.

## Command

```
/parse channel_id:<id> message_id:<id>
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `channel_id` | string (integer) | yes | Discord channel ID where the message lives |
| `message_id` | string (integer) | yes | Discord message ID to parse |

Response is always ephemeral.

## Behaviour

1. Fetch the specified message from Discord via the REST API.
2. Apply the channel allowlist check (same as live message processing). If `DISCORD_FILTER_CHANNEL_IDS` is set and the channel is not in the allowlist (and not a thread of an allowed parent), the command is rejected.
3. Run the message content through the standard parsers (`parse_maptap_message`, then `parse_challenge_message`).
4. If a valid score is found, upsert it into the database exactly as a live message would be — subject to the same `UNIQUE(user_id, guild_id, date, mode)` constraint, meaning the new entry replaces any existing one for the same user/guild/date/mode combination.
5. Reply with the outcome.

## This is NOT a backdoor

All existing rules apply without exception:

- **Channel filter**: the message must be in an allowed channel.
- **Date validation**: the date in the score must not be in the future (beyond the ±2-day tolerance).
- **Score validation**: individual scores 0–100, final score ≤ 1000, formula match.
- **Deduplication**: standard upsert — latest submission wins for a given user/guild/date/mode tuple.

There is no priority flag, no timestamp override, and no way to bypass any guard.

## Responses

| Situation | Response |
|-----------|----------|
| Score saved successfully | `Score processed successfully (final score: <N>).` |
| Message doesn't match any maptap format | `No maptap score found in that message.` |
| Score format is invalid | `Failed to process score: <reason>` |
| Channel not in allowlist | `Channel <id> is not in the allowed list.` |
| Message cannot be fetched | `Could not fetch message <id> in channel <id>: <reason>` |
| Invalid numeric ID provided | `Invalid channel_id / message_id ...: must be a numeric ID.` |
| Invoked by non-admin | `You do not have permission to use this command.` |

## Access

Admin-only. Registered as a guild-specific slash command on the guild set by `DISCORD_ADMIN_GUILD_ID`. See [admin.md](./admin.md) for how admin access is configured.

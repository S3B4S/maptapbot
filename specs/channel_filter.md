# Channel Filter

## Description

The bot can be restricted to only parse and record maptap scores from a specific set of Discord channels. This is configured via an environment variable.

## Configuration

Set `DISCORD_CHANNEL_IDS` in `.env` to a comma-separated list of Discord channel snowflake IDs:

```
DISCORD_CHANNEL_IDS=123456789012345678,987654321098765432
```

If `DISCORD_CHANNEL_IDS` is unset or empty, the bot processes messages from **all channels** in all guilds (existing behavior).

## Behavior

| `DISCORD_CHANNEL_IDS` value | Behavior |
|---|---|
| Unset or empty | All channels processed (no filter) |
| One or more valid IDs | Only messages from listed channels are parsed |
| Invalid IDs (non-numeric) | Silently skipped; only valid IDs in the list are used |

Messages received from channels not in the allowlist are silently ignored — no reply, no reaction.

## Notes

- Discord does not support server-side channel subscriptions via the Gateway. Filtering is done client-side immediately after the `MESSAGE_CREATE` event is received, before any parsing occurs.
- The filter applies only to the score-parsing `message` handler. Slash commands (`/leaderboard_*`, `/today`) are unaffected and work from any channel.
- Channel IDs are parsed once at startup and held in memory. Restarting the bot is required to pick up changes to `DISCORD_CHANNEL_IDS`.

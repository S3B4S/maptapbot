# Admin interface

## Admin detection

The bot reads `DISCORD_ADMIN_USER_IDS` from the environment on startup (comma-separated `u64` user IDs).
A user is an admin if their Discord user ID is in this list.

## Commands

Admin commands are guild-specific slash commands registered on the guild set by `DISCORD_ADMIN_GUILD_ID`.
If a non-admin invokes an admin command, the bot replies with: `"You do not have permission to use this command."`

| Command | Description |
|---|---|
| `/delete_score <message_id>` | Hard-delete a specific score entry by its Discord message ID |
| `/invalidate_score <message_id>` | Soft-delete a score (sets `invalid = 1`); prior valid score (if any) becomes the effective row |
| `/list_scores <user_id>` | Show all scores for a given user across all dates and modes |
| `/list_all_scores` | Dump the full contents of the `scores` table |
| `/list_users` | List all users known to the bot |
| `/raw_score <message_id>` | Show the raw stored message for a score entry by its Discord message ID |
| `/stats` | Show aggregate DB stats (total entries, invalidated, unique users, date range, per-mode counts); shows delta since last invocation per admin user |
| `/backup` | Create a timestamped backup of the database file |
| `/hit_list <action> [user_id]` | Manage the hit list (`action`: `read`, `add`, `delete`) |
| `/parse <channel_id> <message_id>` | Re-process an existing Discord message through the score pipeline |
| `/sync_to_postgres` | Copy all SQLite data to PostgreSQL (requires `POSTGRES_URL`; SQLite wins on conflicts) |

## Parameters

- `<message_id>` — Discord message snowflake ID (string of digits)
- `<user_id>` — Discord user ID (string of digits)
- `<channel_id>` — Discord channel snowflake ID (string of digits)

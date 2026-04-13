# Admin interface

## Admin detection

The bot reads `ADMIN_IDS` from the environment on startup (comma-separated `u64` user IDs).
A user is an admin if their Discord user ID is in this list.

## Commands

Admin commands are slash commands available in DMs with the bot.
If a non-admin invokes an admin command, the bot replies with: `"You do not have permission to use this command."`

| Command | Description |
|---|---|
| `/delete_score <user_id> <date> <mode>` | Delete a specific score entry |
| `/list_scores <user_id>` | Show all scores for a given user across all dates and modes |
| `/list_all_scores` | Dump the full contents of the `scores` table |
| `/list_users` | List all users known to the bot |
| `/raw_score <user_id> <date> <mode>` | Show the raw stored message for a score entry |
| `/clear_day <date>` | Wipe all scores for a given date |
| `/stats` | Show aggregate DB stats (total entries, unique users, date range, per-mode counts) |

## Parameters

- `<user_id>` — Discord user ID (integer)
- `<date>` — ISO format: `YYYY-MM-DD`
- `<mode>` — one of `daily_default` or `daily_challenge`

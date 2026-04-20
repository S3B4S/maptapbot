# MapTapBot

A Discord bot for tracking and leaderboarding game scores. Automatically parses game messages, stores scores in a local SQLite database, and provides real-time leaderboards.

![Overview](images/bot-in-action.png)

## Features

- **Automatic Score Parsing**: Monitors Discord messages for score submissions in multiple formats
- **Score Tracking**: Stores all scores with player names, dates, modes, and scores
- **Leaderboards**: Generate leaderboards for all time, monthly, weekly, and daily rankings
- **Admin Commands**: Delete scores, list user history, and manage the database
- **Channel Filtering**: Optional allowlist to process messages only from specific channels
- **Admin Access Control**: Restrict admin commands to designated Discord users

## Quick Start

### Prerequisites

- Rust 1.70+
- A Discord bot token (create one at [Discord Developer Portal](https://discord.com/developers/applications))

### Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/maptapbot.git
cd maptapbot
```

2. Create a `.env` file in the project root:
```env
DISCORD_TOKEN=your_bot_token_here
DATABASE_PATH=maptap.db
DISCORD_FILTER_CHANNEL_IDS=123456789,987654321
DISCORD_ADMIN_USER_IDS=111111111,222222222
DISCORD_ADMIN_GUILD_ID=222222222222
```

3. Build and run:
```bash
cargo build --release
cargo run --release
```

The bot will start and connect to Discord.

## Configuration

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | Yes | Your Discord bot token |
| `DATABASE_PATH` | No | Path to SQLite database (default: `maptap.db`) |
| `DISCORD_FILTER_CHANNEL_IDS` | No | Comma-separated channel IDs to monitor. If not set, monitors all channels |
| `DISCORD_ADMIN_USER_IDS` | No | Comma-separated Discord user IDs with admin privileges |
| `DISCORD_ADMIN_GUILD_ID` | No | Guild where admin-only commands are registered. If not set, admin commands won't appear anywhere |
| `DISCORD_LOGGING_CHANNEL_ID` | No | Channel ID to receive bot startup log messages |
| `POSTGRES_URL` | No | Enables `/sync_to_postgres` command to copy data to PostgreSQL |

### Example Configuration

```env
# Required
DISCORD_TOKEN=MzU0NjUyMzQyNDEyMzQ1MjM0.DPEUMg.abcdefghijklmnopqrstuvwxyz

# Optional - Restrict to specific channels
DISCORD_FILTER_CHANNEL_IDS=1234567890,0987654321

# Optional - Grant admin access (user IDs) and set the guild for admin commands
DISCORD_ADMIN_USER_IDS=123456789,987654321
DISCORD_ADMIN_GUILD_ID=111122223333

# Optional - Custom database location
DATABASE_PATH=/var/lib/maptap/scores.db
```

## Usage

### Automatic Score Parsing

The bot automatically monitors messages in Discord and parses score submissions. Just send your scores in the channel and the bot will track them!

![Message Parsing](images/message-parsing.png)

The bot recognizes two formats shared directly from [maptap.gg](https://maptap.gg):

**Daily (default) format:**
```
www.maptap.gg April 12
89🎉 82✨ 94🏆 88🎓 97🏅
Final score: 450
```

**Challenge format:**
```
⚡ MapTap Challenge Round - Apr 12
www.maptap.gg/challenge
89🎉 82✨ 94🏆 88🎓 97🏅
Score: 914 in 21.1s (4.0s to spare!)
```

Just paste your results into the monitored channel and the bot reacts with 🗺️ to confirm the score was recorded.

### Commands

#### Leaderboard Commands

```
/today                          — Get a link to today's maptap challenge
/leaderboard_daily              — Show today's scores for this server
/leaderboard_permanent          — Show all-time average scores for this server
/leaderboard_challenge_daily    — Show today's challenge scores for this server
/leaderboard_challenge_permanent — Show all-time challenge averages for this server
/help                           — Show available commands
```

![Leaderboard Display](images/bot-in-action.png)

#### Admin Commands

Available only to users in `DISCORD_ADMIN_USER_IDS`. These commands are registered exclusively on the `DISCORD_ADMIN_GUILD_ID` server and are invisible elsewhere.

| Command | Description |
|---------|-------------|
| `/delete_score <message_id>` | Hard-delete a score entry by Discord message ID |
| `/invalidate_score <message_id>` | Soft-delete a score entry — excluded from leaderboards, but the prior valid score for that user/date/mode becomes effective |
| `/list_scores <user_id>` | Show all score history for a user |
| `/list_all_scores` | Dump the full scores table |
| `/list_users` | List all known users |
| `/raw_score <message_id>` | Show the raw stored message for a score entry |
| `/stats` | Show aggregate DB stats with delta since your last `/stats` call |
| `/backup` | Create a timestamped backup of the SQLite database |
| `/hit_list <action> [user_id]` | Manage the hit list (`read`, `add`, `delete`) |
| `/parse <channel_id> <message_id>` | Re-process an existing Discord message through the score pipeline |
| `/sync_to_postgres` | Copy all SQLite data to PostgreSQL (requires `POSTGRES_URL`) |

![Admin Commands](images/commands.png)

## Architecture

### Core Components

- **Handler** (`src/handler.rs`): Processes Discord events, routes interactions to plugins
- **Plugin system** (`src/plugin.rs`, `src/plugins/`): Self-contained feature modules; each plugin registers its own commands and handles its own interactions. Admin plugins set `is_admin_plugin() = true` for automatic gating and guild-specific registration
- **Repository** (`src/repository.rs`): Trait abstracting all data access — plugins only interact with storage through this interface
- **Parser** (`src/parser.rs`): Extracts scores from message content
- **Database** (`src/db.rs`): SQLite interface for score storage
- **Models** (`src/models.rs`): Data structures for scores and users

### Database Schema

The bot uses SQLite to store:
- User information (Discord ID, username)
- Score records (player, score, date, mode)
- Timestamps for leaderboard calculations

## Deployment

### Docker

A Dockerfile is included for containerized deployment:

```bash
docker build -t maptapbot .
docker run --env-file .env maptapbot
```

### Production

For production deployments:

1. Use environment variable secrets management (not `.env` files)
2. Mount a persistent volume for the database
3. Configure proper Discord intents and permissions
4. Use a process manager (systemd, supervisor) for reliability

## Development

### Project Structure

```
maptapbot/
├── src/
│   ├── main.rs             # Entry point, plugin instantiation
│   ├── handler.rs          # Discord event handler, plugin dispatch
│   ├── plugin.rs           # Plugin trait definition
│   ├── plugins/
│   │   ├── admin_plugin/   # Admin commands (guild-only, gated)
│   │   ├── leaderboard_plugin/ # Leaderboard commands + buttons
│   │   └── self_plugin/    # /self personal stats
│   ├── repository.rs       # Repository trait (all data access)
│   ├── sqlite_repo.rs      # SQLite implementation of Repository
│   ├── parser.rs           # Score message parsing logic
│   ├── db.rs               # SQLite database operations
│   ├── models.rs           # Data structures
│   └── tests/              # Unit tests
├── specs/                  # Spec files (source of truth for behavior)
├── Cargo.toml              # Rust dependencies
├── Dockerfile              # Container configuration
└── README.md               # This file
```

### Running Tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Submit a pull request

## Troubleshooting

### Bot doesn't respond to messages

- Verify `DISCORD_TOKEN` is correct
- Check bot permissions in Discord server settings
- Ensure the bot has "Message Content" intent enabled

### Scores not being tracked

- Check that channels are not filtered, or message is in an allowed channel
- Verify message format matches parser patterns
- Check bot logs for parsing errors

### Database errors

- Ensure `DATABASE_PATH` location is writable
- Check for file permissions
- SQLite is bundled into the binary — no separate installation required

## License

MIT License - see LICENSE file for details

## Support

For issues and questions:
- Open an issue on GitHub
- Check existing documentation in `/specs`

---

**Last Updated**: April 2026

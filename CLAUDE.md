# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # development build
cargo build --release        # optimized build
cargo run --release          # run bot (requires .env or env vars)
cargo test                   # run all tests
cargo test <test_name>       # run a single test
docker build -t maptapbot .  # build Docker image
```

## Spec-first development

This is a spec-first project. The files in `specs/` are the source of truth for intended behavior — implementation must conform to them, not the other way around. Before changing any feature area, read the relevant spec. When behavior is ambiguous or there's a conflict between the spec and the code, the spec wins.

| Spec | Covers |
|------|--------|
| `specs/parse.md` | General message parsing rules |
| `specs/daily_mode.md` | Daily score format and validation |
| `specs/challenge_mode.md` | Challenge score format and validation |
| `specs/admin.md` | Admin commands behavior |
| `specs/leaderboard_daily_date_params.md` | Date parameter handling for leaderboards |
| `specs/leaderboard_weekly.md` | `/leaderboard_weekly` command — week/scoring params, embed format, buttons |
| `specs/channel_filter.md` | Channel allowlist behavior |
| `specs/reduce_noise.md` | When/how the bot suppresses output |
| `specs/todays_challenge.md` | `/today` command behavior |
| `specs/help.md` | `/help` command behavior |
| `specs/setup.md` | Bot setup and configuration |

## Architecture

MapTapBot is a Rust Discord bot that parses game scores from map.gg, stores them in SQLite, and surfaces leaderboards via slash commands. Optionally syncs to PostgreSQL.

### Data flow

1. Discord message arrives → `handler.rs::message()`
2. `parser.rs` extracts scores using pattern matching on 3 consecutive lines
3. `models.rs::MaptapScore` validates constraints (scores 0–100, formula check)
4. `db.rs` upserts into SQLite (dedup key: user/guild/date/mode; newer message_id wins)
5. Bot reacts with emoji indicating success/rank
6. Slash commands in `handler.rs::interaction_create()` query DB and return formatted embeds

### Key modules

| Module | Role |
|--------|------|
| `main.rs` | Entry point: load env, init DB, start serenity client |
| `handler.rs` | Event handler: message parsing dispatch, slash command routing, leaderboard state tracking |
| `parser.rs` | Parses daily and challenge score formats from raw Discord message text |
| `models.rs` | `GameMode` enum and `MaptapScore` struct with validation |
| `db.rs` | SQLite schema, migrations (5+), leaderboard queries, admin operations |
| `admin.rs` | Admin-only slash command handlers |
| `embed.rs` | Discord embed formatting for daily/weekly/permanent leaderboards |
| `pg_db.rs` | PostgreSQL sync (optional, enabled by `POSTGRES_URL`) |

### State in handler

`Handler` holds three pieces of shared state:
- `db: Mutex<Database>` — single SQLite connection
- `leaderboard_msgs` — maps `(guild_id, command_name)` → last posted leaderboard message (for the Remove button; only the invoker can remove)
- `full_leaderboard_msgs` — maps `(guild_id, command_name)` → full leaderboard thread post

### Score parsing

Two recognized formats:
- **Daily**: `www.maptap.gg <Month Day>` followed by 5 score lines, then `Final score: NNN`
- **Challenge**: `⚡ MapTap Challenge Round - <Month Day>` / `www.maptap.gg/challenge` followed by 5 score lines, then `Score: NNN in X.Xs`

Scores satisfy: `(s1+s2)*1 + s3*2 + (s4+s5)*3 = final_score`. Challenge mode allows `--` (timed-out) tiles.

### Database design

- Append-only: each `message_id` is PK; on duplicate (user/guild/date/mode), newer row replaces older via upsert
- Soft-delete via `invalid=1` flag (`/invalidate_score`); hard-delete also available (`/delete_score`)
- `stats_snapshots` table enables delta reporting between `/stats` calls

### Environment variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | Yes | Bot token |
| `DATABASE_PATH` | No | SQLite path (default: `maptap.db`) |
| `DISCORD_FILTER_CHANNEL_IDS` | No | Comma-separated allowlist; if absent, all channels parsed |
| `DISCORD_ADMIN_USER_IDS` | No | Comma-separated Discord user IDs with admin access |
| `DISCORD_ADMIN_GUILD_ID` | No | Guild where admin slash commands are registered |
| `DISCORD_LOGGING_CHANNEL_ID` | No | Channel to receive bot log events |
| `POSTGRES_URL` | No | Enables PostgreSQL sync via `/sync_to_postgres` |

### Testing

Tests live in `src/tests/` covering parser edge cases, DB operations, and model validation. Tests use in-memory SQLite (`:memory:`).

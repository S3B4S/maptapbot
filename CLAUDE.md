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

Plugin-local specs live alongside the plugin (e.g. `src/plugins/self_plugin/self.spec.md`). Global specs live in `specs/`.

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
| `src/plugins/self_plugin/self.spec.md` | `/self` command behavior |

## Architecture

MapTapBot is a Rust Discord bot that parses game scores from map.gg, stores them in SQLite, and surfaces leaderboards via slash commands. Optionally syncs to PostgreSQL.

### Data flow

1. Discord message arrives → `handler.rs::message()`
2. `parser.rs` extracts scores using pattern matching on 3 consecutive lines
3. `models.rs::MaptapScore` validates constraints (scores 0–100, formula check)
4. `db.rs` upserts into SQLite (dedup key: user/guild/date/mode; newer message_id wins)
5. Bot reacts with emoji indicating success/rank
6. Slash commands in `handler.rs::interaction_create()` dispatch to plugins first, then fall through to legacy handlers

### Key modules

| Module | Role |
|--------|------|
| `main.rs` | Entry point: load env, init DB, instantiate plugins, start serenity client |
| `handler.rs` | Event handler: message parsing dispatch, plugin routing, legacy command fallback |
| `plugin.rs` | `Plugin` trait definition — the interface all plugins implement |
| `plugins/` | Plugin implementations (see Plugin system below) |
| `repository.rs` | `Repository` trait — read-only DB interface passed to plugins |
| `sqlite_repo.rs` | `SqliteRepository` — wraps `Mutex<Database>` to implement `Repository` |
| `parser.rs` | Parses daily and challenge score formats from raw Discord message text |
| `models.rs` | `GameMode` enum and `MaptapScore` struct with validation |
| `db.rs` | SQLite schema, migrations (5+), leaderboard queries, admin operations |
| `admin.rs` | Admin-only slash command handlers (**legacy** — to be migrated to a plugin) |
| `embed.rs` | Discord embed formatting for daily/weekly/permanent leaderboards |
| `pg_db.rs` | PostgreSQL sync (optional, enabled by `POSTGRES_URL`) |

---

## Plugin system

**Plugins are the canonical way to add features.** All new commands — including admin commands — should be implemented as plugins. Existing code in `admin.rs`, `help.rs`, and the hardcoded `today` / `parse` / `sync_to_postgres` handlers in `handler.rs` is legacy and should migrate to plugins over time.

### The `Plugin` trait (`src/plugin.rs`)

```rust
pub trait Plugin: Send + Sync {
    fn commands(&self) -> Vec<PluginCommand>;           // slash commands to register + dispatch
    async fn handle_command(&self, ctx, cmd, repo);     // called when a matching command fires
    fn component_prefixes(&self) -> Vec<&'static str>;  // button custom_id prefixes (default: none)
    async fn handle_component(&self, ctx, interaction, repo); // button handler (default: no-op)
}
```

`PluginCommand` pairs a `&'static str` name with a `CreateCommand` definition. The name is used for O(n) dispatch in `handler.rs` — it must exactly match the Discord command name.

### How plugins are wired up

1. **Implement** `Plugin` for a struct in `src/plugins/<name>_plugin/mod.rs`.
2. **Register** it in `src/plugins/mod.rs` (`pub mod <name>_plugin;`).
3. **Instantiate** it in `main.rs` and push it into the `plugins: Vec<Box<dyn Plugin>>` passed to `Handler::new()`.

`handler.rs::ready()` collects `plugin.commands()` from all plugins and registers them as global slash commands. Admin-guild-specific registration is still handled separately for legacy admin commands, but new plugins can handle guild-scoped logic themselves if needed.

### Existing plugins

| Plugin | Location | Commands | Notes |
|--------|----------|----------|-------|
| `SelfPlugin` | `src/plugins/self_plugin/` | `/self` | Personal stats embed, ephemeral |
| `LeaderboardPlugin` | `src/plugins/leaderboard_plugin/` | `/leaderboard_daily`, `/leaderboard_weekly`, `/leaderboard_permanent`, `/leaderboard_challenge_daily`, `/leaderboard_challenge_permanent` | Manages leaderboard message tracking + button handlers (`full_lb`, `remove_lb`, `remove_full_lb`) |

### Plugin state

Plugins own their own state. `LeaderboardPlugin` holds `leaderboard_msgs` and `full_leaderboard_msgs` internally (both `std::sync::Mutex<HashMap<...>>`). The `Handler` struct no longer owns these.

### Repository vs Database

Plugins receive a `&dyn Repository` (read-only leaderboard/score queries). Admin operations that need write access (delete, invalidate, backup, etc.) use `&Mutex<Database>` directly — currently only the legacy `admin.rs` path does this. A future admin plugin would need to extend `Repository` or receive a write-capable handle.

---

### Legacy / not-yet-migrated

These are still hardcoded in `handler.rs` or live in standalone modules:

| Feature | Location | Status |
|---------|----------|--------|
| `/today` | `handler.rs` inline | Legacy builtin |
| `/help` | `handler.rs` + `help.rs` | Legacy builtin |
| Admin commands (`/delete_score`, `/invalidate_score`, `/list_scores`, `/list_all_scores`, `/raw_score`, `/stats`, `/backup`, `/hit_list`, `/list_users`) | `admin.rs` + `handler.rs` | Legacy — migrate to an admin plugin |
| `/parse` | `handler_parse.rs` | Legacy admin command |
| `/sync_to_postgres` | `handler_pg_sync.rs` | Legacy admin command |

---

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

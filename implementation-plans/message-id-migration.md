# Migration: message_id as primary key + backfill

## Overview

Add `message_id` and `channel_id` columns to the `scores` table. Switch the primary key from the composite `(user_id, guild_id, date, mode)` to `message_id`. Backfill existing rows by scanning Discord channel history.

Two phases:
1. **Schema migration** — runs automatically on startup (like existing migrations)
2. **Backfill command** — admin-triggered, fills in real Discord IDs for legacy rows

---

## Phase 1: Schema migration (Migration 3)

### Detection

Check if the `message_id` column exists on the `scores` table:

```sql
SELECT COUNT(*) FROM pragma_table_info('scores') WHERE name = 'message_id'
```

If count is 0, run the migration.

### Migration SQL

```sql
BEGIN;

CREATE TABLE scores_new (
    message_id    TEXT PRIMARY KEY,
    channel_id    TEXT,
    user_id       TEXT NOT NULL,
    guild_id      TEXT,
    date          TEXT NOT NULL,
    mode          TEXT NOT NULL DEFAULT 'daily_default',
    time_spent_ms INTEGER,
    score1        INTEGER,
    score2        INTEGER,
    score3        INTEGER,
    score4        INTEGER,
    score5        INTEGER,
    final_score   INTEGER NOT NULL,
    raw_message   TEXT,
    created_at    TEXT DEFAULT (datetime('now')),
    UNIQUE (user_id, guild_id, date, mode),
    FOREIGN KEY (user_id) REFERENCES users(user_id)
);

-- Existing rows get a synthetic message_id since PK cannot be NULL.
-- Format: "legacy-<hex>" where hex = hex(user_id || '|' || guild_id || '|' || date || '|' || mode)
-- These are deterministic and clearly distinguishable from real Discord snowflakes.
INSERT INTO scores_new
    (message_id, channel_id,
     user_id, guild_id, date, mode, time_spent_ms,
     score1, score2, score3, score4, score5,
     final_score, raw_message, created_at)
SELECT
    'legacy-' || hex(user_id || '|' || COALESCE(guild_id, '') || '|' || date || '|' || mode),
    NULL,
    user_id, guild_id, date, mode, time_spent_ms,
    score1, score2, score3, score4, score5,
    final_score, raw_message, created_at
FROM scores;

DROP TABLE scores;
ALTER TABLE scores_new RENAME TO scores;

COMMIT;
```

### Notes

- Synthetic IDs use the `legacy-` prefix so they are trivially distinguishable from real Discord snowflakes (which are numeric strings).
- `channel_id` is NULL for legacy rows until backfilled.
- The `UNIQUE(user_id, guild_id, date, mode)` constraint preserves the existing upsert behavior — `ON CONFLICT` in `upsert_score` targets this constraint.

---

## Phase 2: Backfill via Discord API

### Trigger

New admin-only slash command:

```
/admin backfill
```

Restricted to users with `ADMINISTRATOR` permission (same as existing admin commands).

### Algorithm

```
counters = { updated: 0, already_filled: 0, unmatched: 0, channels: 0, api_calls: 0 }
min_date = SELECT MIN(date) FROM scores

for each channel_id in DISCORD_CHANNEL_IDS:
    counters.channels += 1
    before_id = None

    loop:
        messages = fetch_messages(channel_id, before=before_id, limit=100)
        counters.api_calls += 1

        if messages is empty:
            break

        for msg in messages:
            before_id = msg.id  -- for pagination

            -- Stop scanning if we've gone past our earliest DB entry
            if msg.timestamp.date() < min_date - 1 day:
                break outer loop for this channel

            user_id = msg.author.id
            guild_id = msg.guild_id
            content = msg.content

            parsed = parse_maptap_message(user_id, guild_id, content)
                     OR parse_challenge_message(user_id, guild_id, content)

            if parsed is None:
                continue  -- not a maptap message, silent skip

            if parsed is Err:
                continue  -- invalid maptap message, silent skip

            score = parsed.unwrap()

            -- Attempt to update the matching legacy row
            rows_affected = UPDATE scores
                SET message_id = msg.id,
                    channel_id = msg.channel_id
                WHERE user_id = score.user_id
                  AND guild_id = score.guild_id
                  AND date = score.date
                  AND mode = score.mode
                  AND message_id LIKE 'legacy-%'

            if rows_affected == 1:
                counters.updated += 1
                log SUCCESS
            else if rows_affected == 0:
                -- Either already backfilled or no matching DB row
                exists = SELECT message_id FROM scores
                    WHERE user_id = score.user_id
                      AND guild_id = score.guild_id
                      AND date = score.date
                      AND mode = score.mode

                if exists AND NOT starts_with(exists, 'legacy-'):
                    counters.already_filled += 1
                    log ALREADY_FILLED
                else:
                    counters.unmatched += 1
                    log UNMATCHED

log SUMMARY(counters)
```

### Important: process order

Discord's `GET /channels/{channel_id}/messages` returns messages in **reverse chronological order** (newest first). This is correct for our purposes — since "latest post wins," the most recent message for a given `(user_id, date, mode)` is the one that was actually kept in the DB. It gets matched first, and the `AND message_id LIKE 'legacy-%'` guard prevents earlier (overwritten) messages from clobbering it.

### Discord API details

- **Endpoint**: serenity's `ChannelId::messages(&http, GetMessages::new().before(before_id).limit(100))`
- **Rate limits**: Discord allows ~50 requests/second per bot. For a channel with 10,000 messages, that's ~100 API calls — should complete in seconds.
- **Pagination**: Each call returns up to 100 messages. Use the ID of the last (oldest) message as the `before` parameter for the next call.

---

## Logging

### Per-row logging

**Successful update:**
```
info!("Backfill: updated score — user={}, date={}, mode={} -> message_id={}, channel_id={}",
      user_id, date, mode, message_id, channel_id)
```

**Row already has real message_id (skip):**
```
debug!("Backfill: skip (already filled) — user={}, date={}, mode={}, message_id={}",
       user_id, date, mode, existing_message_id)
```

**Parsed valid score but no matching DB row:**
```
warn!("Backfill: no matching DB row — message_id={}, user={}, date={}, mode={}",
      message_id, user_id, date, mode)
```

**Discord API error for a channel:**
```
error!("Backfill: failed to fetch messages from channel={}: {}", channel_id, error)
```

### Summary report at exit

```
info!("Backfill complete: {} updated, {} already filled, {} unmatched, {} channels scanned, {} API calls",
      counters.updated, counters.already_filled, counters.unmatched, counters.channels, counters.api_calls)
```

Additionally, report how many legacy rows remain:

```
remaining = SELECT COUNT(*) FROM scores WHERE message_id LIKE 'legacy-%'
info!("Backfill: {} rows still have synthetic IDs (source messages not found)", remaining)
```

---

## Edge cases

| Case | Handling |
|---|---|
| **Deleted messages** | Won't appear in channel history. Rows keep their `legacy-` synthetic ID. Counted in final "still have synthetic IDs" report. |
| **Edited messages** | Discord returns the latest edit. If editing broke the format, parse fails silently. If it parses differently, the `AND message_id LIKE 'legacy-%'` guard + reverse-chronological processing means the most recent valid post for a composite key is matched first. |
| **Multiple valid msgs per user/day/mode** | "Latest post wins" — already the DB rule. Reverse-chronological scanning + the `LIKE 'legacy-%'` guard means the first (most recent) match claims the row; subsequent older messages for the same key are logged as `unmatched`. |
| **DMs (NULL guild_id)** | No channel to scan. These rows keep synthetic IDs. |
| **Bot messages** | Skip messages where `msg.author.bot == true`. |
| **Channels no longer in allowlist** | Only channels in `DISCORD_CHANNEL_IDS` are scanned. Scores from removed channels keep synthetic IDs. |

---

## Code changes required

### `models.rs` — `MaptapScore` struct

Add fields:
```rust
pub message_id: u64,
pub channel_id: Option<u64>,
```

### `parser.rs` — parser signatures

Both `parse_maptap_message` and `parse_challenge_message` need `channel_id: u64` and `message_id: u64` parameters (or the caller sets them on the returned `MaptapScore` after parsing).

**Recommended approach**: keep parser signatures unchanged (they deal with text parsing, not Discord metadata). Have the caller (`handler.rs`) set `message_id` and `channel_id` on the returned `MaptapScore` after a successful parse. This keeps the parsers pure text-processing functions.

### `handler.rs` — message handler

After a successful parse, set the Discord metadata before saving:

```rust
let mut score = score;
score.message_id = msg.id.get();
score.channel_id = Some(msg.channel_id.get());
```

### `db.rs` — schema + queries

- **Migration 3**: as described in Phase 1 above.
- **`upsert_score`**: include `message_id` and `channel_id` in INSERT. Change `ON CONFLICT` to target the UNIQUE constraint: `ON CONFLICT(user_id, guild_id, date, mode)`. Update SET clause to also overwrite `message_id` and `channel_id`.
- **`ScoreRow`**: add `message_id: String` and `channel_id: Option<String>` fields.
- **Admin queries** (`list_scores`, `list_all_scores`): include `message_id` and `channel_id` in SELECT.
- **`delete_score`**: consider allowing deletion by `message_id` as well (single-column lookup).

### `handler.rs` — new `/admin backfill` command

New slash command implementing the Phase 2 algorithm. Responds with an ephemeral message summarizing results.

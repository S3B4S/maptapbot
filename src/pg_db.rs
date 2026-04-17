use std::sync::Mutex;

use tokio_postgres::NoTls;
use tracing::info;

use crate::db::{Database, UserRow};

// ── Row types ────────────────────────────────────────────────────────────────

/// Full score row for PostgreSQL sync.
#[derive(Debug)]
struct SyncScoreRow {
    message_id: String,
    channel_id: Option<String>,
    channel_parent_id: Option<String>,
    user_id: String,
    guild_id: Option<String>,
    date: String,
    mode: String,
    time_spent_ms: Option<i64>,
    score1: Option<i64>,
    score2: Option<i64>,
    score3: Option<i64>,
    score4: Option<i64>,
    score5: Option<i64>,
    final_score: i64,
    raw_message: Option<String>,
    created_at: Option<String>,
    posted_at: String,
    invalid: bool,
}

/// Full stats_snapshots row for PostgreSQL sync.
#[derive(Debug)]
struct SyncStatsSnapshotRow {
    user_id: String,
    taken_at: String,
    total_entries: i64,
    unique_users: i64,
    min_date: Option<String>,
    max_date: Option<String>,
    daily_default_count: i64,
    daily_challenge_count: i64,
}

// ── SQLite dump helpers ───────────────────────────────────────────────────────

fn dump_scores(conn: &rusqlite::Connection) -> Result<Vec<SyncScoreRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT message_id, channel_id, channel_parent_id,
                user_id, guild_id, date, mode, time_spent_ms,
                score1, score2, score3, score4, score5,
                final_score, raw_message, created_at, posted_at, invalid
         FROM scores ORDER BY posted_at",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SyncScoreRow {
            message_id: row.get(0)?,
            channel_id: row.get(1)?,
            channel_parent_id: row.get(2)?,
            user_id: row.get(3)?,
            guild_id: row.get(4)?,
            date: row.get(5)?,
            mode: row.get(6)?,
            time_spent_ms: row.get(7)?,
            score1: row.get(8)?,
            score2: row.get(9)?,
            score3: row.get(10)?,
            score4: row.get(11)?,
            score5: row.get(12)?,
            final_score: row.get(13)?,
            raw_message: row.get(14)?,
            created_at: row.get(15)?,
            posted_at: row.get(16)?,
            invalid: row.get::<_, i64>(17)? != 0,
        })
    })?;
    rows.collect()
}

fn dump_stats_snapshots(
    conn: &rusqlite::Connection,
) -> Result<Vec<SyncStatsSnapshotRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT user_id, taken_at, total_entries, unique_users,
                min_date, max_date, daily_default_count, daily_challenge_count
         FROM stats_snapshots",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SyncStatsSnapshotRow {
            user_id: row.get(0)?,
            taken_at: row.get(1)?,
            total_entries: row.get(2)?,
            unique_users: row.get(3)?,
            min_date: row.get(4)?,
            max_date: row.get(5)?,
            daily_default_count: row.get(6)?,
            daily_challenge_count: row.get(7)?,
        })
    })?;
    rows.collect()
}

fn dump_hit_list(conn: &rusqlite::Connection) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT user_id FROM hit_list")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect()
}

// ── Sync function ─────────────────────────────────────────────────────────────

/// Copy all SQLite data to PostgreSQL.
/// SQLite is the source of truth: existing PG rows are untouched (ON CONFLICT DO NOTHING).
pub async fn sync_sqlite_to_postgres(db: &Mutex<Database>, pg_url: &str) -> String {
    // 1. Dump all tables from SQLite, then release the lock.
    let dump_result: Result<(Vec<UserRow>, Vec<SyncScoreRow>, Vec<SyncStatsSnapshotRow>, Vec<String>), String> = {
        let guard = match db.lock() {
            Ok(g) => g,
            Err(e) => return format!("Failed to lock SQLite database: {}", e),
        };
        let users = match guard.list_users() {
            Ok(v) => v,
            Err(e) => return format!("SQLite error reading users: {}", e),
        };
        let scores = match dump_scores(&guard.conn) {
            Ok(v) => v,
            Err(e) => return format!("SQLite error reading scores: {}", e),
        };
        let snapshots = match dump_stats_snapshots(&guard.conn) {
            Ok(v) => v,
            Err(e) => return format!("SQLite error reading stats_snapshots: {}", e),
        };
        let hit_list = match dump_hit_list(&guard.conn) {
            Ok(v) => v,
            Err(e) => return format!("SQLite error reading hit_list: {}", e),
        };
        Ok((users, scores, snapshots, hit_list))
    };

    let (users, scores, snapshots, hit_list) = match dump_result {
        Ok(t) => t,
        Err(e) => return e,
    };

    info!(
        "pg_sync: dumped {} users, {} scores, {} snapshots, {} hit_list from SQLite",
        users.len(),
        scores.len(),
        snapshots.len(),
        hit_list.len()
    );

    // 2. Connect to PostgreSQL.
    let (mut client, connection) = match tokio_postgres::connect(pg_url, NoTls).await {
        Ok(pair) => pair,
        Err(e) => return format!("Failed to connect to PostgreSQL: {}", e),
    };
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("pg_sync: connection error: {}", e);
        }
    });

    // 3. Create tables.
    let ddl = "
        CREATE TABLE IF NOT EXISTS users (
            user_id  TEXT PRIMARY KEY,
            username TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS scores (
            message_id        TEXT PRIMARY KEY,
            channel_id        TEXT,
            channel_parent_id TEXT,
            user_id           TEXT NOT NULL,
            guild_id          TEXT,
            date              TEXT NOT NULL,
            mode              TEXT NOT NULL DEFAULT 'daily_default',
            time_spent_ms     BIGINT,
            score1            BIGINT,
            score2            BIGINT,
            score3            BIGINT,
            score4            BIGINT,
            score5            BIGINT,
            final_score       BIGINT NOT NULL,
            raw_message       TEXT,
            created_at        TEXT,
            posted_at         TEXT NOT NULL,
            invalid           BIGINT NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS stats_snapshots (
            user_id               TEXT PRIMARY KEY,
            taken_at              TEXT NOT NULL,
            total_entries         BIGINT NOT NULL,
            unique_users          BIGINT NOT NULL,
            min_date              TEXT,
            max_date              TEXT,
            daily_default_count   BIGINT NOT NULL,
            daily_challenge_count BIGINT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS hit_list (
            user_id TEXT PRIMARY KEY
        );
    ";
    if let Err(e) = client.batch_execute(ddl).await {
        return format!("Failed to create PostgreSQL tables: {}", e);
    }

    // 4. Insert everything in a single transaction.
    let tx = match client.transaction().await {
        Ok(t) => t,
        Err(e) => return format!("Failed to begin PostgreSQL transaction: {}", e),
    };

    // users
    let users_total = users.len() as u64;
    let mut users_copied: u64 = 0;
    for row in &users {
        match tx
            .execute(
                "INSERT INTO users (user_id, username) VALUES ($1, $2) \
                 ON CONFLICT (user_id) DO NOTHING",
                &[&row.user_id, &row.username],
            )
            .await
        {
            Ok(n) => users_copied += n,
            Err(e) => {
                let _ = tx.rollback().await;
                return format!("PG error on user {}: {}", row.user_id, e);
            }
        }
    }

    // scores
    let scores_total = scores.len() as u64;
    let mut scores_copied: u64 = 0;
    for row in &scores {
        let invalid_val = row.invalid as i64;
        match tx
            .execute(
                "INSERT INTO scores (
                     message_id, channel_id, channel_parent_id,
                     user_id, guild_id, date, mode, time_spent_ms,
                     score1, score2, score3, score4, score5,
                     final_score, raw_message, created_at, posted_at, invalid
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
                 ON CONFLICT (message_id) DO NOTHING",
                &[
                    &row.message_id,
                    &row.channel_id,
                    &row.channel_parent_id,
                    &row.user_id,
                    &row.guild_id,
                    &row.date,
                    &row.mode,
                    &row.time_spent_ms,
                    &row.score1,
                    &row.score2,
                    &row.score3,
                    &row.score4,
                    &row.score5,
                    &row.final_score,
                    &row.raw_message,
                    &row.created_at,
                    &row.posted_at,
                    &invalid_val,
                ],
            )
            .await
        {
            Ok(n) => scores_copied += n,
            Err(e) => {
                let _ = tx.rollback().await;
                return format!("PG error on score {}: {}", row.message_id, e);
            }
        }
    }

    // stats_snapshots
    let snapshots_total = snapshots.len() as u64;
    let mut snapshots_copied: u64 = 0;
    for row in &snapshots {
        match tx
            .execute(
                "INSERT INTO stats_snapshots (
                     user_id, taken_at, total_entries, unique_users,
                     min_date, max_date, daily_default_count, daily_challenge_count
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                 ON CONFLICT (user_id) DO NOTHING",
                &[
                    &row.user_id,
                    &row.taken_at,
                    &row.total_entries,
                    &row.unique_users,
                    &row.min_date,
                    &row.max_date,
                    &row.daily_default_count,
                    &row.daily_challenge_count,
                ],
            )
            .await
        {
            Ok(n) => snapshots_copied += n,
            Err(e) => {
                let _ = tx.rollback().await;
                return format!("PG error on snapshot {}: {}", row.user_id, e);
            }
        }
    }

    // hit_list
    let hit_total = hit_list.len() as u64;
    let mut hit_copied: u64 = 0;
    for user_id in &hit_list {
        match tx
            .execute(
                "INSERT INTO hit_list (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
                &[user_id],
            )
            .await
        {
            Ok(n) => hit_copied += n,
            Err(e) => {
                let _ = tx.rollback().await;
                return format!("PG error on hit_list {}: {}", user_id, e);
            }
        }
    }

    // 5. Commit.
    if let Err(e) = tx.commit().await {
        return format!("Failed to commit PostgreSQL transaction: {}", e);
    }

    info!("pg_sync: complete");

    format!(
        "SQLite \u{2192} PostgreSQL sync complete.\n\
         \u{2003}users:           {}/{} copied\n\
         \u{2003}scores:          {}/{} copied\n\
         \u{2003}stats_snapshots: {}/{} copied\n\
         \u{2003}hit_list:        {}/{} copied",
        users_copied,
        users_total,
        scores_copied,
        scores_total,
        snapshots_copied,
        snapshots_total,
        hit_copied,
        hit_total,
    )
}

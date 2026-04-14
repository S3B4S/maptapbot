use rusqlite::{params, Connection};

use crate::models::{GameMode, MaptapScore};

/// Row returned by leaderboard queries.
#[derive(Debug)]
pub struct LeaderboardRow {
    pub user_id: String,
    pub username: String,
    /// Individual scores — None means the tile was timed out (challenge mode only, daily view).
    pub score1: Option<f64>,
    pub score2: Option<f64>,
    pub score3: Option<f64>,
    pub score4: Option<f64>,
    pub score5: Option<f64>,
    pub final_score: f64,
    /// Only populated for challenge leaderboards.
    pub time_spent_ms: Option<f64>,
}

/// Row returned by admin score-listing queries.
#[derive(Debug)]
pub struct ScoreRow {
    pub message_id: String,
    pub channel_id: Option<String>,
    pub user_id: String,
    pub username: String,
    pub guild_id: Option<String>,
    pub date: String,
    pub mode: String,
    pub score1: Option<i64>,
    pub score2: Option<i64>,
    pub score3: Option<i64>,
    pub score4: Option<i64>,
    pub score5: Option<i64>,
    pub final_score: i64,
    pub time_spent_ms: Option<i64>,
}

/// Row returned by list_users.
#[derive(Debug)]
pub struct UserRow {
    pub user_id: String,
    pub username: String,
}

/// Aggregate DB statistics.
#[derive(Debug)]
pub struct DbStats {
    pub total_entries: i64,
    pub unique_users: i64,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
    pub daily_default_count: i64,
    pub daily_challenge_count: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.initialize()?;
        db.migrate()?;
        Ok(db)
    }

    fn initialize(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                user_id  TEXT PRIMARY KEY,
                username TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS scores (
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
            );",
        )?;
        Ok(())
    }

    /// Migrate existing databases that predate the mode/time_spent_ms columns
    /// and the (user_id, guild_id, date, mode) primary key.
    ///
    /// Migration 1: add mode/time_spent_ms columns (keyed on absence of `mode` column).
    /// Migration 2: make score1-5 nullable (keyed on `notnull` flag of score1 column).
    /// Migration 3: add message_id (PK) + channel_id columns (keyed on absence of `message_id` column).
    fn migrate(&self) -> Result<(), rusqlite::Error> {
        // Migration 1: add mode column + restructure PK
        let has_mode: bool = {
            let mut stmt = self
                .conn
                .prepare("SELECT COUNT(*) FROM pragma_table_info('scores') WHERE name = 'mode'")?;
            let count: i64 = stmt.query_row([], |row| row.get(0))?;
            count > 0
        };

        if !has_mode {
            self.conn.execute_batch(
                "BEGIN;

                CREATE TABLE scores_new (
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
                    PRIMARY KEY (user_id, guild_id, date, mode),
                    FOREIGN KEY (user_id) REFERENCES users(user_id)
                );

                INSERT INTO scores_new
                    (user_id, guild_id, date, mode, time_spent_ms,
                     score1, score2, score3, score4, score5,
                     final_score, raw_message, created_at)
                SELECT
                    user_id, guild_id, date, 'daily_default', NULL,
                    score1, score2, score3, score4, score5,
                    final_score, raw_message, created_at
                FROM scores;

                DROP TABLE scores;
                ALTER TABLE scores_new RENAME TO scores;

                COMMIT;",
            )?;
            // Don't return early — fall through to check subsequent migrations.
        }

        // Migration 2: make score1-5 nullable (if score1 still has notnull constraint)
        let score1_notnull: bool = {
            let mut stmt = self.conn.prepare(
                "SELECT \"notnull\" FROM pragma_table_info('scores') WHERE name = 'score1'",
            )?;
            let notnull: i64 = stmt.query_row([], |row| row.get(0))?;
            notnull != 0
        };

        if score1_notnull {
            self.conn.execute_batch(
                "BEGIN;

                CREATE TABLE scores_new (
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
                    PRIMARY KEY (user_id, guild_id, date, mode),
                    FOREIGN KEY (user_id) REFERENCES users(user_id)
                );

                INSERT INTO scores_new SELECT * FROM scores;

                DROP TABLE scores;
                ALTER TABLE scores_new RENAME TO scores;

                COMMIT;",
            )?;
        }

        // Migration 3: add message_id (PK) + channel_id columns.
        // Existing rows get a synthetic "legacy-<hex>" message_id since PK cannot be NULL.
        let has_message_id: bool = {
            let mut stmt = self.conn.prepare(
                "SELECT COUNT(*) FROM pragma_table_info('scores') WHERE name = 'message_id'",
            )?;
            let count: i64 = stmt.query_row([], |row| row.get(0))?;
            count > 0
        };

        if !has_message_id {
            self.conn.execute_batch(
                "BEGIN;

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

                COMMIT;",
            )?;
        }

        Ok(())
    }

    /// Insert or update the username for a user.
    pub fn upsert_user(&self, user_id: u64, username: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO users (user_id, username) VALUES (?1, ?2)
             ON CONFLICT(user_id) DO UPDATE SET username = excluded.username",
            params![user_id.to_string(), username],
        )?;
        Ok(())
    }

    /// Insert or replace a score (latest post wins for same user+guild+date+mode).
    pub fn upsert_score(&self, score: &MaptapScore) -> Result<(), rusqlite::Error> {
        let guild_id_str = score.guild_id.map(|g| g.to_string());
        self.conn.execute(
            "INSERT INTO scores
                 (message_id, channel_id,
                  user_id, guild_id, date, mode, time_spent_ms,
                  score1, score2, score3, score4, score5, final_score, raw_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(user_id, guild_id, date, mode) DO UPDATE SET
                message_id    = excluded.message_id,
                channel_id    = excluded.channel_id,
                score1        = excluded.score1,
                score2        = excluded.score2,
                score3        = excluded.score3,
                score4        = excluded.score4,
                score5        = excluded.score5,
                final_score   = excluded.final_score,
                time_spent_ms = excluded.time_spent_ms,
                raw_message   = excluded.raw_message,
                created_at    = datetime('now')",
            params![
                score.message_id.to_string(),
                score.channel_id.to_string(),
                score.user_id.to_string(),
                guild_id_str,
                score.date.format("%Y-%m-%d").to_string(),
                score.mode.as_str(),
                score.time_spent_ms,
                score.scores[0].map(|v| v as i64),
                score.scores[1].map(|v| v as i64),
                score.scores[2].map(|v| v as i64),
                score.scores[3].map(|v| v as i64),
                score.scores[4].map(|v| v as i64),
                score.final_score,
                score.raw_message,
            ],
        )?;
        Ok(())
    }

    /// Daily leaderboard (default mode): all scores for a given guild + date,
    /// sorted by final_score desc.
    pub fn get_daily_leaderboard(
        &self,
        guild_id: u64,
        date: &str,
    ) -> Result<Vec<LeaderboardRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.user_id, u.username,
                    s.score1, s.score2, s.score3, s.score4, s.score5, s.final_score
             FROM scores s
             JOIN users u ON s.user_id = u.user_id
             WHERE s.guild_id = ?1 AND s.date = ?2 AND s.mode = 'daily_default'
             ORDER BY s.final_score DESC",
        )?;
        let rows = stmt.query_map(params![guild_id.to_string(), date], |row| {
            Ok(LeaderboardRow {
                user_id: row.get(0)?,
                username: row.get(1)?,
                score1: row.get::<_, Option<i64>>(2)?.map(|v| v as f64),
                score2: row.get::<_, Option<i64>>(3)?.map(|v| v as f64),
                score3: row.get::<_, Option<i64>>(4)?.map(|v| v as f64),
                score4: row.get::<_, Option<i64>>(5)?.map(|v| v as f64),
                score5: row.get::<_, Option<i64>>(6)?.map(|v| v as f64),
                final_score: row.get::<_, i64>(7)? as f64,
                time_spent_ms: None,
            })
        })?;
        rows.collect()
    }

    /// Permanent leaderboard (default mode): average scores across all days for a given
    /// guild, sorted by average final_score desc.
    pub fn get_permanent_leaderboard(
        &self,
        guild_id: u64,
    ) -> Result<Vec<LeaderboardRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.user_id,
                    u.username,
                    AVG(s.score1)       as avg_s1,
                    AVG(s.score2)       as avg_s2,
                    AVG(s.score3)       as avg_s3,
                    AVG(s.score4)       as avg_s4,
                    AVG(s.score5)       as avg_s5,
                    AVG(s.final_score)  as avg_final
             FROM scores s
             JOIN users u ON s.user_id = u.user_id
             WHERE s.guild_id = ?1 AND s.mode = 'daily_default'
             GROUP BY s.user_id
             ORDER BY avg_final DESC",
        )?;
        let rows = stmt.query_map(params![guild_id.to_string()], |row| {
            Ok(LeaderboardRow {
                user_id: row.get(0)?,
                username: row.get(1)?,
                score1: row.get::<_, Option<f64>>(2)?,
                score2: row.get::<_, Option<f64>>(3)?,
                score3: row.get::<_, Option<f64>>(4)?,
                score4: row.get::<_, Option<f64>>(5)?,
                score5: row.get::<_, Option<f64>>(6)?,
                final_score: row.get(7)?,
                time_spent_ms: None,
            })
        })?;
        rows.collect()
    }

    /// Daily challenge leaderboard: all challenge scores for a given guild + date,
    /// sorted by final_score desc, then time_spent_ms asc (faster is better).
    pub fn get_daily_challenge_leaderboard(
        &self,
        guild_id: u64,
        date: &str,
    ) -> Result<Vec<LeaderboardRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.user_id, u.username,
                    s.score1, s.score2, s.score3, s.score4, s.score5,
                    s.final_score, s.time_spent_ms
             FROM scores s
             JOIN users u ON s.user_id = u.user_id
             WHERE s.guild_id = ?1 AND s.date = ?2 AND s.mode = 'daily_challenge'
             ORDER BY s.final_score DESC, s.time_spent_ms ASC",
        )?;
        let rows = stmt.query_map(params![guild_id.to_string(), date], |row| {
            Ok(LeaderboardRow {
                user_id: row.get(0)?,
                username: row.get(1)?,
                score1: row.get::<_, Option<i64>>(2)?.map(|v| v as f64),
                score2: row.get::<_, Option<i64>>(3)?.map(|v| v as f64),
                score3: row.get::<_, Option<i64>>(4)?.map(|v| v as f64),
                score4: row.get::<_, Option<i64>>(5)?.map(|v| v as f64),
                score5: row.get::<_, Option<i64>>(6)?.map(|v| v as f64),
                final_score: row.get::<_, i64>(7)? as f64,
                time_spent_ms: row.get::<_, Option<i64>>(8)?.map(|v| v as f64),
            })
        })?;
        rows.collect()
    }

    /// Permanent challenge leaderboard: average scores + average time across all
    /// challenge days for a given guild, sorted by avg final_score desc.
    pub fn get_permanent_challenge_leaderboard(
        &self,
        guild_id: u64,
    ) -> Result<Vec<LeaderboardRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.user_id,
                    u.username,
                    AVG(COALESCE(s.score1, 0)) as avg_s1,
                    AVG(COALESCE(s.score2, 0)) as avg_s2,
                    AVG(COALESCE(s.score3, 0)) as avg_s3,
                    AVG(COALESCE(s.score4, 0)) as avg_s4,
                    AVG(COALESCE(s.score5, 0)) as avg_s5,
                    AVG(s.final_score)          as avg_final,
                    AVG(s.time_spent_ms)        as avg_time
             FROM scores s
             JOIN users u ON s.user_id = u.user_id
             WHERE s.guild_id = ?1 AND s.mode = 'daily_challenge'
             GROUP BY s.user_id
             ORDER BY avg_final DESC",
        )?;
        let rows = stmt.query_map(params![guild_id.to_string()], |row| {
            Ok(LeaderboardRow {
                user_id: row.get(0)?,
                username: row.get(1)?,
                score1: row.get::<_, Option<f64>>(2)?,
                score2: row.get::<_, Option<f64>>(3)?,
                score3: row.get::<_, Option<f64>>(4)?,
                score4: row.get::<_, Option<f64>>(5)?,
                score5: row.get::<_, Option<f64>>(6)?,
                final_score: row.get(7)?,
                time_spent_ms: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    // ── Admin query methods ──────────────────────────────────────────────

    /// Delete a specific score entry. Returns the number of rows deleted (0 or 1).
    pub fn delete_score(
        &self,
        user_id: &str,
        date: &str,
        mode: &str,
    ) -> Result<usize, rusqlite::Error> {
        let deleted = self.conn.execute(
            "DELETE FROM scores WHERE user_id = ?1 AND date = ?2 AND mode = ?3",
            params![user_id, date, mode],
        )?;
        Ok(deleted)
    }

    /// List all scores for a given user across all dates and modes.
    pub fn list_scores(&self, user_id: &str) -> Result<Vec<ScoreRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.message_id, s.channel_id,
                    s.user_id, COALESCE(u.username, s.user_id) as username,
                    s.guild_id, s.date, s.mode,
                    s.score1, s.score2, s.score3, s.score4, s.score5,
                    s.final_score, s.time_spent_ms
             FROM scores s
             LEFT JOIN users u ON s.user_id = u.user_id
             WHERE s.user_id = ?1
             ORDER BY s.date DESC, s.mode",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(ScoreRow {
                message_id: row.get(0)?,
                channel_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                guild_id: row.get(4)?,
                date: row.get(5)?,
                mode: row.get(6)?,
                score1: row.get(7)?,
                score2: row.get(8)?,
                score3: row.get(9)?,
                score4: row.get(10)?,
                score5: row.get(11)?,
                final_score: row.get(12)?,
                time_spent_ms: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    /// Dump all scores in the table.
    pub fn list_all_scores(&self) -> Result<Vec<ScoreRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT s.message_id, s.channel_id,
                    s.user_id, COALESCE(u.username, s.user_id) as username,
                    s.guild_id, s.date, s.mode,
                    s.score1, s.score2, s.score3, s.score4, s.score5,
                    s.final_score, s.time_spent_ms
             FROM scores s
             LEFT JOIN users u ON s.user_id = u.user_id
             ORDER BY s.date DESC, s.user_id, s.mode",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ScoreRow {
                message_id: row.get(0)?,
                channel_id: row.get(1)?,
                user_id: row.get(2)?,
                username: row.get(3)?,
                guild_id: row.get(4)?,
                date: row.get(5)?,
                mode: row.get(6)?,
                score1: row.get(7)?,
                score2: row.get(8)?,
                score3: row.get(9)?,
                score4: row.get(10)?,
                score5: row.get(11)?,
                final_score: row.get(12)?,
                time_spent_ms: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    /// List all known users.
    pub fn list_users(&self) -> Result<Vec<UserRow>, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT user_id, username FROM users ORDER BY username")?;
        let rows = stmt.query_map([], |row| {
            Ok(UserRow {
                user_id: row.get(0)?,
                username: row.get(1)?,
            })
        })?;
        rows.collect()
    }

    /// Return the raw stored message for a specific score entry.
    pub fn raw_score(
        &self,
        user_id: &str,
        date: &str,
        mode: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT raw_message FROM scores WHERE user_id = ?1 AND date = ?2 AND mode = ?3",
        )?;
        let mut rows = stmt.query(params![user_id, date, mode])?;
        match rows.next()? {
            Some(row) => Ok(row.get(0)?),
            None => Ok(None),
        }
    }

    /// Delete all scores for a given date. Returns the number of rows deleted.
    pub fn clear_day(&self, date: &str) -> Result<usize, rusqlite::Error> {
        let deleted = self
            .conn
            .execute("DELETE FROM scores WHERE date = ?1", params![date])?;
        Ok(deleted)
    }

    /// Aggregate DB stats: total entries, unique users, date range, per-mode counts.
    pub fn stats(&self) -> Result<DbStats, rusqlite::Error> {
        let total_entries: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM scores", [], |row| row.get(0))?;
        let unique_users: i64 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT user_id) FROM scores", [], |row| {
                    row.get(0)
                })?;
        let min_date: Option<String> =
            self.conn
                .query_row("SELECT MIN(date) FROM scores", [], |row| row.get(0))?;
        let max_date: Option<String> =
            self.conn
                .query_row("SELECT MAX(date) FROM scores", [], |row| row.get(0))?;
        let daily_default_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM scores WHERE mode = 'daily_default'",
            [],
            |row| row.get(0),
        )?;
        let daily_challenge_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM scores WHERE mode = 'daily_challenge'",
            [],
            |row| row.get(0),
        )?;
        Ok(DbStats {
            total_entries,
            unique_users,
            min_date,
            max_date,
            daily_default_count,
            daily_challenge_count,
        })
    }

    // ── Backfill methods ─────────────────────────────────────────────

    /// Get the earliest date in the scores table (for backfill stop condition).
    pub fn min_score_date(&self) -> Result<Option<String>, rusqlite::Error> {
        self.conn
            .query_row("SELECT MIN(date) FROM scores", [], |row| row.get(0))
    }

    /// Attempt to backfill a legacy row with real Discord message metadata.
    /// Only updates if the row still has a synthetic `legacy-` message_id.
    /// Returns the number of rows updated (0 or 1).
    pub fn backfill_score(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
        date: &str,
        mode: &str,
        message_id: &str,
        channel_id: &str,
    ) -> Result<usize, rusqlite::Error> {
        let updated = self.conn.execute(
            "UPDATE scores
             SET message_id = ?5, channel_id = ?6
             WHERE user_id = ?1
               AND guild_id IS ?2
               AND date = ?3
               AND mode = ?4
               AND message_id LIKE 'legacy-%'",
            params![user_id, guild_id, date, mode, message_id, channel_id],
        )?;
        Ok(updated)
    }

    /// Check whether a score exists for the given composite key and return its message_id.
    pub fn get_score_message_id(
        &self,
        user_id: &str,
        guild_id: Option<&str>,
        date: &str,
        mode: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT message_id FROM scores
             WHERE user_id = ?1 AND guild_id IS ?2 AND date = ?3 AND mode = ?4",
        )?;
        let mut rows = stmt.query(params![user_id, guild_id, date, mode])?;
        match rows.next()? {
            Some(row) => Ok(row.get(0)?),
            None => Ok(None),
        }
    }

    /// Count how many rows still have synthetic legacy message IDs.
    pub fn count_legacy_scores(&self) -> Result<i64, rusqlite::Error> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM scores WHERE message_id LIKE 'legacy-%'",
            [],
            |row| row.get(0),
        )
    }

    /// Create a backup of the database using SQLite's backup API.
    /// This is safe to call while the database is open and handles in-progress transactions.
    pub fn backup(&self, dest_path: &str) -> Result<(), rusqlite::Error> {
        let mut dest = Connection::open(dest_path)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(250), None)
    }
}

#[cfg(test)]
#[path = "tests/db.rs"]
mod tests;

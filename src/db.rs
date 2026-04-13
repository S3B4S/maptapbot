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
            );",
        )?;
        Ok(())
    }

    /// Migrate existing databases that predate the mode/time_spent_ms columns
    /// and the (user_id, guild_id, date, mode) primary key.
    ///
    /// Migration 1: add mode/time_spent_ms columns (keyed on absence of `mode` column).
    /// Migration 2: make score1-5 nullable (keyed on `notnull` flag of score1 column).
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
            return Ok(());
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
                 (user_id, guild_id, date, mode, time_spent_ms,
                  score1, score2, score3, score4, score5, final_score, raw_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(user_id, guild_id, date, mode) DO UPDATE SET
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn test_db() -> Database {
        Database::open(":memory:").unwrap()
    }

    fn make_score(
        user_id: u64,
        guild_id: u64,
        day: u32,
        scores: [Option<u32>; 5],
        final_score: u32,
        mode: GameMode,
        time_spent_ms: Option<u32>,
    ) -> MaptapScore {
        MaptapScore {
            user_id,
            guild_id: Some(guild_id),
            mode,
            time_spent_ms,
            date: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            scores,
            final_score,
            raw_message: "test".to_string(),
        }
    }

    /// Helper: upsert user then default-mode score
    fn insert_score(
        db: &Database,
        user_id: u64,
        guild_id: u64,
        day: u32,
        scores: [Option<u32>; 5],
        final_score: u32,
    ) {
        db.upsert_user(user_id, &format!("user{}", user_id))
            .unwrap();
        db.upsert_score(&make_score(
            user_id,
            guild_id,
            day,
            scores,
            final_score,
            GameMode::DailyDefault,
            None,
        ))
        .unwrap();
    }

    /// Helper: upsert user then challenge-mode score
    fn insert_challenge_score(
        db: &Database,
        user_id: u64,
        guild_id: u64,
        day: u32,
        scores: [Option<u32>; 5],
        final_score: u32,
        time_spent_ms: u32,
    ) {
        db.upsert_user(user_id, &format!("user{}", user_id))
            .unwrap();
        db.upsert_score(&make_score(
            user_id,
            guild_id,
            day,
            scores,
            final_score,
            GameMode::DailyChallenge,
            Some(time_spent_ms),
        ))
        .unwrap();
    }

    #[test]
    fn test_upsert_user() {
        let db = test_db();
        db.upsert_user(1, "alice").unwrap();
        db.upsert_user(1, "alice_renamed").unwrap();

        let name: String = db
            .conn
            .query_row(
                "SELECT username FROM users WHERE user_id = '1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "alice_renamed");
    }

    #[test]
    fn test_insert_and_query() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );

        let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].final_score, 823.0);
        assert_eq!(results[0].username, "user1");
        assert_eq!(results[0].time_spent_ms, None);
    }

    #[test]
    fn test_upsert_overwrites() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(50), Some(50), Some(50), Some(50), Some(50)],
            600,
        );
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );

        let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].final_score, 823.0);
    }

    #[test]
    fn test_multiple_users_daily() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );
        insert_score(
            &db,
            2,
            100,
            13,
            [Some(50), Some(50), Some(50), Some(50), Some(50)],
            600,
        );

        let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].final_score, 823.0);
        assert_eq!(results[1].final_score, 600.0);
    }

    #[test]
    fn test_guild_scoping() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );
        insert_score(
            &db,
            2,
            200,
            13,
            [Some(50), Some(50), Some(50), Some(50), Some(50)],
            600,
        );

        assert_eq!(
            db.get_daily_leaderboard(100, "2026-04-13").unwrap().len(),
            1
        );
        assert_eq!(
            db.get_daily_leaderboard(200, "2026-04-13").unwrap().len(),
            1
        );
    }

    #[test]
    fn test_permanent_leaderboard_averages() {
        let db = test_db();
        // User 1: two days, scores 800 and 600 -> avg 700
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(80), Some(80), Some(80), Some(80), Some(80)],
            800,
        );
        insert_score(
            &db,
            1,
            100,
            14,
            [Some(60), Some(60), Some(60), Some(60), Some(60)],
            600,
        );
        // User 2: one day, score 750
        insert_score(
            &db,
            2,
            100,
            13,
            [Some(75), Some(75), Some(75), Some(75), Some(75)],
            750,
        );

        let results = db.get_permanent_leaderboard(100).unwrap();
        assert_eq!(results.len(), 2);
        // User 2 (avg 750) first, user 1 (avg 700) second
        assert_eq!(results[0].user_id, "2");
        assert_eq!(results[0].username, "user2");
        assert_eq!(results[0].final_score, 750.0);
        assert_eq!(results[1].user_id, "1");
        assert_eq!(results[1].username, "user1");
        assert_eq!(results[1].final_score, 700.0);
    }

    #[test]
    fn test_username_update_reflected_in_leaderboard() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );

        // Rename user
        db.upsert_user(1, "new_name").unwrap();

        let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(results[0].username, "new_name");
    }

    // ── Challenge leaderboard tests ──────────────────────────────

    #[test]
    fn test_challenge_daily_leaderboard() {
        let db = test_db();
        // user1: score 914 in 21100ms
        insert_challenge_score(
            &db,
            1,
            100,
            13,
            [Some(89), Some(82), Some(94), Some(88), Some(97)],
            914,
            21100,
        );
        // user2: same score but slower
        insert_challenge_score(
            &db,
            2,
            100,
            13,
            [Some(89), Some(82), Some(94), Some(88), Some(97)],
            914,
            25000,
        );

        let results = db
            .get_daily_challenge_leaderboard(100, "2026-04-13")
            .unwrap();
        assert_eq!(results.len(), 2);
        // Same final score → ordered by time (faster first)
        assert_eq!(results[0].user_id, "1");
        assert_eq!(results[0].time_spent_ms, Some(21100.0));
        assert_eq!(results[1].user_id, "2");
    }

    #[test]
    fn test_challenge_default_scores_not_mixed() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );
        insert_challenge_score(
            &db,
            2,
            100,
            13,
            [Some(89), Some(82), Some(94), Some(88), Some(97)],
            914,
            21100,
        );

        let default_results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(default_results.len(), 1);
        assert_eq!(default_results[0].user_id, "1");

        let challenge_results = db
            .get_daily_challenge_leaderboard(100, "2026-04-13")
            .unwrap();
        assert_eq!(challenge_results.len(), 1);
        assert_eq!(challenge_results[0].user_id, "2");
    }

    #[test]
    fn test_challenge_permanent_leaderboard_averages() {
        let db = test_db();
        // user1: two days, 914 and 800
        insert_challenge_score(
            &db,
            1,
            100,
            12,
            [Some(89), Some(82), Some(94), Some(88), Some(97)],
            914,
            21100,
        );
        insert_challenge_score(
            &db,
            1,
            100,
            13,
            [Some(80), Some(80), Some(80), Some(80), Some(80)],
            800,
            30000,
        );
        // user2: one day, 900
        insert_challenge_score(
            &db,
            2,
            100,
            12,
            [Some(85), Some(85), Some(90), Some(85), Some(90)],
            900,
            18000,
        );

        let results = db.get_permanent_challenge_leaderboard(100).unwrap();
        assert_eq!(results.len(), 2);
        // user2 (avg 900) > user1 (avg 857)
        assert_eq!(results[0].user_id, "2");
        assert_eq!(results[0].time_spent_ms, Some(18000.0));
        assert_eq!(results[1].user_id, "1");
        // avg time for user1: (21100 + 30000) / 2 = 25550
        assert_eq!(results[1].time_spent_ms, Some(25550.0));
    }

    #[test]
    fn test_same_user_can_have_both_modes_same_day() {
        let db = test_db();
        insert_score(
            &db,
            1,
            100,
            13,
            [Some(93), Some(90), Some(83), Some(61), Some(97)],
            823,
        );
        insert_challenge_score(
            &db,
            1,
            100,
            13,
            [Some(89), Some(82), Some(94), Some(88), Some(97)],
            914,
            21100,
        );

        let default_results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
        assert_eq!(default_results.len(), 1);
        assert_eq!(default_results[0].final_score, 823.0);

        let challenge_results = db
            .get_daily_challenge_leaderboard(100, "2026-04-13")
            .unwrap();
        assert_eq!(challenge_results.len(), 1);
        assert_eq!(challenge_results[0].final_score, 914.0);
    }

    #[test]
    fn test_challenge_null_score_stored_and_retrieved() {
        let db = test_db();
        // Timed-out tile: last score is None
        // (96+4)*1 + 68*2 + (91+0)*3 = 509
        insert_challenge_score(
            &db,
            1,
            100,
            13,
            [Some(96), Some(4), Some(68), Some(91), None],
            509,
            25000,
        );

        let results = db
            .get_daily_challenge_leaderboard(100, "2026-04-13")
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score5, None);
        assert_eq!(results[0].final_score, 509.0);
    }

    #[test]
    fn test_challenge_null_score_averaged_as_zero() {
        let db = test_db();
        // Day 1: all scores present — score5 = 80
        insert_challenge_score(
            &db,
            1,
            100,
            12,
            [Some(80), Some(80), Some(80), Some(80), Some(80)],
            800,
            20000,
        );
        // Day 2: score5 timed out (None = 0 for avg)
        // (96+4)*1 + 68*2 + (91+0)*3 = 509
        insert_challenge_score(
            &db,
            1,
            100,
            13,
            [Some(96), Some(4), Some(68), Some(91), None],
            509,
            25000,
        );

        let results = db.get_permanent_challenge_leaderboard(100).unwrap();
        assert_eq!(results.len(), 1);
        // avg score5: (80 + 0) / 2 = 40.0
        assert_eq!(results[0].score5, Some(40.0));
    }
}

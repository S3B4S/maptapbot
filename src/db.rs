use rusqlite::{params, Connection};

use crate::models::MaptapScore;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.initialize()?;
        Ok(db)
    }

    fn initialize(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scores (
                user_id     TEXT NOT NULL,
                date        TEXT NOT NULL,
                score1      INTEGER NOT NULL,
                score2      INTEGER NOT NULL,
                score3      INTEGER NOT NULL,
                score4      INTEGER NOT NULL,
                score5      INTEGER NOT NULL,
                final_score INTEGER NOT NULL,
                raw_message TEXT,
                created_at  TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (user_id, date)
            );",
        )?;
        Ok(())
    }

    /// Insert or replace a score (latest post wins for same user+date).
    pub fn upsert_score(&self, score: &MaptapScore) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO scores (user_id, date, score1, score2, score3, score4, score5, final_score, raw_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(user_id, date) DO UPDATE SET
                score1 = excluded.score1,
                score2 = excluded.score2,
                score3 = excluded.score3,
                score4 = excluded.score4,
                score5 = excluded.score5,
                final_score = excluded.final_score,
                raw_message = excluded.raw_message,
                created_at = datetime('now')",
            params![
                score.user_id.to_string(),
                score.date.format("%Y-%m-%d").to_string(),
                score.scores[0],
                score.scores[1],
                score.scores[2],
                score.scores[3],
                score.scores[4],
                score.final_score,
                score.raw_message,
            ],
        )?;
        Ok(())
    }

    /// Get all scores for a given date, ordered by final_score descending.
    pub fn get_scores_by_date(&self, date: &str) -> Result<Vec<(String, u32)>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT user_id, final_score FROM scores WHERE date = ?1 ORDER BY final_score DESC",
        )?;
        let rows = stmt.query_map(params![date], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
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

    fn make_score(user_id: u64, day: u32, final_score: u32) -> MaptapScore {
        MaptapScore {
            user_id,
            date: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
            scores: [93, 90, 83, 61, 97],
            final_score,
            raw_message: "test".to_string(),
        }
    }

    #[test]
    fn test_insert_and_query() {
        let db = test_db();
        let score = make_score(1, 13, 823);
        db.upsert_score(&score).unwrap();

        let results = db.get_scores_by_date("2026-04-13").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 823);
    }

    #[test]
    fn test_upsert_overwrites() {
        let db = test_db();
        db.upsert_score(&make_score(1, 13, 800)).unwrap();
        db.upsert_score(&make_score(1, 13, 823)).unwrap();

        let results = db.get_scores_by_date("2026-04-13").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 823);
    }

    #[test]
    fn test_multiple_users() {
        let db = test_db();
        db.upsert_score(&make_score(1, 13, 823)).unwrap();
        db.upsert_score(&make_score(2, 13, 750)).unwrap();

        let results = db.get_scores_by_date("2026-04-13").unwrap();
        assert_eq!(results.len(), 2);
        // Ordered by final_score desc
        assert_eq!(results[0].1, 823);
        assert_eq!(results[1].1, 750);
    }
}

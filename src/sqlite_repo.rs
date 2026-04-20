use std::sync::Mutex;

use crate::db::{Database, LeaderboardRow, ScoreRow};
use crate::repository::Repository;

pub struct SqliteRepository<'a> {
    db: &'a Mutex<Database>,
}

impl<'a> SqliteRepository<'a> {
    pub fn new(db: &'a Mutex<Database>) -> Self {
        SqliteRepository { db }
    }
}

impl Repository for SqliteRepository<'_> {
    fn get_scores(&self) -> Result<Vec<ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_all_scores().map_err(|e| e.to_string())
    }

    fn get_scores_today(&self) -> Result<Vec<ScoreRow>, String> {
        todo!()
    }

    fn get_scores_user(&self, user_id: String) -> Result<Vec<ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_scores(&user_id).map_err(|e| e.to_string())
    }

    fn get_daily_leaderboard(&self, guild_id: u64, date: &str) -> Result<Vec<LeaderboardRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_daily_leaderboard(guild_id, date).map_err(|e| e.to_string())
    }

    fn get_permanent_leaderboard(&self, guild_id: u64) -> Result<Vec<LeaderboardRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_permanent_leaderboard(guild_id).map_err(|e| e.to_string())
    }

    fn get_daily_challenge_leaderboard(&self, guild_id: u64, date: &str) -> Result<Vec<LeaderboardRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_daily_challenge_leaderboard(guild_id, date).map_err(|e| e.to_string())
    }

    fn get_permanent_challenge_leaderboard(&self, guild_id: u64) -> Result<Vec<LeaderboardRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_permanent_challenge_leaderboard(guild_id).map_err(|e| e.to_string())
    }

    fn get_weekly_leaderboard(&self, guild_id: u64, week_start: &str, week_end: &str, use_sum: bool) -> Result<Vec<LeaderboardRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_weekly_leaderboard(guild_id, week_start, week_end, use_sum).map_err(|e| e.to_string())
    }
}

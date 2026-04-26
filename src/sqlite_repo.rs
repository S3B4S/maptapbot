use std::sync::Mutex;

use crate::db::{Database, DbStats, LeaderboardRow, ScoreRow, StatsDelta, StatsSnapshot, UserRow};
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
    // ── Read: scores ────────────────────────────────────────────────────────

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

    // ── Read: leaderboards ───────────────────────────────────────────────────

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

    // ── Admin: score management ──────────────────────────────────────────────

    fn get_score(&self, message_id: &str) -> Result<Option<ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_score(message_id).map_err(|e| e.to_string())
    }

    fn delete_score(&self, message_id: &str) -> Result<usize, String> {
        let db = self.db.lock().unwrap();
        db.delete_score(message_id).map_err(|e| e.to_string())
    }

    fn invalidate_score(&self, message_id: &str) -> Result<usize, String> {
        let db = self.db.lock().unwrap();
        db.invalidate_score(message_id).map_err(|e| e.to_string())
    }

    fn list_scores(&self, user_id: &str) -> Result<Vec<ScoreRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_scores(user_id).map_err(|e| e.to_string())
    }

    fn list_users(&self) -> Result<Vec<UserRow>, String> {
        let db = self.db.lock().unwrap();
        db.list_users().map_err(|e| e.to_string())
    }

    fn raw_score(&self, message_id: &str) -> Result<Option<String>, String> {
        let db = self.db.lock().unwrap();
        db.raw_score(message_id).map_err(|e| e.to_string())
    }

    // ── Admin: stats ─────────────────────────────────────────────────────────

    fn stats(&self) -> Result<DbStats, String> {
        let db = self.db.lock().unwrap();
        db.stats().map_err(|e| e.to_string())
    }

    fn get_stats_snapshot(&self, key: &str) -> Result<Option<StatsSnapshot>, String> {
        let db = self.db.lock().unwrap();
        db.get_stats_snapshot(key).map_err(|e| e.to_string())
    }

    fn scores_since(&self, since: &str) -> Result<StatsDelta, String> {
        let db = self.db.lock().unwrap();
        db.scores_since(since).map_err(|e| e.to_string())
    }

    fn upsert_stats_snapshot(&self, key: &str, stats: &DbStats, now: &str) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        db.upsert_stats_snapshot(key, stats, now).map_err(|e| e.to_string())
    }

    // ── Admin: backup ────────────────────────────────────────────────────────

    fn backup(&self, path: &str) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        db.backup(path).map_err(|e| e.to_string())
    }

    // ── Admin: hit list ──────────────────────────────────────────────────────

    fn get_hit_list(&self) -> Result<Vec<(String, String)>, String> {
        let db = self.db.lock().unwrap();
        db.get_hit_list().map_err(|e| e.to_string())
    }

    fn add_to_hit_list(&self, id: &str) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        db.add_to_hit_list(id).map_err(|e| e.to_string())
    }

    fn remove_from_hit_list(&self, id: &str) -> Result<usize, String> {
        let db = self.db.lock().unwrap();
        db.remove_from_hit_list(id).map_err(|e| e.to_string())
    }

    // ── Admin: ban list ───────────────────────────────────────────────────────

    fn ban_user(&self, user_id: &str) -> Result<(), String> {
        let db = self.db.lock().unwrap();
        db.ban_user(user_id).map_err(|e| e.to_string())
    }

    fn unban_user(&self, user_id: &str) -> Result<usize, String> {
        let db = self.db.lock().unwrap();
        db.unban_user(user_id).map_err(|e| e.to_string())
    }

    fn get_banned_users(&self) -> Result<Vec<UserRow>, String> {
        let db = self.db.lock().unwrap();
        db.get_banned_users().map_err(|e| e.to_string())
    }
}

use crate::db::{DbStats, LeaderboardRow, ScoreRow, StatsDelta, StatsSnapshot, UserRow};

pub trait Repository: Send + Sync {
    // ── Read: scores ────────────────────────────────────────────────────────
    fn get_scores(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_today(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_user(&self, user_id: String) -> Result<Vec<ScoreRow>, String>;

    // ── Read: leaderboards ───────────────────────────────────────────────────
    /// Daily leaderboard for a guild on a given date ("YYYY-MM-DD").
    /// Rows are sorted by final_score DESC — position in the vec is rank.
    fn get_daily_leaderboard(&self, guild_id: u64, date: &str) -> Result<Vec<LeaderboardRow>, String>;

    /// All-time average leaderboard for a guild.
    fn get_permanent_leaderboard(&self, guild_id: u64) -> Result<Vec<LeaderboardRow>, String>;

    /// Daily challenge leaderboard for a guild on a given date.
    fn get_daily_challenge_leaderboard(&self, guild_id: u64, date: &str) -> Result<Vec<LeaderboardRow>, String>;

    /// All-time challenge leaderboard for a guild.
    fn get_permanent_challenge_leaderboard(&self, guild_id: u64) -> Result<Vec<LeaderboardRow>, String>;

    /// Weekly leaderboard for a guild across a date range.
    /// week_start / week_end are "YYYY-MM-DD". `use_sum` toggles sum vs avg aggregation.
    fn get_weekly_leaderboard(&self, guild_id: u64, week_start: &str, week_end: &str, use_sum: bool) -> Result<Vec<LeaderboardRow>, String>;

    // ── Admin: score management ──────────────────────────────────────────────
    fn get_score(&self, message_id: &str) -> Result<Option<ScoreRow>, String>;
    fn delete_score(&self, message_id: &str) -> Result<usize, String>;
    fn invalidate_score(&self, message_id: &str) -> Result<usize, String>;
    fn list_scores(&self, user_id: &str) -> Result<Vec<ScoreRow>, String>;
    fn list_users(&self) -> Result<Vec<UserRow>, String>;
    fn raw_score(&self, message_id: &str) -> Result<Option<String>, String>;

    // ── Admin: stats ─────────────────────────────────────────────────────────
    fn stats(&self) -> Result<DbStats, String>;
    fn get_stats_snapshot(&self, key: &str) -> Result<Option<StatsSnapshot>, String>;
    fn scores_since(&self, since: &str) -> Result<StatsDelta, String>;
    fn upsert_stats_snapshot(&self, key: &str, stats: &DbStats, now: &str) -> Result<(), String>;

    // ── Admin: backup ────────────────────────────────────────────────────────
    fn backup(&self, path: &str) -> Result<(), String>;

    // ── Admin: hit list ──────────────────────────────────────────────────────
    fn get_hit_list(&self) -> Result<Vec<(String, String)>, String>;
    fn add_to_hit_list(&self, id: &str) -> Result<(), String>;
    fn remove_from_hit_list(&self, id: &str) -> Result<usize, String>;
}

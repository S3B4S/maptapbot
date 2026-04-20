use crate::db::{LeaderboardRow, ScoreRow};

pub trait Repository: Send + Sync {
    fn get_scores(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_today(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_user(&self, user_id: String) -> Result<Vec<ScoreRow>, String>;

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
}

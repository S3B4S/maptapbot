use crate::db::{LeaderboardRow, ScoreRow};

pub trait Repository: Send + Sync {
    fn get_scores(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_today(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_user(&self, user_id: String) -> Result<Vec<ScoreRow>, String>;

    /// Daily leaderboard for a guild on a given date ("YYYY-MM-DD").
    /// Rows are sorted by final_score DESC — position in the vec is rank.
    fn get_daily_leaderboard(&self, guild_id: u64, date: &str) -> Result<Vec<LeaderboardRow>, String>;

    /// Weekly leaderboard for a guild across a date range (avg scoring).
    /// week_start / week_end are "YYYY-MM-DD". Rows sorted by avg final_score DESC.
    fn get_weekly_leaderboard(&self, guild_id: u64, week_start: &str, week_end: &str) -> Result<Vec<LeaderboardRow>, String>;
}

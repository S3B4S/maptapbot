use crate::{db::ScoreRow};

pub trait Repository: Send + Sync {
    // Should be the ScoreRow as saved in the DB
    fn get_scores(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_today(&self) -> Result<Vec<ScoreRow>, String>;
    fn get_scores_user(&self, user_id: String) -> Result<Vec<ScoreRow>, String>;
}

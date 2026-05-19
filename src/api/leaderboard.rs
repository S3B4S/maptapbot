use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::api::DbState;
use crate::repository::Repository;
use crate::sqlite_repo::SqliteRepository;

#[derive(Deserialize)]
pub struct DailyParams {
    pub guild_id: u64,
    pub date: Option<String>,
}

#[derive(Serialize)]
pub struct LeaderboardEntry {
    pub rank: usize,
    pub user_id: String,
    pub username: String,
    pub score1: Option<f64>,
    pub score2: Option<f64>,
    pub score3: Option<f64>,
    pub score4: Option<f64>,
    pub score5: Option<f64>,
    pub final_score: f64,
}

pub async fn daily(
    State(db): State<DbState>,
    Query(params): Query<DailyParams>,
) -> Result<Json<Vec<LeaderboardEntry>>, (StatusCode, String)> {
    let date = params
        .date
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

    let rows = {
        let repo = SqliteRepository::new(&db);
        repo.get_daily_leaderboard(params.guild_id, &date)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()))?
    };

    let entries = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| LeaderboardEntry {
            rank: i + 1,
            user_id: row.user_id,
            username: row.username,
            score1: row.score1,
            score2: row.score2,
            score3: row.score3,
            score4: row.score4,
            score5: row.score5,
            final_score: row.final_score,
        })
        .collect();

    Ok(Json(entries))
}

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::DbState;
use crate::repository::Repository;
use crate::sqlite_repo::SqliteRepository;

#[derive(Deserialize, IntoParams)]
pub struct DailyParams {
    /// Discord guild (server) ID to scope the leaderboard to.
    pub guild_id: u64,
    /// Date in `YYYY-MM-DD` format. Defaults to today (UTC) when omitted.
    pub date: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct LeaderboardEntry {
    /// 1-based rank position on the leaderboard.
    pub rank: usize,
    /// Discord user ID.
    pub user_id: String,
    /// Discord display name at the time the score was posted.
    pub username: String,
    pub score1: Option<f64>,
    pub score2: Option<f64>,
    pub score3: Option<f64>,
    pub score4: Option<f64>,
    pub score5: Option<f64>,
    /// Weighted final score: `(s1+s2)*1 + s3*2 + (s4+s5)*3`.
    pub final_score: f64,
}

/// Daily leaderboard for a guild on a given date.
///
/// Returns entries sorted by `final_score` descending. The `rank` field is
/// 1-based and matches the array position.
#[utoipa::path(
    get,
    path = "/leaderboard/daily",
    params(DailyParams),
    responses(
        (status = 200, description = "Ranked leaderboard entries", body = Vec<LeaderboardEntry>),
        (status = 429, description = "Rate limit exceeded"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Leaderboard"
)]
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

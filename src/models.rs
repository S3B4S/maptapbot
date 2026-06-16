use chrono::{DateTime, NaiveDate, Utc};

#[derive(Debug, Clone, PartialEq)]
pub enum GameMode {
    DailyDefault,
    DailyChallenge,
    Frontier,
}

impl GameMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameMode::DailyDefault => "daily_default",
            GameMode::DailyChallenge => "daily_challenge",
            GameMode::Frontier => "frontier",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "daily_default" => Some(GameMode::DailyDefault),
            "daily_challenge" => Some(GameMode::DailyChallenge),
            "frontier" => Some(GameMode::Frontier),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaptapScore {
    pub message_id: u64,
    pub channel_id: u64,
    pub channel_parent_id: Option<u64>, // Some(id) if posted in a thread; None otherwise
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub mode: GameMode,
    pub time_spent_ms: Option<u32>, // None for DailyDefault; required for DailyChallenge and Frontier
    pub date: NaiveDate,
    pub scores: [Option<u32>; 5],
    pub final_score: u32,
    pub raw_message: String,
    pub posted_at: DateTime<Utc>,
    /// Frontier mode only: level reached
    pub frontier_level: Option<u32>,
    /// Frontier mode only: rounds played
    pub frontier_rounds: Option<u32>,
    /// Frontier mode only: location where the run ended ("Fell at ...")
    pub frontier_location: Option<String>,
}

impl MaptapScore {
    /// Validate all constraints. Behavior depends on `mode`:
    /// - DailyDefault: each score 0–100 (no None), final ≤ 1000, formula match.
    /// - DailyChallenge: same as DailyDefault but `--` (None) is allowed per tile.
    /// - Frontier: no per-tile scores or formula; no upper cap on final_score;
    ///   level/rounds/location and time_spent_ms must be present.
    pub fn validate(&self) -> Result<(), String> {
        if self.mode == GameMode::Frontier {
            return self.validate_frontier();
        }

        for (i, &score) in self.scores.iter().enumerate() {
            match score {
                Some(v) if v > 100 => {
                    return Err(format!("Score {} is {} (must be 0-100)", i + 1, v));
                }
                None if self.mode != GameMode::DailyChallenge => {
                    return Err(format!(
                        "Score {} is missing (-- is only valid in challenge mode)",
                        i + 1
                    ));
                }
                _ => {}
            }
        }

        if self.final_score > 1000 {
            return Err(format!(
                "Final score {} exceeds maximum of 1000",
                self.final_score
            ));
        }

        let expected = self.compute_final_score();
        if self.final_score != expected {
            return Err(format!(
                "Final score mismatch: reported {} but computed {} from formula \
                 (s1+s2)*1 + s3*2 + (s4+s5)*3",
                self.final_score, expected
            ));
        }

        Ok(())
    }

    fn validate_frontier(&self) -> Result<(), String> {
        if self.scores.iter().any(|s| s.is_some()) {
            return Err("Frontier scores must not have per-tile scores".to_string());
        }
        let level = self
            .frontier_level
            .ok_or_else(|| "Frontier score is missing level".to_string())?;
        if level < 1 {
            return Err(format!("Frontier level must be >= 1 (got {})", level));
        }
        self.frontier_rounds
            .ok_or_else(|| "Frontier score is missing rounds".to_string())?;
        let loc = self
            .frontier_location
            .as_deref()
            .ok_or_else(|| "Frontier score is missing location".to_string())?;
        if loc.trim().is_empty() {
            return Err("Frontier location must be non-empty".to_string());
        }
        self.time_spent_ms
            .ok_or_else(|| "Frontier score is missing time_spent_ms".to_string())?;
        Ok(())
    }

    /// (s1 + s2) * 1 + s3 * 2 + (s4 + s5) * 3
    /// None scores (timed-out tiles) are treated as 0.
    /// Not meaningful for Frontier mode.
    pub fn compute_final_score(&self) -> u32 {
        let [s1, s2, s3, s4, s5] = self.scores.map(|s| s.unwrap_or(0));
        (s1 + s2) + s3 * 2 + (s4 + s5) * 3
    }
}

#[cfg(test)]
#[path = "tests/models.rs"]
mod tests;

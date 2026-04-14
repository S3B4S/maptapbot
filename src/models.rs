use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub enum GameMode {
    DailyDefault,
    DailyChallenge,
}

impl GameMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameMode::DailyDefault => "daily_default",
            GameMode::DailyChallenge => "daily_challenge",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "daily_default" => Some(GameMode::DailyDefault),
            "daily_challenge" => Some(GameMode::DailyChallenge),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaptapScore {
    pub message_id: u64,
    pub channel_id: u64,
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub mode: GameMode,
    pub time_spent_ms: Option<u32>, // None for DailyDefault
    pub date: NaiveDate,
    pub scores: [Option<u32>; 5],
    pub final_score: u32,
    pub raw_message: String,
}

impl MaptapScore {
    /// Validate all constraints from the spec:
    /// - Each score 0-100 inclusive (None = timed-out tile, only valid for DailyChallenge)
    /// - Final score <= 1000
    /// - Final score matches formula: (s1+s2)*1 + s3*2 + (s4+s5)*3
    pub fn validate(&self) -> Result<(), String> {
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

    /// (s1 + s2) * 1 + s3 * 2 + (s4 + s5) * 3
    /// None scores (timed-out tiles) are treated as 0.
    pub fn compute_final_score(&self) -> u32 {
        let [s1, s2, s3, s4, s5] = self.scores.map(|s| s.unwrap_or(0));
        (s1 + s2) + s3 * 2 + (s4 + s5) * 3
    }
}

#[cfg(test)]
#[path = "tests/models.rs"]
mod tests;

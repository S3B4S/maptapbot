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
mod tests {
    use super::*;

    fn make_score(scores: [Option<u32>; 5], final_score: u32) -> MaptapScore {
        MaptapScore {
            user_id: 1,
            guild_id: Some(100),
            mode: GameMode::DailyDefault,
            time_spent_ms: None,
            date: NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(),
            scores,
            final_score,
            raw_message: String::new(),
        }
    }

    #[test]
    fn test_valid_score() {
        // From spec example: 93 90 83 61 97 -> (93+90)*1 + 83*2 + (61+97)*3
        // = 183 + 166 + 474 = 823
        let s = make_score([Some(93), Some(90), Some(83), Some(61), Some(97)], 823);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn test_score_out_of_range() {
        let s = make_score([Some(101), Some(90), Some(83), Some(61), Some(97)], 823);
        assert!(s.validate().is_err());
    }

    #[test]
    fn test_final_score_mismatch() {
        let s = make_score([Some(93), Some(90), Some(83), Some(61), Some(97)], 999);
        let err = s.validate().unwrap_err();
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_final_score_exceeds_max() {
        let s = make_score(
            [Some(100), Some(100), Some(100), Some(100), Some(100)],
            1001,
        );
        let err = s.validate().unwrap_err();
        assert!(err.contains("exceeds"));
    }

    #[test]
    fn test_compute_formula() {
        let s = make_score([Some(93), Some(90), Some(83), Some(61), Some(97)], 0);
        assert_eq!(s.compute_final_score(), 823);
    }

    #[test]
    fn test_game_mode_round_trip() {
        assert_eq!(
            GameMode::from_str("daily_default"),
            Some(GameMode::DailyDefault)
        );
        assert_eq!(
            GameMode::from_str("daily_challenge"),
            Some(GameMode::DailyChallenge)
        );
        assert_eq!(GameMode::DailyChallenge.as_str(), "daily_challenge");
    }

    #[test]
    fn test_none_score_valid_in_challenge_mode() {
        // 96 + 4 + 68 + 91 + 0(None) -> (96+4)*1 + 68*2 + (91+0)*3 = 100 + 136 + 273 = 509
        let s = MaptapScore {
            user_id: 1,
            guild_id: Some(100),
            mode: GameMode::DailyChallenge,
            time_spent_ms: Some(25000),
            date: chrono::NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(),
            scores: [Some(96), Some(4), Some(68), Some(91), None],
            final_score: 509,
            raw_message: String::new(),
        };
        assert!(s.validate().is_ok());
        assert_eq!(s.compute_final_score(), 509);
    }

    #[test]
    fn test_none_score_invalid_in_daily_default() {
        let s = MaptapScore {
            user_id: 1,
            guild_id: Some(100),
            mode: GameMode::DailyDefault,
            time_spent_ms: None,
            date: chrono::NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(),
            scores: [Some(93), Some(90), Some(83), Some(61), None],
            final_score: 0,
            raw_message: String::new(),
        };
        let err = s.validate().unwrap_err();
        assert!(err.contains("challenge mode"));
    }
}

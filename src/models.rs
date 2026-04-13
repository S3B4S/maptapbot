use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct MaptapScore {
    pub user_id: u64,
    pub date: NaiveDate,
    pub scores: [u32; 5],
    pub final_score: u32,
    pub raw_message: String,
}

impl MaptapScore {
    /// Validate all constraints from the spec:
    /// - Each score 0-100 inclusive
    /// - Final score <= 1000
    /// - Final score matches formula: (s1+s2)*1 + s3*2 + (s4+s5)*3
    pub fn validate(&self) -> Result<(), String> {
        for (i, &score) in self.scores.iter().enumerate() {
            if score > 100 {
                return Err(format!("Score {} is {} (must be 0-100)", i + 1, score));
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
    pub fn compute_final_score(&self) -> u32 {
        let [s1, s2, s3, s4, s5] = self.scores;
        (s1 + s2) + s3 * 2 + (s4 + s5) * 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_score(scores: [u32; 5], final_score: u32) -> MaptapScore {
        MaptapScore {
            user_id: 1,
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
        let s = make_score([93, 90, 83, 61, 97], 823);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn test_score_out_of_range() {
        let s = make_score([101, 90, 83, 61, 97], 823);
        assert!(s.validate().is_err());
    }

    #[test]
    fn test_final_score_mismatch() {
        let s = make_score([93, 90, 83, 61, 97], 999);
        let err = s.validate().unwrap_err();
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_final_score_exceeds_max() {
        let s = make_score([100, 100, 100, 100, 100], 1001);
        let err = s.validate().unwrap_err();
        assert!(err.contains("exceeds"));
    }

    #[test]
    fn test_compute_formula() {
        let s = make_score([93, 90, 83, 61, 97], 0);
        assert_eq!(s.compute_final_score(), 823);
    }
}

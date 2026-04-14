use chrono::NaiveDate;

use super::*;

fn make_score(scores: [Option<u32>; 5], final_score: u32) -> MaptapScore {
    MaptapScore {
        message_id: 1,
        channel_id: 1,
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
        message_id: 1,
        channel_id: 1,
        user_id: 1,
        guild_id: Some(100),
        mode: GameMode::DailyChallenge,
        time_spent_ms: Some(25000),
        date: NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(),
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
        message_id: 1,
        channel_id: 1,
        user_id: 1,
        guild_id: Some(100),
        mode: GameMode::DailyDefault,
        time_spent_ms: None,
        date: NaiveDate::from_ymd_opt(2026, 4, 13).unwrap(),
        scores: [Some(93), Some(90), Some(83), Some(61), None],
        final_score: 0,
        raw_message: String::new(),
    };
    let err = s.validate().unwrap_err();
    assert!(err.contains("challenge mode"));
}

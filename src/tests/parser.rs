use chrono::{Datelike, Utc};

use super::*;

const G: Option<u64> = Some(100);

// ── Basic valid ──────────────────────────────────────────────

#[test]
fn test_parse_valid_message() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let result = parse_maptap_message(12345, G, msg);
    assert!(result.is_some());
    let score = result.unwrap().unwrap();
    assert_eq!(
        score.scores,
        [Some(93), Some(90), Some(83), Some(61), Some(97)]
    );
    assert_eq!(score.final_score, 823);
    assert_eq!(score.date.month(), 4);
    assert_eq!(score.date.day(), 13);
    assert_eq!(score.guild_id, G);
    assert_eq!(score.mode, GameMode::DailyDefault);
    assert_eq!(score.time_spent_ms, None);
}

#[test]
fn test_parse_not_maptap() {
    let msg = "hello world";
    assert!(parse_maptap_message(1, G, msg).is_none());
}

// ── Validation errors ────────────────────────────────────────

#[test]
fn test_parse_wrong_score_count() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁\nFinal score: 823";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

#[test]
fn test_parse_final_score_mismatch() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 999";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    let err = result.unwrap().unwrap_err();
    assert!(err.contains("mismatch"));
}

#[test]
fn test_parse_score_out_of_range() {
    let msg = "www.maptap.gg April 13\n150🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    let err = result.unwrap().unwrap_err();
    assert!(err.contains("must be 0-100"));
}

// ── Text BEFORE the block (allowed) ──────────────────────────

#[test]
fn test_text_before_on_separate_line() {
    let msg =
        "this is horrible\nwww.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.final_score, 823);
}

#[test]
fn test_text_before_on_same_line() {
    let msg = "this is horrible www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.final_score, 823);
}

// ── Text AFTER the block (allowed) ───────────────────────────

#[test]
fn test_text_after_on_separate_line() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823\nthis is amazing";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.final_score, 823);
}

#[test]
fn test_text_after_on_same_line_as_final_score() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823 this is amazing";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.final_score, 823);
}

// ── Invalid: text interrupting the 3 lines ──────────────────

#[test]
fn test_invalid_text_after_header_on_same_line() {
    let msg = "www.maptap.gg April 13 This sucks\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    assert!(parse_maptap_message(1, G, msg).is_none());
}

#[test]
fn test_invalid_text_after_scores_on_same_line() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥 wow I did so well today\nFinal score: 823";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

#[test]
fn test_invalid_text_before_scores_on_same_line() {
    let msg =
        "www.maptap.gg April 13\nnahh I'm embarrassed 93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

// ── Raw message safety ───────────────────────────────────────

#[test]
fn test_raw_message_exact_content() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert_eq!(
        score.raw_message,
        "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823"
    );
}

#[test]
fn test_raw_message_excludes_surrounding_text() {
    let msg = "Hey everyone!\nwww.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823\nSee you tomorrow";
    let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
    assert!(!score.raw_message.contains("Hey everyone"));
    assert!(!score.raw_message.contains("See you tomorrow"));
}

// ── Edge: not enough lines after header ──────────────────────

#[test]
fn test_header_not_enough_lines_after() {
    let msg = "some text\nmore text\nwww.maptap.gg April 13";
    assert!(parse_maptap_message(1, G, msg).is_none());
}

// ── Challenge mode ───────────────────────────────────────────

#[test]
fn test_parse_valid_challenge() {
    let msg = "⚡ MapTap Challenge Round - Apr 12\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s (4.0s to spare!)";
    let result = parse_challenge_message(1, G, msg);
    assert!(result.is_some(), "expected Some, got None");
    let score = result.unwrap().unwrap();
    assert_eq!(
        score.scores,
        [Some(89), Some(82), Some(94), Some(88), Some(97)]
    );
    assert_eq!(score.final_score, 914);
    assert_eq!(score.date.month(), 4);
    assert_eq!(score.date.day(), 12);
    assert_eq!(score.mode, GameMode::DailyChallenge);
    assert_eq!(score.time_spent_ms, Some(21100));
}

#[test]
fn test_challenge_time_parsing() {
    // 21.1s → 21100ms
    let msg = "⚡ MapTap Challenge Round - Apr 12\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s";
    let score = parse_challenge_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.time_spent_ms, Some(21100));
}

#[test]
fn test_challenge_formula_validation() {
    // (89+82)*1 + 94*2 + (88+97)*3 = 171 + 188 + 555 = 914
    let msg = "⚡ MapTap Challenge Round - Apr 12\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s";
    let score = parse_challenge_message(1, G, msg).unwrap().unwrap();
    assert_eq!(score.compute_final_score(), 914);
}

#[test]
fn test_challenge_score_mismatch() {
    let msg = "⚡ MapTap Challenge Round - Apr 12\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 999 in 21.1s";
    let result = parse_challenge_message(1, G, msg).unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("mismatch"));
}

#[test]
fn test_challenge_not_matched_by_default_parser() {
    let msg = "⚡ MapTap Challenge Round - Apr 12\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s";
    assert!(parse_maptap_message(1, G, msg).is_none());
}

#[test]
fn test_default_not_matched_by_challenge_parser() {
    let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
    assert!(parse_challenge_message(1, G, msg).is_none());
}

#[test]
fn test_parse_challenge_timed_out() {
    // Spec example: 96🏅 4🤮 68🙂 91🎉 -- → last score is None
    // (96+4)*1 + 68*2 + (91+0)*3 = 100 + 136 + 273 = 509
    let msg = "⚡ MapTap Challenge Round - Apr 13\nwww.maptap.gg/challenge\n96🏅 4🤮 68🙂 91🎉 --\nScore: 509 in 25.0s (TIME UP!)";
    let result = parse_challenge_message(1, G, msg);
    assert!(result.is_some(), "expected Some, got None");
    let score = result.unwrap().unwrap();
    assert_eq!(score.scores, [Some(96), Some(4), Some(68), Some(91), None]);
    assert_eq!(score.final_score, 509);
    assert_eq!(score.time_spent_ms, Some(25000));
}

#[test]
fn test_parse_scores_line_with_dash() {
    // -- token should parse as None
    let line = "96🏅 4🤮 68🙂 91🎉 --";
    let result = parse_scores_line(line).unwrap();
    assert_eq!(result, [Some(96), Some(4), Some(68), Some(91), None]);
}

#[test]
fn test_parse_scores_line_invalid_token() {
    // Something other than digits or -- is a parse failure
    let line = "96🏅 4🤮 68🙂 91🎉 xx";
    assert!(parse_scores_line(line).is_err());
}

#[test]
fn test_dash_score_rejected_in_daily_default() {
    // -- in a daily default message should fail validation
    let msg = "www.maptap.gg April 13\n96🏆 4👑 68😁 91🫢 --\nFinal score: 509";
    let result = parse_maptap_message(1, G, msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_err());
}

#[test]
fn test_parse_month_abbreviated() {
    assert_eq!(parse_month("Jan"), Some(1));
    assert_eq!(parse_month("Apr"), Some(4));
    assert_eq!(parse_month("Sep"), Some(9));
    assert_eq!(parse_month("Dec"), Some(12));
}

#[test]
fn test_parse_month_full() {
    assert_eq!(parse_month("January"), Some(1));
    assert_eq!(parse_month("April"), Some(4));
}

// ── Future date rejection ────────────────────────────────────

#[test]
fn test_daily_future_date_rejected() {
    // Build a message with 2 days from now to ensure it's always beyond the allowed +1 day.
    let future = (Utc::now() + chrono::Duration::days(2)).date_naive();
    let month_name = future.format("%B").to_string(); // e.g. "April"
    let day = future.day();
    // Compute a valid score for the formula: s1=10,s2=10,s3=10,s4=10,s5=10 → (10+10)*1 + 10*2 + (10+10)*3 = 20+20+60 = 100
    let msg = format!(
        "www.maptap.gg {} {}\n10🏆 10👑 10😁 10🫢 10🔥\nFinal score: 100",
        month_name, day
    );
    let result = parse_maptap_message(1, G, &msg);
    assert!(result.is_some(), "should be recognized as a maptap block");
    let err = result.unwrap().unwrap_err();
    assert!(
        err.contains("future"),
        "expected future-date error, got: {err}"
    );
}

#[test]
fn test_challenge_future_date_rejected() {
    let future = (Utc::now() + chrono::Duration::days(2)).date_naive();
    let month_abbr = future.format("%b").to_string(); // e.g. "Apr"
    let day = future.day();
    // s1=89,s2=82,s3=94,s4=88,s5=97 → (89+82)*1 + 94*2 + (88+97)*3 = 171+188+555 = 914
    let msg = format!(
        "⚡ MapTap Challenge Round - {} {}\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s (4.0s to spare!)",
        month_abbr, day
    );
    let result = parse_challenge_message(1, G, &msg);
    assert!(
        result.is_some(),
        "should be recognized as a challenge block"
    );
    let err = result.unwrap().unwrap_err();
    assert!(
        err.contains("future"),
        "expected future-date error, got: {err}"
    );
}

#[test]
fn test_daily_today_date_accepted() {
    // Today's date must not be rejected as future
    let today = Utc::now().date_naive();
    let month_name = today.format("%B").to_string();
    let day = today.day();
    let msg = format!(
        "www.maptap.gg {} {}\n10🏆 10👑 10😁 10🫢 10🔥\nFinal score: 100",
        month_name, day
    );
    let result = parse_maptap_message(1, G, &msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_ok(), "today's date should be accepted");
}

#[test]
fn test_challenge_today_date_accepted() {
    let today = Utc::now().date_naive();
    let month_abbr = today.format("%b").to_string();
    let day = today.day();
    let msg = format!(
        "⚡ MapTap Challenge Round - {} {}\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s (4.0s to spare!)",
        month_abbr, day
    );
    let result = parse_challenge_message(1, G, &msg);
    assert!(result.is_some());
    assert!(result.unwrap().is_ok(), "today's date should be accepted");
}

#[test]
fn test_daily_tomorrow_date_accepted() {
    // Tomorrow (+1 day) must be accepted to accommodate users in timezones ahead of the server.
    let tomorrow = (Utc::now() + chrono::Duration::days(1)).date_naive();
    let month_name = tomorrow.format("%B").to_string();
    let day = tomorrow.day();
    let msg = format!(
        "www.maptap.gg {} {}\n10🏆 10👑 10😁 10🫢 10🔥\nFinal score: 100",
        month_name, day
    );
    let result = parse_maptap_message(1, G, &msg);
    assert!(result.is_some());
    assert!(
        result.unwrap().is_ok(),
        "tomorrow's date should be accepted"
    );
}

#[test]
fn test_challenge_tomorrow_date_accepted() {
    // Tomorrow (+1 day) must be accepted to accommodate users in timezones ahead of the server.
    let tomorrow = (Utc::now() + chrono::Duration::days(1)).date_naive();
    let month_abbr = tomorrow.format("%b").to_string();
    let day = tomorrow.day();
    let msg = format!(
        "⚡ MapTap Challenge Round - {} {}\nwww.maptap.gg/challenge\n89🎉 82✨ 94🏆 88🎓 97🏅\nScore: 914 in 21.1s (4.0s to spare!)",
        month_abbr, day
    );
    let result = parse_challenge_message(1, G, &msg);
    assert!(result.is_some());
    assert!(
        result.unwrap().is_ok(),
        "tomorrow's date should be accepted"
    );
}

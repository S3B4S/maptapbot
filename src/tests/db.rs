use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::models::GameMode;
use chrono::{NaiveDate, Utc};

/// Auto-incrementing counter to generate unique message IDs for tests.
/// Starts at 17 digits to mimic Discord snowflakes — keeps lex sort of
/// `message_id` (TEXT in SQLite) in agreement with numeric order, which
/// matters because the leaderboard tie-break is `ORDER BY message_id DESC`.
static NEXT_MSG_ID: AtomicU64 = AtomicU64::new(10_000_000_000_000_000);

fn test_db() -> Database {
    Database::open(":memory:").unwrap()
}

fn make_score(
    user_id: u64,
    guild_id: u64,
    day: u32,
    scores: [Option<u32>; 5],
    final_score: u32,
    mode: GameMode,
    time_spent_ms: Option<u32>,
) -> MaptapScore {
    MaptapScore {
        message_id: NEXT_MSG_ID.fetch_add(1, Ordering::Relaxed),
        channel_id: 500,
        channel_parent_id: None,
        user_id,
        guild_id: Some(guild_id),
        mode,
        time_spent_ms,
        date: NaiveDate::from_ymd_opt(2026, 4, day).unwrap(),
        scores,
        final_score,
        raw_message: "test".to_string(),
        posted_at: Utc::now(),
    }
}

/// Helper: upsert user then default-mode score. Returns the inserted message_id
/// so tests can address the row (e.g. for invalidate).
fn insert_score(
    db: &Database,
    user_id: u64,
    guild_id: u64,
    day: u32,
    scores: [Option<u32>; 5],
    final_score: u32,
) -> u64 {
    db.upsert_user(user_id, &format!("user{}", user_id))
        .unwrap();
    let score = make_score(
        user_id,
        guild_id,
        day,
        scores,
        final_score,
        GameMode::DailyDefault,
        None,
    );
    let msg_id = score.message_id;
    db.insert_score(&score).unwrap();
    msg_id
}

/// Helper: upsert user then challenge-mode score
fn insert_challenge_score(
    db: &Database,
    user_id: u64,
    guild_id: u64,
    day: u32,
    scores: [Option<u32>; 5],
    final_score: u32,
    time_spent_ms: u32,
) -> u64 {
    db.upsert_user(user_id, &format!("user{}", user_id))
        .unwrap();
    let score = make_score(
        user_id,
        guild_id,
        day,
        scores,
        final_score,
        GameMode::DailyChallenge,
        Some(time_spent_ms),
    );
    let msg_id = score.message_id;
    db.insert_score(&score).unwrap();
    msg_id
}

#[test]
fn test_upsert_user() {
    let db = test_db();
    db.upsert_user(1, "alice").unwrap();
    db.upsert_user(1, "alice_renamed").unwrap();

    let name: String = db
        .conn
        .query_row(
            "SELECT username FROM users WHERE user_id = '1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(name, "alice_renamed");
}

#[test]
fn test_insert_and_query() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );

    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].final_score, 823.0);
    assert_eq!(results[0].username, "user1");
    assert_eq!(results[0].time_spent_ms, None);
}

#[test]
fn test_append_only_latest_post_wins() {
    // Two posts for the same (user, guild, date, mode) create two rows;
    // the leaderboard shows the later (higher message_id ⇒ later posted_at)
    // one. The earlier row is preserved — that's the whole point of the
    // append-only model.
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
    );
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );

    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].final_score, 823.0);

    // Both rows should still exist in the table.
    let count: i64 = db
        .conn
        .query_row(
            "SELECT COUNT(*) FROM scores WHERE user_id = '1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_multiple_users_daily() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );
    insert_score(
        &db,
        2,
        100,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
    );

    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].final_score, 823.0);
    assert_eq!(results[1].final_score, 600.0);
}

#[test]
fn test_guild_scoping() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );
    insert_score(
        &db,
        2,
        200,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
    );

    assert_eq!(
        db.get_daily_leaderboard(100, "2026-04-13").unwrap().len(),
        1
    );
    assert_eq!(
        db.get_daily_leaderboard(200, "2026-04-13").unwrap().len(),
        1
    );
}

#[test]
fn test_permanent_leaderboard_averages() {
    let db = test_db();
    // User 1: two days, scores 800 and 600 -> avg 700
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(80), Some(80), Some(80), Some(80), Some(80)],
        800,
    );
    insert_score(
        &db,
        1,
        100,
        14,
        [Some(60), Some(60), Some(60), Some(60), Some(60)],
        600,
    );
    // User 2: one day, score 750
    insert_score(
        &db,
        2,
        100,
        13,
        [Some(75), Some(75), Some(75), Some(75), Some(75)],
        750,
    );

    let results = db.get_permanent_leaderboard(100).unwrap();
    assert_eq!(results.len(), 2);
    // User 2 (avg 750) first, user 1 (avg 700) second
    assert_eq!(results[0].user_id, "2");
    assert_eq!(results[0].username, "user2");
    assert_eq!(results[0].final_score, 750.0);
    assert_eq!(results[1].user_id, "1");
    assert_eq!(results[1].username, "user1");
    assert_eq!(results[1].final_score, 700.0);
}

#[test]
fn test_username_update_reflected_in_leaderboard() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );

    // Rename user
    db.upsert_user(1, "new_name").unwrap();

    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results[0].username, "new_name");
}

// ── Challenge leaderboard tests ──────────────────────────────

#[test]
fn test_challenge_daily_leaderboard() {
    let db = test_db();
    // user1: score 914 in 21100ms
    insert_challenge_score(
        &db,
        1,
        100,
        13,
        [Some(89), Some(82), Some(94), Some(88), Some(97)],
        914,
        21100,
    );
    // user2: same score but slower
    insert_challenge_score(
        &db,
        2,
        100,
        13,
        [Some(89), Some(82), Some(94), Some(88), Some(97)],
        914,
        25000,
    );

    let results = db
        .get_daily_challenge_leaderboard(100, "2026-04-13")
        .unwrap();
    assert_eq!(results.len(), 2);
    // Same final score → ordered by time (faster first)
    assert_eq!(results[0].user_id, "1");
    assert_eq!(results[0].time_spent_ms, Some(21100.0));
    assert_eq!(results[1].user_id, "2");
}

#[test]
fn test_challenge_default_scores_not_mixed() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );
    insert_challenge_score(
        &db,
        2,
        100,
        13,
        [Some(89), Some(82), Some(94), Some(88), Some(97)],
        914,
        21100,
    );

    let default_results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(default_results.len(), 1);
    assert_eq!(default_results[0].user_id, "1");

    let challenge_results = db
        .get_daily_challenge_leaderboard(100, "2026-04-13")
        .unwrap();
    assert_eq!(challenge_results.len(), 1);
    assert_eq!(challenge_results[0].user_id, "2");
}

#[test]
fn test_challenge_permanent_leaderboard_averages() {
    let db = test_db();
    // user1: two days, 914 and 800
    insert_challenge_score(
        &db,
        1,
        100,
        12,
        [Some(89), Some(82), Some(94), Some(88), Some(97)],
        914,
        21100,
    );
    insert_challenge_score(
        &db,
        1,
        100,
        13,
        [Some(80), Some(80), Some(80), Some(80), Some(80)],
        800,
        30000,
    );
    // user2: one day, 900
    insert_challenge_score(
        &db,
        2,
        100,
        12,
        [Some(85), Some(85), Some(90), Some(85), Some(90)],
        900,
        18000,
    );

    let results = db.get_permanent_challenge_leaderboard(100).unwrap();
    assert_eq!(results.len(), 2);
    // user2 (avg 900) > user1 (avg 857)
    assert_eq!(results[0].user_id, "2");
    assert_eq!(results[0].time_spent_ms, Some(18000.0));
    assert_eq!(results[1].user_id, "1");
    // avg time for user1: (21100 + 30000) / 2 = 25550
    assert_eq!(results[1].time_spent_ms, Some(25550.0));
}

#[test]
fn test_same_user_can_have_both_modes_same_day() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );
    insert_challenge_score(
        &db,
        1,
        100,
        13,
        [Some(89), Some(82), Some(94), Some(88), Some(97)],
        914,
        21100,
    );

    let default_results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(default_results.len(), 1);
    assert_eq!(default_results[0].final_score, 823.0);

    let challenge_results = db
        .get_daily_challenge_leaderboard(100, "2026-04-13")
        .unwrap();
    assert_eq!(challenge_results.len(), 1);
    assert_eq!(challenge_results[0].final_score, 914.0);
}

#[test]
fn test_challenge_null_score_stored_and_retrieved() {
    let db = test_db();
    // Timed-out tile: last score is None
    // (96+4)*1 + 68*2 + (91+0)*3 = 509
    insert_challenge_score(
        &db,
        1,
        100,
        13,
        [Some(96), Some(4), Some(68), Some(91), None],
        509,
        25000,
    );

    let results = db
        .get_daily_challenge_leaderboard(100, "2026-04-13")
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].score5, None);
    assert_eq!(results[0].final_score, 509.0);
}

#[test]
fn test_challenge_null_score_averaged_as_zero() {
    let db = test_db();
    // Day 1: all scores present — score5 = 80
    insert_challenge_score(
        &db,
        1,
        100,
        12,
        [Some(80), Some(80), Some(80), Some(80), Some(80)],
        800,
        20000,
    );
    // Day 2: score5 timed out (None = 0 for avg)
    // (96+4)*1 + 68*2 + (91+0)*3 = 509
    insert_challenge_score(
        &db,
        1,
        100,
        13,
        [Some(96), Some(4), Some(68), Some(91), None],
        509,
        25000,
    );

    let results = db.get_permanent_challenge_leaderboard(100).unwrap();
    assert_eq!(results.len(), 1);
    // avg score5: (80 + 0) / 2 = 40.0
    assert_eq!(results[0].score5, Some(40.0));
}

// ── Message ID tests ──────────────────────────────

#[test]
fn test_message_id_stored_as_pk() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );

    // Verify message_id and channel_id are stored.
    let row: (String, String) = db
        .conn
        .query_row(
            "SELECT message_id, channel_id FROM scores WHERE user_id = '1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    // message_id should be a stringified u64, not a legacy- prefix
    assert!(!row.0.starts_with("legacy-"));
    assert_eq!(row.1, "500"); // channel_id from make_score
}

#[test]
fn test_reparse_same_message_updates_in_place() {
    // Re-processing the same Discord message (same message_id) — e.g. via
    // /parse — updates the existing row rather than creating a new one.
    let db = test_db();
    db.upsert_user(1, "user1").unwrap();

    let first = make_score(
        1,
        100,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
        GameMode::DailyDefault,
        None,
    );
    let msg_id = first.message_id;
    db.insert_score(&first).unwrap();

    // Same message_id, different parsed content (as if user edited the message
    // and we re-parsed it).
    let second = MaptapScore {
        message_id: msg_id,
        scores: [Some(93), Some(90), Some(83), Some(61), Some(97)],
        final_score: 823,
        ..make_score(
            1,
            100,
            13,
            [Some(0), Some(0), Some(0), Some(0), Some(0)],
            0,
            GameMode::DailyDefault,
            None,
        )
    };
    db.insert_score(&second).unwrap();

    let count: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM scores", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results[0].final_score, 823.0);
}

#[test]
fn test_invalidate_falls_back_to_prior_valid_score() {
    // The headline bug-fix: user posts a legit score, then posts an
    // overwriting wrong score. Admin invalidates the wrong one. The legit
    // score (the earlier row) becomes the effective score for the day.
    let db = test_db();
    let legit_msg_id = insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );
    let wrong_msg_id = insert_score(
        &db,
        1,
        100,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
    );

    // Before invalidation: the wrong (later) score is effective.
    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results[0].final_score, 600.0);

    // Admin invalidates the wrong one.
    let updated = db.invalidate_score(&wrong_msg_id.to_string()).unwrap();
    assert_eq!(updated, 1);

    // After invalidation: the legit (earlier, still-valid) score wins.
    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].final_score, 823.0);

    // Both rows still exist in the table; only `invalid` differs.
    let (total, invalid): (i64, i64) = db
        .conn
        .query_row(
            "SELECT COUNT(*), SUM(invalid) FROM scores WHERE user_id = '1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(total, 2);
    assert_eq!(invalid, 1);

    // Sanity: invalidating the legit row too drops the user from the board.
    db.invalidate_score(&legit_msg_id.to_string()).unwrap();
    let results = db.get_daily_leaderboard(100, "2026-04-13").unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn test_permanent_leaderboard_ignores_invalidated_rows() {
    // Each day's effective score is the latest *valid* row. Permanent
    // average must be taken over effective-per-day rows, not raw rows —
    // otherwise invalidated throwaway posts pollute the average.
    let db = test_db();
    // Day 1: legit 800, then a throwaway 400 that gets invalidated.
    insert_score(
        &db,
        1,
        100,
        12,
        [Some(80), Some(80), Some(80), Some(80), Some(80)],
        800,
    );
    let throwaway = insert_score(
        &db,
        1,
        100,
        12,
        [Some(40), Some(40), Some(40), Some(40), Some(40)],
        400,
    );
    db.invalidate_score(&throwaway.to_string()).unwrap();
    // Day 2: 600.
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(60), Some(60), Some(60), Some(60), Some(60)],
        600,
    );

    let results = db.get_permanent_leaderboard(100).unwrap();
    assert_eq!(results.len(), 1);
    // avg over effective-per-day = (800 + 600) / 2 = 700.
    assert_eq!(results[0].final_score, 700.0);
}

#[test]
fn test_invalidate_preserved_across_reparse() {
    // Re-processing an already-invalidated message must not un-invalidate it.
    let db = test_db();
    db.upsert_user(1, "user1").unwrap();

    let score = make_score(
        1,
        100,
        13,
        [Some(50), Some(50), Some(50), Some(50), Some(50)],
        600,
        GameMode::DailyDefault,
        None,
    );
    let msg_id = score.message_id;
    db.insert_score(&score).unwrap();
    db.invalidate_score(&msg_id.to_string()).unwrap();

    // Re-parse the same Discord message (same message_id).
    db.insert_score(&score).unwrap();

    let invalid: i64 = db
        .conn
        .query_row(
            "SELECT invalid FROM scores WHERE message_id = ?1",
            [msg_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(invalid, 1);
}

#[test]
fn test_list_scores_includes_message_id() {
    let db = test_db();
    insert_score(
        &db,
        1,
        100,
        13,
        [Some(93), Some(90), Some(83), Some(61), Some(97)],
        823,
    );

    let rows = db.list_scores("1").unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].message_id.starts_with("legacy-"));
    assert_eq!(rows[0].channel_id, Some("500".to_string()));
}

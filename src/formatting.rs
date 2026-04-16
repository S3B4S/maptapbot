/// Truncate a username to `max_len` characters, appending ".." if truncated.
pub fn truncate_username(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        let mut truncated = name[..max_len - 2].to_string();
        truncated.push_str("..");
        truncated
    }
}

/// Human-readable title for a leaderboard command.
pub fn leaderboard_title(name: &str) -> &'static str {
    match name {
        "leaderboard_daily" => "Daily Leaderboard",
        "leaderboard_permanent" => "Permanent Leaderboard",
        "leaderboard_challenge_daily" => "Daily Challenge Leaderboard",
        "leaderboard_challenge_permanent" => "Permanent Challenge Leaderboard",
        _ => "Leaderboard",
    }
}

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

/// Returns the reaction emoji(s) for a player's 1-indexed daily leaderboard position.
///
/// - 1st  → 🥇
/// - 2nd  → 🥈
/// - 3rd  → 🥉
/// - 4–9  → number emoji (4️⃣ … 9️⃣)
/// - 10   → 🔟
/// - >10  → 🔟 ➕
pub fn daily_position_reactions(pos: usize) -> Vec<serenity::model::channel::ReactionType> {
    use serenity::model::channel::ReactionType;
    match pos {
        1 => vec![ReactionType::Unicode("🥇".to_string())],
        2 => vec![ReactionType::Unicode("🥈".to_string())],
        3 => vec![ReactionType::Unicode("🥉".to_string())],
        4..=9 => {
            let digit = (b'0' + pos as u8) as char;
            vec![ReactionType::Unicode(format!("{}\u{FE0F}\u{20E3}", digit))]
        }
        10 => vec![ReactionType::Unicode("\u{1F51F}".to_string())],
        _ => vec![
            ReactionType::Unicode("\u{1F51F}".to_string()),
            ReactionType::Unicode("\u{2795}".to_string()),
        ],
    }
}

/// Generate a Discord deep-link to a specific message.
pub fn discord_message_link(guild_id: &str, channel_id: &str, message_id: &str) -> String {
    format!(
        "https://discord.com/channels/{}/{}/{}",
        guild_id, channel_id, message_id
    )
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

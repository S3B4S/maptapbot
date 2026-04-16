// ── Embed constants ─────────────────────────────────────────────────────

use chrono::NaiveDate;
use serenity::all::CreateEmbed;

use crate::{db::LeaderboardRow, formatting::truncate_username};

const COLOR_GOLD: u32 = 0xFFD700;
const COLOR_ELECTRIC_BLUE: u32 = 0x4A90E2;

const MEDALS: [&str; 3] = ["\u{1f947}", "\u{1f948}", "\u{1f949}"]; // 🥇🥈🥉
const SKULL: &str = "\u{1f480}"; // 💀

/// Determine the embed colour for a leaderboard command.
fn embed_color(is_challenge: bool) -> u32 {
    if is_challenge {
        COLOR_ELECTRIC_BLUE
    } else {
        COLOR_GOLD
    }
}

/// Determine the link URL for a leaderboard command.
fn leaderboard_url(is_challenge: bool) -> &'static str {
    if is_challenge {
        "https://maptap.gg/challenge"
    } else {
        "https://maptap.gg"
    }
}

/// Build the embed description (header line).
/// `date` is used for daily leaderboards; `None` falls back to today (UTC).
/// Ignored when `is_permanent` is true.
fn build_description(count: usize, is_permanent: bool, is_challenge: bool, date: Option<NaiveDate>) -> String {
    let url = leaderboard_url(is_challenge);
    if is_permanent {
        format!("All-time \u{00b7} {} players \u{00b7} {}", count, url)
    } else {
        let d = date.unwrap_or_else(|| chrono::Utc::now().date_naive());
        let day = d.format("%A, %B %-d").to_string();
        format!(
            "{} (UTC) \u{00b7} {} players submitted \u{00b7} {}",
            day, count, url
        )
    }
}

/// Format a single leaderboard entry.
/// `emoji` is the prefix (medal or skull).
fn format_entry(
    emoji: &str,
    row: &LeaderboardRow,
    is_challenge: bool,
    is_permanent: bool,
) -> String {
    let name = truncate_username(&row.username, 20);
    let score = if is_permanent {
        format!("{:.1}", row.final_score)
    } else {
        format!("{:.0}", row.final_score)
    };
    if is_challenge {
        match row.time_spent_ms {
            Some(ms) => format!("{} {} ({}, {:.1}s)", emoji, name, score, ms / 1000.0),
            None => format!("{} {} ({})", emoji, name, score),
        }
    } else {
        format!("{} {} ({})", emoji, name, score)
    }
}

/// Build the "Top 3" field value from the first up-to-3 rows.
fn build_top3_value(
    rows: &[LeaderboardRow],
    is_challenge: bool,
    is_permanent: bool,
) -> String {
    rows.iter()
        .enumerate()
        .take(3)
        .map(|(i, row)| format_entry(MEDALS[i], row, is_challenge, is_permanent))
        .collect::<Vec<_>>()
        .join("  ")
}

/// Build the "Bottom 3" field value.
/// Worst-first (rank N, N-1, N-2), only entries that don't overlap with top 3.
fn build_bottom3_value(
    rows: &[LeaderboardRow],
    is_challenge: bool,
    is_permanent: bool,
) -> Option<String> {
    let len = rows.len();
    if len <= 3 {
        return None;
    }
    // Start index: skip the top 3 entries to avoid overlap
    let start = std::cmp::max(3, len.saturating_sub(3));
    let bottom: Vec<String> = rows[start..len]
        .iter()
        .rev() // worst first
        .map(|row| format_entry(SKULL, row, is_challenge, is_permanent))
        .collect();
    Some(bottom.join("  "))
}

/// Build a summary embed for the leaderboard.
/// `date` is the leaderboard date for daily commands; `None` for permanent or "today".
pub fn build_summary_embed(
    title: &str,
    rows: &[LeaderboardRow],
    is_permanent: bool,
    is_challenge: bool,
    date: Option<NaiveDate>,
) -> CreateEmbed {
    let color = embed_color(is_challenge);
    let desc = build_description(rows.len(), is_permanent, is_challenge, date);
    let top3 = build_top3_value(rows, is_challenge, is_permanent);

    let mut embed = CreateEmbed::new()
        .title(title)
        .color(color)
        .description(desc)
        .field("Top 3", &top3, false);

    if let Some(bottom) = build_bottom3_value(rows, is_challenge, is_permanent) {
        embed = embed.field("Bottom 3", &bottom, false);
    }

    embed
}

/// Build the full-list embed posted into a thread.
/// Lists every entry ranked, truncated to fit Discord's 4096-char embed description limit.
/// `date` is the leaderboard date for daily commands; `None` for permanent or "today".
pub fn build_full_embed(
    title: &str,
    rows: &[LeaderboardRow],
    is_permanent: bool,
    is_challenge: bool,
    date: Option<NaiveDate>,
) -> CreateEmbed {
    let color = embed_color(is_challenge);
    let desc = build_description(rows.len(), is_permanent, is_challenge, date);

    let mut lines = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let name = truncate_username(&row.username, 20);
        let score = if is_permanent {
            format!("{:.1}", row.final_score)
        } else {
            format!("{:.0}", row.final_score)
        };
        let line = if is_challenge {
            match row.time_spent_ms {
                Some(ms) => {
                    format!("{}. {} — {} ({:.1}s)", i + 1, name, score, ms / 1000.0)
                }
                None => format!("{}. {} — {}", i + 1, name, score),
            }
        } else {
            format!("{}. {} — {}", i + 1, name, score)
        };
        lines.push(line);
    }

    // Embed description limit is 4096 chars. Truncate if needed.
    let mut body = String::new();
    let suffix = "\n... (truncated)";
    let budget = 4096 - desc.len() - 2 - suffix.len(); // 2 for the \n\n separator
    let mut truncated = false;
    for line in &lines {
        if body.len() + line.len() + 1 > budget {
            body.push_str(suffix);
            truncated = true;
            break;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(line);
    }

    let full_desc = if truncated {
        format!("{}\n\n{}", desc, body)
    } else {
        format!("{}\n\n{}", desc, body)
    };

    CreateEmbed::new()
        .title(title)
        .color(color)
        .description(full_desc)
}

use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};

use serenity::async_trait;

use crate::{plugin::{Plugin, PluginCommand}, repository::Repository};

pub struct SelfPlugin;

fn build_self_response(cmd: &CommandInteraction, repo: &dyn Repository) -> Result<CreateInteractionResponse, String> {
    let user = &cmd.user;
    let user_id_str = user.id.get().to_string();

    let all_scores = repo.get_scores_user(user_id_str)?;

    // Deduplicate: for each (date, mode), keep only the latest valid row by posted_at.
    // list_scores returns rows ordered by date DESC, mode, posted_at DESC — so the
    // first occurrence of each (date, mode) pair is already the latest.
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let effective: Vec<_> = all_scores.iter()
        .filter(|s| !s.invalid)
        .filter(|s| seen.insert((s.date.clone(), s.mode.clone())))
        .collect();

    let daily_count = effective.iter().filter(|s| s.mode == "daily_default").count() as u32;
    let challenge_count = effective.iter().filter(|s| s.mode == "daily_challenge").count() as u32;
    let total_count = daily_count + challenge_count;

    if total_count == 0 {
        return Ok(CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("You haven't submitted any scores yet! Play at https://maptap.gg and share your results here.")
                .ephemeral(true),
        ));
    }

    // Perfect 100s across all effective scores
    let perfect_100s: u32 = effective.iter()
        .flat_map(|s| [s.score1, s.score2, s.score3, s.score4, s.score5])
        .filter(|t| *t == Some(100))
        .count() as u32;

    let total_tiles: u32 = effective.iter()
        .flat_map(|s| [s.score1, s.score2, s.score3, s.score4, s.score5])
        .filter(|t| t.is_some())
        .count() as u32;

    let perfect_pct = if total_tiles > 0 { perfect_100s * 100 / total_tiles } else { 0 };

    // Average final score per mode
    let avg_daily: f64 = {
        let scores: Vec<f64> = effective.iter()
            .filter(|s| s.mode == "daily_default")
            .map(|s| s.final_score as f64)
            .collect();
        if scores.is_empty() { 0.0 } else { scores.iter().sum::<f64>() / scores.len() as f64 }
    };

    let avg_challenge: f64 = {
        let scores: Vec<f64> = effective.iter()
            .filter(|s| s.mode == "daily_challenge")
            .map(|s| s.final_score as f64)
            .collect();
        if scores.is_empty() { 0.0 } else { scores.iter().sum::<f64>() / scores.len() as f64 }
    };

    // Personal best: highest final_score across all effective scores
    let personal_best: Option<(i64, String)> = effective.iter()
        .max_by_key(|s| s.final_score)
        .map(|s| {
            let date = NaiveDate::parse_from_str(&s.date, "%Y-%m-%d")
                .map(|d| d.format("%B %d, %Y").to_string())
                .unwrap_or_else(|_| s.date.clone());
            (s.final_score, date)
        });

    // Playing since: earliest date across all effective scores
    let playing_since: Option<String> = effective.iter()
        .min_by(|a, b| a.date.cmp(&b.date))
        .map(|s| {
            NaiveDate::parse_from_str(&s.date, "%Y-%m-%d")
                .map(|d| d.format("%B %d, %Y").to_string())
                .unwrap_or_else(|_| s.date.clone())
        });

    // Streaks: consecutive daily_default days ending at today (or yesterday)
    let today = Utc::now().date_naive();
    let daily_dates: HashSet<NaiveDate> = effective.iter()
        .filter(|s| s.mode == "daily_default")
        .filter_map(|s| NaiveDate::parse_from_str(&s.date, "%Y-%m-%d").ok())
        .collect();

    let current_streak: u32 = {
        let mut streak = 0u32;
        // Start counting from today; if today has no score, try yesterday
        let mut day = if daily_dates.contains(&today) { today } else { today - Duration::days(1) };
        while daily_dates.contains(&day) {
            streak += 1;
            day = day - Duration::days(1);
        }
        streak
    };

    let best_streak: u32 = if daily_dates.is_empty() {
        0
    } else {
        let mut sorted: Vec<NaiveDate> = daily_dates.iter().cloned().collect();
        sorted.sort();
        let mut best = 1u32;
        let mut current = 1u32;
        for i in 1..sorted.len() {
            if sorted[i] == sorted[i - 1] + Duration::days(1) {
                current += 1;
                if current > best { best = current; }
            } else {
                current = 1;
            }
        }
        best
    };

    // Zero tiles across all effective scores
    let zero_tiles: u32 = effective.iter()
        .flat_map(|s| [s.score1, s.score2, s.score3, s.score4, s.score5])
        .filter(|t| *t == Some(0))
        .count() as u32;

    let zero_pct = if total_tiles > 0 { zero_tiles * 100 / total_tiles } else { 0 };

    // Rotating footer
    let footers = [
        "Keep tapping those maps! 🌍",
        "Every tile is a new opportunity. 🗺️",
        "The world won't map itself. 📍",
        "Geography nerd? Absolutely. 🧭",
        "One day you'll get that 1000. 💪",
    ];
    let footer_idx = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0)
        % footers.len();

    let author = CreateEmbedAuthor::new(&user.name)
        .icon_url(user.avatar_url().unwrap_or_default());

    let mut embed = CreateEmbed::new()
        .author(author)
        .title("🗺️ Your MapTap Stats")
        .color(0x5865F2u32)
        .field(
            "🎯 Scores submitted",
            format!("{} daily · {} challenge · {} total", daily_count, challenge_count, total_count),
            false,
        )
        .field(
            "💯 Perfect 100s",
            format!("{} tiles scored 100 ({}% of all tiles)", perfect_100s, perfect_pct),
            false,
        )
        .field(
            "😬 Zero tiles",
            format!("{} tiles scored 0 ({}% of all tiles)", zero_tiles, zero_pct),
            false,
        )
        .field(
            "🔥 Current streak",
            if current_streak > 0 {
                format!("{} days in a row", current_streak)
            } else {
                "No active streak".to_string()
            },
            true,
        )
        .field("🏆 Best streak", format!("{} days", best_streak), true)
        .field(
            "⭐ Average score",
            format!("{:.1} daily · {:.1} challenge", avg_daily, avg_challenge),
            false,
        )
        .footer(CreateEmbedFooter::new(footers[footer_idx]));

    if let Some((score, date)) = personal_best {
        embed = embed.field("🚀 Personal best", format!("{} on {}", score, date), true);
    }

    if let Some(since) = playing_since {
        embed = embed.field("📅 Playing since", since, true);
    }

    // Daily and weekly rank — only available in guilds
    if let Some(guild_id) = cmd.guild_id {
        let gid = guild_id.get();
        let uid_str = user.id.get().to_string();
        let today_str = today.format("%Y-%m-%d").to_string();

        // Today's daily rank
        if let Ok(daily_lb) = repo.get_daily_leaderboard(gid, &today_str) {
            let rank = daily_lb.iter().position(|r| r.user_id == uid_str).map(|i| i + 1);
            let value = match rank {
                Some(r) => format!("#{} of {}", r, daily_lb.len()),
                None => "Not on today's board".to_string(),
            };
            embed = embed.field("📅 Today's rank", value, true);
        }

        // This week's rank (current ISO week, avg scoring)
        let iso = today.iso_week();
        if let Some(week_start) = NaiveDate::from_isoywd_opt(iso.year(), iso.week(), Weekday::Mon) {
            let week_sunday = week_start + Duration::days(6);
            let week_end = if week_sunday >= today { today } else { week_sunday };
            let ws_str = week_start.format("%Y-%m-%d").to_string();
            let we_str = week_end.format("%Y-%m-%d").to_string();

            if let Ok(weekly_lb) = repo.get_weekly_leaderboard(gid, &ws_str, &we_str, false) {
                let rank = weekly_lb.iter().position(|r| r.user_id == uid_str).map(|i| i + 1);
                let value = match rank {
                    Some(r) => format!("#{} of {}", r, weekly_lb.len()),
                    None => "Not on this week's board".to_string(),
                };
                embed = embed.field("📆 This week's rank", value, true);
            }
        }
    }

    Ok(CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .ephemeral(true),
    ))
}

#[async_trait]
impl Plugin for SelfPlugin {
    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand {
                name: "self",
                description: "View your personal MapTap stats (only visible for you)",
                command: CreateCommand::new("self").description("View your personal MapTap stats"),
            },
        ]
    }

    async fn handle_command(&self, ctx: &Context, cmd: &CommandInteraction, repo: &dyn Repository) {
        let response = match build_self_response(cmd, repo) {
            Ok(r) => r,
            Err(e) => CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(e)
                    .ephemeral(true),
            ),
        };
        if let Err(e) = cmd.create_response(&ctx.http, response).await {
            tracing::error!("Plugin /self failed to respond: {}", e);
        }
    }
}

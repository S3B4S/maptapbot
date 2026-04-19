use serenity::all::{
    CommandInteraction, Context, CreateCommand, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};

use crate::{plugin::{Plugin, PluginCommand}, repository::Repository};

pub struct SelfPlugin;

fn handle_interaction(ctx: &Context, cmd: &CommandInteraction, repo: &dyn Repository) -> Result<CreateInteractionResponse, String> {
    let user = &cmd.user;

    // TODO: replace placeholder stats with real repo calls once Repository has user stat methods
    let daily_count: u32 = 0;
    let challenge_count: u32 = 0;
    let total_count: u32 = daily_count + challenge_count;

    // Empty state — no scores yet
    if total_count == 0 {
        return Ok(CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("You haven't submitted any scores yet! Play at https://maptap.gg and share your results here.")
                .ephemeral(true),
        ));
    }

    // Rotating footer flavour line
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

    // TODO: fetch from repo
    let perfect_100s: u32 = 0;
    let total_tiles: u32 = daily_count * 5 + challenge_count * 5;
    let perfect_pct = if total_tiles > 0 { perfect_100s * 100 / total_tiles } else { 0 };

    let current_streak: u32 = 0;   // TODO: repo.get_current_streak(user_id)
    let best_streak: u32 = 0;      // TODO: repo.get_best_streak(user_id)
    let avg_daily: f64 = 0.0;      // TODO: repo.get_avg_score(user_id, daily)
    let avg_challenge: f64 = 0.0;  // TODO: repo.get_avg_score(user_id, challenge)
    let personal_best: Option<(u32, String)> = None; // TODO: (score, "April 13, 2026")
    let playing_since: Option<String> = None;         // TODO: repo.get_first_score_date(user_id)
    let strongest_tile: Option<(usize, f64)> = None;  // TODO: (tile_pos, avg)
    let weakest_tile: Option<(usize, f64)> = None;    // TODO: (tile_pos, avg)

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

    // Server rank — only shown in guilds
    if cmd.guild_id.is_some() {
        // TODO: repo.get_server_rank(user_id, guild_id)
        embed = embed.field("🏅 Server rank", "Not ranked yet".to_string(), false);
    }

    if let Some((pos, avg)) = strongest_tile {
        embed = embed.field("🧩 Strongest tile", format!("Tile {} — avg {:.1}", pos, avg), true);
    }

    if let Some((pos, avg)) = weakest_tile {
        embed = embed.field("😬 Weakest tile", format!("Tile {} — avg {:.1}", pos, avg), true);
    }

    Ok(CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .ephemeral(true),
    ))
}

impl Plugin for SelfPlugin {
    fn commands(&self) -> Vec<CreateCommand> {
        vec![
            CreateCommand::new("self").description("View your personal MapTap stats"),
        ]
    }

    fn register_commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand {
                command_name: "self".to_string(),
                command_description: "View your personal MapTap stats".to_string(),
                handle_interaction,
            }
        ]
    }
}

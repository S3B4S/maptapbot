use serenity::async_trait;
use serenity::builder::{
    CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::model::application::Interaction;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::db::{Database, LeaderboardRow};
use crate::parser::parse_maptap_message;

pub struct Handler {
    db: std::sync::Mutex<Database>,
}

impl Handler {
    pub fn new(db: Database) -> Self {
        Self {
            db: std::sync::Mutex::new(db),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Sanitize control characters (ANSI escape sequences, etc.) to prevent log injection
        let sanitized: String = msg
            .content
            .chars()
            .map(|c| if c.is_control() && c != '\n' { '?' } else { c })
            .collect();

        println!("{}", sanitized);

        // Ignore messages from bots (including ourselves)
        if msg.author.bot {
            return;
        }

        let user_id = msg.author.id.get();
        let guild_id = msg.guild_id.map(|g| g.get());
        let content = &msg.content;

        let result = match parse_maptap_message(user_id, guild_id, content) {
            Some(r) => r,
            None => return, // Not a maptap message, ignore silently
        };

        match result {
            Ok(score) => {
                let date_str = score.date.format("%Y-%m-%d").to_string();
                let final_score = score.final_score;

                // Scope the lock so it's dropped before any .await
                let db_result = self
                    .db
                    .lock()
                    .map_err(|e| format!("Failed to lock DB: {}", e))
                    .and_then(|db| {
                        db.upsert_user(score.user_id, &msg.author.name)
                            .map_err(|e| format!("DB error (user): {}", e))?;
                        db.upsert_score(&score)
                            .map_err(|e| format!("DB error (score): {}", e))
                    });

                if let Err(e) = db_result {
                    error!("{}", e);
                    let _ = msg
                        .reply(&ctx.http, "Internal error saving your score.")
                        .await;
                    return;
                }

                info!(
                    "Saved score for user {} on {}: {}",
                    msg.author.name, date_str, final_score
                );

                let reply = format!(
                    "Recorded! {} scored **{}** on {}",
                    msg.author.name, final_score, date_str
                );
                let _ = msg.reply(&ctx.http, reply).await;
            }
            Err(e) => {
                warn!("Invalid maptap message from {}: {}", msg.author.name, e);
                let reply = format!("Invalid maptap score: {}", e);
                let _ = msg.reply(&ctx.http, reply).await;
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands = vec![
            CreateCommand::new("today").description("Get a link to today's maptap challenge"),
            CreateCommand::new("leaderboard_daily")
                .description("Show today's leaderboard for this server"),
            CreateCommand::new("leaderboard_permanent")
                .description("Show the all-time average leaderboard for this server"),
        ];

        if let Err(e) =
            serenity::model::application::Command::set_global_commands(&ctx.http, commands).await
        {
            error!("Failed to register slash commands: {}", e);
        } else {
            info!("Slash commands registered");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction {
            let guild_id = cmd.guild_id.map(|g| g.get());

            match cmd.data.name.as_str() {
                "today" => {
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Today's challenge: https://maptap.gg/")
                            .ephemeral(true),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /today: {}", e);
                    }
                }
                "leaderboard_daily" => {
                    let Some(gid) = guild_id else {
                        let _ = cmd
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("This command can only be used in a server.")
                                        .ephemeral(true),
                                ),
                            )
                            .await;
                        return;
                    };

                    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                    let rows = {
                        let db = self.db.lock().unwrap();
                        db.get_daily_leaderboard(gid, &today)
                    };

                    let content = match rows {
                        Ok(rows) if rows.is_empty() => {
                            "No scores recorded for today yet!".to_string()
                        }
                        Ok(rows) => format_leaderboard_table("Daily Leaderboard", &rows, false),
                        Err(e) => {
                            error!("DB error: {}", e);
                            "Internal error fetching leaderboard.".to_string()
                        }
                    };

                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(content),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /leaderboard_daily: {}", e);
                    }
                }
                "leaderboard_permanent" => {
                    let Some(gid) = guild_id else {
                        let _ = cmd
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("This command can only be used in a server.")
                                        .ephemeral(true),
                                ),
                            )
                            .await;
                        return;
                    };

                    let rows = {
                        let db = self.db.lock().unwrap();
                        db.get_permanent_leaderboard(gid)
                    };

                    let content = match rows {
                        Ok(rows) if rows.is_empty() => "No scores recorded yet!".to_string(),
                        Ok(rows) => {
                            format_leaderboard_table("Permanent Leaderboard (Averages)", &rows, true)
                        }
                        Err(e) => {
                            error!("DB error: {}", e);
                            "Internal error fetching leaderboard.".to_string()
                        }
                    };

                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(content),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /leaderboard_permanent: {}", e);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Format a leaderboard table as a Discord code block.
/// If `averages` is true, values are shown with 1 decimal place.
fn format_leaderboard_table(title: &str, rows: &[LeaderboardRow], averages: bool) -> String {
    let mut out = format!("**{}**\n```\n", title);
    let header = format!(
        "{:<4} {:<20} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7}",
        "#", "User", "S1", "S2", "S3", "S4", "S5", "Total"
    );
    let width = header.len();
    out.push_str(&header);
    out.push('\n');
    out.push_str(&"-".repeat(width));
    out.push('\n');

    for (i, row) in rows.iter().enumerate() {
        let name = truncate_username(&row.username, 20);
        if averages {
            out.push_str(&format!(
                "{:<4} {:<20} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>7.1}\n",
                i + 1,
                name,
                row.score1,
                row.score2,
                row.score3,
                row.score4,
                row.score5,
                row.final_score,
            ));
        } else {
            out.push_str(&format!(
                "{:<4} {:<20} {:>5.0} {:>5.0} {:>5.0} {:>5.0} {:>5.0} {:>7.0}\n",
                i + 1,
                name,
                row.score1,
                row.score2,
                row.score3,
                row.score4,
                row.score5,
                row.final_score,
            ));
        }
    }

    out.push_str("```");
    out
}

/// Truncate a username to `max_len` characters, appending ".." if truncated.
fn truncate_username(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        let mut truncated = name[..max_len - 2].to_string();
        truncated.push_str("..");
        truncated
    }
}

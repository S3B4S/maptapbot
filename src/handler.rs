use std::collections::HashMap;

use serenity::async_trait;
use serenity::builder::{
    CreateCommand, CreateCommandOption, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use serenity::model::application::{CommandOptionType, Interaction};
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, MessageId};
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::db::{Database, LeaderboardRow, ScoreRow};
use crate::parser::{parse_challenge_message, parse_maptap_message};

pub struct Handler {
    db: std::sync::Mutex<Database>,
    /// Tracks the last posted leaderboard message per (guild_id, command_name).
    /// Used to delete the previous message before posting a new one.
    leaderboard_msgs: std::sync::Mutex<HashMap<(u64, &'static str), (ChannelId, MessageId)>>,
    /// Optional allowlist of channel IDs. When `Some`, only messages from these
    /// channels are parsed. When `None`, all channels are processed.
    channel_ids: Option<Vec<u64>>,
    /// Discord user IDs that have admin privileges.
    admin_ids: Vec<u64>,
}

impl Handler {
    pub fn new(db: Database, channel_ids: Option<Vec<u64>>, admin_ids: Vec<u64>) -> Self {
        Self {
            db: std::sync::Mutex::new(db),
            leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
            channel_ids,
            admin_ids,
        }
    }

    /// Check whether a Discord user ID is in the admin list.
    fn is_admin(&self, user_id: u64) -> bool {
        self.admin_ids.contains(&user_id)
    }

    /// Dispatch an admin command and return the response text.
    fn handle_admin_command(
        &self,
        name: &str,
        options: &[serenity::model::application::ResolvedOption<'_>],
    ) -> String {
        let get_str = |key: &str| -> Option<&str> {
            options.iter().find_map(|o| {
                if o.name == key {
                    if let serenity::model::application::ResolvedValue::String(s) = o.value {
                        return Some(s);
                    }
                }
                None
            })
        };

        let db = match self.db.lock() {
            Ok(db) => db,
            Err(e) => return format!("Internal error: failed to lock DB: {}", e),
        };

        match name {
            "delete_score" => {
                let Some(user_id) = get_str("user_id") else {
                    return "Missing required parameter: user_id".to_string();
                };
                let Some(date) = get_str("date") else {
                    return "Missing required parameter: date".to_string();
                };
                let Some(mode) = get_str("mode") else {
                    return "Missing required parameter: mode".to_string();
                };
                match db.delete_score(user_id, date, mode) {
                    Ok(0) => format!("No score found for user `{}` on `{}` (mode: `{}`).", user_id, date, mode),
                    Ok(n) => format!("Deleted {} score(s) for user `{}` on `{}` (mode: `{}`).", n, user_id, date, mode),
                    Err(e) => format!("DB error: {}", e),
                }
            }
            "list_scores" => {
                let Some(user_id) = get_str("user_id") else {
                    return "Missing required parameter: user_id".to_string();
                };
                match db.list_scores(user_id) {
                    Ok(rows) if rows.is_empty() => format!("No scores found for user `{}`.", user_id),
                    Ok(rows) => format_score_rows(&rows),
                    Err(e) => format!("DB error: {}", e),
                }
            }
            "list_all_scores" => match db.list_all_scores() {
                Ok(rows) if rows.is_empty() => "No scores in the database.".to_string(),
                Ok(rows) => format_score_rows(&rows),
                Err(e) => format!("DB error: {}", e),
            },
            "list_users" => match db.list_users() {
                Ok(rows) if rows.is_empty() => "No users in the database.".to_string(),
                Ok(rows) => {
                    let mut out = format!("**Users ({} total)**\n```\n", rows.len());
                    out.push_str(&format!("{:<22} {}\n", "User ID", "Username"));
                    out.push_str(&"-".repeat(42));
                    out.push('\n');
                    for row in &rows {
                        out.push_str(&format!("{:<22} {}\n", row.user_id, row.username));
                    }
                    out.push_str("```");
                    truncate_message(out)
                }
                Err(e) => format!("DB error: {}", e),
            },
            "raw_score" => {
                let Some(user_id) = get_str("user_id") else {
                    return "Missing required parameter: user_id".to_string();
                };
                let Some(date) = get_str("date") else {
                    return "Missing required parameter: date".to_string();
                };
                let Some(mode) = get_str("mode") else {
                    return "Missing required parameter: mode".to_string();
                };
                match db.raw_score(user_id, date, mode) {
                    Ok(Some(raw)) => format!("Raw message for `{}` on `{}` (`{}`):\n```\n{}\n```", user_id, date, mode, raw),
                    Ok(None) => format!("No score found for user `{}` on `{}` (mode: `{}`).", user_id, date, mode),
                    Err(e) => format!("DB error: {}", e),
                }
            }
            "clear_day" => {
                let Some(date) = get_str("date") else {
                    return "Missing required parameter: date".to_string();
                };
                match db.clear_day(date) {
                    Ok(0) => format!("No scores found for date `{}`.", date),
                    Ok(n) => format!("Deleted {} score(s) for date `{}`.", n, date),
                    Err(e) => format!("DB error: {}", e),
                }
            }
            "stats" => match db.stats() {
                Ok(stats) => {
                    let date_range = match (&stats.min_date, &stats.max_date) {
                        (Some(min), Some(max)) => format!("{} to {}", min, max),
                        _ => "N/A".to_string(),
                    };
                    format!(
                        "**DB Stats**\n```\n\
                         Total entries:    {}\n\
                         Unique users:     {}\n\
                         Date range:       {}\n\
                         daily_default:    {}\n\
                         daily_challenge:  {}\n\
                         ```",
                        stats.total_entries,
                        stats.unique_users,
                        date_range,
                        stats.daily_default_count,
                        stats.daily_challenge_count,
                    )
                }
                Err(e) => format!("DB error: {}", e),
            },
            _ => "Unknown admin command.".to_string(),
        }
    }

    /// Look up and remove the previous leaderboard message for (guild_id, cmd).
    fn take_prev_leaderboard_msg(
        &self,
        guild_id: u64,
        cmd: &'static str,
    ) -> Option<(ChannelId, MessageId)> {
        self.leaderboard_msgs
            .lock()
            .ok()?
            .remove(&(guild_id, cmd))
    }

    /// Store the new leaderboard message for (guild_id, cmd).
    fn store_leaderboard_msg(
        &self,
        guild_id: u64,
        cmd: &'static str,
        channel_id: ChannelId,
        message_id: MessageId,
    ) {
        if let Ok(mut map) = self.leaderboard_msgs.lock() {
            map.insert((guild_id, cmd), (channel_id, message_id));
        }
    }

    fn build_leaderboard_content(&self, name: &str, gid: u64) -> String {
        let db = self.db.lock().unwrap();
        match name {
            "leaderboard_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                db.get_daily_leaderboard(gid, &today)
                    .map(|rows| {
                        if rows.is_empty() {
                            "No scores recorded for today yet!".to_string()
                        } else {
                            format_leaderboard_table("Daily Leaderboard", &rows, false)
                        }
                    })
                    .unwrap_or_else(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })
            }
            "leaderboard_permanent" => db
                .get_permanent_leaderboard(gid)
                .map(|rows| {
                    if rows.is_empty() {
                        "No scores recorded yet!".to_string()
                    } else {
                        format_leaderboard_table("Permanent Leaderboard (Averages)", &rows, true)
                    }
                })
                .unwrap_or_else(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                }),
            "leaderboard_challenge_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                db.get_daily_challenge_leaderboard(gid, &today)
                    .map(|rows| {
                        if rows.is_empty() {
                            "No challenge scores recorded for today yet!".to_string()
                        } else {
                            format_challenge_leaderboard_table(
                                "Daily Challenge Leaderboard",
                                &rows,
                                false,
                            )
                        }
                    })
                    .unwrap_or_else(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })
            }
            "leaderboard_challenge_permanent" => db
                .get_permanent_challenge_leaderboard(gid)
                .map(|rows| {
                    if rows.is_empty() {
                        "No challenge scores recorded yet!".to_string()
                    } else {
                        format_challenge_leaderboard_table(
                            "Permanent Challenge Leaderboard (Averages)",
                            &rows,
                            true,
                        )
                    }
                })
                .unwrap_or_else(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                }),
            _ => "Unknown leaderboard command.".to_string(),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots (including ourselves)
        if msg.author.bot {
            return;
        }

        // If a channel allowlist is configured, ignore messages from other channels.
        if let Some(ref ids) = self.channel_ids {
            if !ids.contains(&msg.channel_id.get()) {
                return;
            }
        }

        let user_id = msg.author.id.get();
        let guild_id = msg.guild_id.map(|g| g.get());
        let content = &msg.content;

        // Try default parser first, then challenge parser.
        let result = parse_maptap_message(user_id, guild_id, content)
            .or_else(|| parse_challenge_message(user_id, guild_id, content));

        let result = match result {
            Some(r) => r,
            None => return, // Not a maptap message, ignore silently
        };

        match result {
            Ok(score) => {
                let date_str = score.date.format("%Y-%m-%d").to_string();
                let final_score = score.final_score;
                let mode_label = match score.mode {
                    crate::models::GameMode::DailyDefault => "default",
                    crate::models::GameMode::DailyChallenge => "challenge",
                };

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
                    "Saved {} score for user {} on {}: {}",
                    mode_label, msg.author.name, date_str, final_score
                );

                // React with 🗺️ instead of sending a reply message.
                let _ = msg.react(&ctx.http, '🗺').await;
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

        let mode_option = || {
            CreateCommandOption::new(
                CommandOptionType::String,
                "mode",
                "Game mode (daily_default or daily_challenge)",
            )
            .required(true)
            .add_string_choice("daily_default", "daily_default")
            .add_string_choice("daily_challenge", "daily_challenge")
        };

        let user_id_option = |required: bool| {
            CreateCommandOption::new(CommandOptionType::String, "user_id", "Discord user ID")
                .required(required)
        };

        let date_option = |required: bool| {
            CreateCommandOption::new(
                CommandOptionType::String,
                "date",
                "Date in YYYY-MM-DD format",
            )
            .required(required)
        };

        let commands = vec![
            // User-facing commands
            CreateCommand::new("today").description("Get a link to today's maptap challenge"),
            CreateCommand::new("leaderboard_daily")
                .description("Show today's leaderboard for this server"),
            CreateCommand::new("leaderboard_permanent")
                .description("Show the all-time average leaderboard for this server"),
            CreateCommand::new("leaderboard_challenge_daily")
                .description("Show today's challenge leaderboard for this server"),
            CreateCommand::new("leaderboard_challenge_permanent")
                .description("Show the all-time challenge leaderboard for this server"),
            CreateCommand::new("help").description("Show available commands"),
            // Admin commands
            CreateCommand::new("delete_score")
                .description("Delete a specific score entry")
                .add_option(user_id_option(true))
                .add_option(date_option(true))
                .add_option(mode_option()),
            CreateCommand::new("list_scores")
                .description("Show all scores for a given user")
                .add_option(user_id_option(true)),
            CreateCommand::new("list_all_scores").description("Dump all scores in the database"),
            CreateCommand::new("list_users").description("List all known users"),
            CreateCommand::new("raw_score")
                .description("Show the raw stored message for a score entry")
                .add_option(user_id_option(true))
                .add_option(date_option(true))
                .add_option(mode_option()),
            CreateCommand::new("clear_day")
                .description("Wipe all scores for a given date")
                .add_option(date_option(true)),
            CreateCommand::new("stats").description("Show aggregate DB stats"),
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
            let invoker_id = cmd.user.id.get();

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
                name @ ("leaderboard_daily"
                | "leaderboard_permanent"
                | "leaderboard_challenge_daily"
                | "leaderboard_challenge_permanent") => {
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

                    let content = self.build_leaderboard_content(name, gid);
                    let cmd_key = cmd_name_key(name);

                    // Delete the previous leaderboard message for this command, if any.
                    if let Some((ch_id, msg_id)) = self.take_prev_leaderboard_msg(gid, cmd_key) {
                        let _ = ctx.http.delete_message(ch_id, msg_id, None).await;
                    }

                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content(content),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /{}: {}", name, e);
                        return;
                    }

                    // Retrieve the posted message so we can store its ID for later deletion.
                    match cmd.get_response(&ctx.http).await {
                        Ok(posted) => {
                            self.store_leaderboard_msg(gid, cmd_key, posted.channel_id, posted.id);
                        }
                        Err(e) => {
                            error!("Failed to retrieve response message for /{}: {}", name, e);
                        }
                    }
                }
                "help" => {
                    let content = build_help_text(self.is_admin(invoker_id));
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(content)
                            .ephemeral(true),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /help: {}", e);
                    }
                }
                // ── Admin commands ───────────────────────────────────────
                name @ ("delete_score" | "list_scores" | "list_all_scores" | "list_users"
                | "raw_score" | "clear_day" | "stats") => {
                    if !self.is_admin(invoker_id) {
                        let response = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("You do not have permission to use this command.")
                                .ephemeral(true),
                        );
                        let _ = cmd.create_response(&ctx.http, response).await;
                        return;
                    }

                    let content = self.handle_admin_command(name, &cmd.data.options());
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(content)
                            .ephemeral(true),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /{}: {}", name, e);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Map a runtime command name string to a `'static str` key for the HashMap.
fn cmd_name_key(name: &str) -> &'static str {
    match name {
        "leaderboard_daily" => "leaderboard_daily",
        "leaderboard_permanent" => "leaderboard_permanent",
        "leaderboard_challenge_daily" => "leaderboard_challenge_daily",
        "leaderboard_challenge_permanent" => "leaderboard_challenge_permanent",
        _ => unreachable!("cmd_name_key called with unexpected name: {}", name),
    }
}

/// Format a default-mode leaderboard table as a Discord code block.
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
        let scores = [row.score1, row.score2, row.score3, row.score4, row.score5];
        if averages {
            let fmt: Vec<String> = scores
                .iter()
                .map(|s| format!("{:>5.1}", s.unwrap_or(0.0)))
                .collect();
            out.push_str(&format!(
                "{:<4} {:<20} {} {} {} {} {} {:>7.1}\n",
                i + 1,
                name,
                fmt[0], fmt[1], fmt[2], fmt[3], fmt[4],
                row.final_score,
            ));
        } else {
            let fmt: Vec<String> = scores
                .iter()
                .map(|s| format!("{:>5.0}", s.unwrap_or(0.0)))
                .collect();
            out.push_str(&format!(
                "{:<4} {:<20} {} {} {} {} {} {:>7.0}\n",
                i + 1,
                name,
                fmt[0], fmt[1], fmt[2], fmt[3], fmt[4],
                row.final_score,
            ));
        }
    }

    out.push_str("```");
    out
}

/// Format a challenge leaderboard table as a Discord code block.
/// Adds a Time column after Total.
/// In the daily (non-averages) view, NULL scores (timed-out tiles) render as "--".
fn format_challenge_leaderboard_table(
    title: &str,
    rows: &[LeaderboardRow],
    averages: bool,
) -> String {
    let mut out = format!("**{}**\n```\n", title);
    let header = format!(
        "{:<4} {:<20} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7} {:>7}",
        "#", "User", "S1", "S2", "S3", "S4", "S5", "Total", "Time"
    );
    let width = header.len();
    out.push_str(&header);
    out.push('\n');
    out.push_str(&"-".repeat(width));
    out.push('\n');

    for (i, row) in rows.iter().enumerate() {
        let name = truncate_username(&row.username, 20);
        let time_str = match row.time_spent_ms {
            Some(ms) => format!("{:.1}s", ms / 1000.0),
            None => "-".to_string(),
        };
        let scores = [row.score1, row.score2, row.score3, row.score4, row.score5];
        if averages {
            let fmt: Vec<String> = scores
                .iter()
                .map(|s| format!("{:>5.1}", s.unwrap_or(0.0)))
                .collect();
            out.push_str(&format!(
                "{:<4} {:<20} {} {} {} {} {} {:>7.1} {:>7}\n",
                i + 1,
                name,
                fmt[0], fmt[1], fmt[2], fmt[3], fmt[4],
                row.final_score,
                time_str,
            ));
        } else {
            let fmt: Vec<String> = scores
                .iter()
                .map(|s| match s {
                    Some(v) => format!("{:>5.0}", v),
                    None => format!("{:>5}", "--"),
                })
                .collect();
            out.push_str(&format!(
                "{:<4} {:<20} {} {} {} {} {} {:>7.0} {:>7}\n",
                i + 1,
                name,
                fmt[0], fmt[1], fmt[2], fmt[3], fmt[4],
                row.final_score,
                time_str,
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

/// Build the /help response text. Admin commands are included only when `is_admin` is true.
fn build_help_text(is_admin: bool) -> String {
    let mut text = String::from("**Available Commands**\n\n");
    text.push_str("`/today` — Get a link to today's maptap challenge\n");
    text.push_str("`/leaderboard_daily` — Show today's scores for this server\n");
    text.push_str("`/leaderboard_permanent` — Show the all-time average scores for this server\n");
    text.push_str(
        "`/leaderboard_challenge_daily` — Show today's challenge scores for this server\n",
    );
    text.push_str("`/leaderboard_challenge_permanent` — Show the all-time challenge averages for this server\n");
    text.push_str("`/help` — Show this help message\n");

    if is_admin {
        text.push_str("\n**Admin Commands**\n\n");
        text.push_str("`/delete_score <user_id> <date> <mode>` — Delete a specific score entry\n");
        text.push_str(
            "`/list_scores <user_id>` — Show all scores for a given user across all dates and modes\n",
        );
        text.push_str("`/list_all_scores` — Dump the full contents of the scores table\n");
        text.push_str("`/list_users` — List all users known to the bot\n");
        text.push_str(
            "`/raw_score <user_id> <date> <mode>` — Show the raw stored message for a score entry\n",
        );
        text.push_str("`/clear_day <date>` — Wipe all scores for a given date\n");
        text.push_str("`/stats` — Show aggregate DB stats\n");
    }

    text
}

/// Format score rows into a code-block table, truncated to Discord's message limit.
fn format_score_rows(rows: &[ScoreRow]) -> String {
    let mut out = format!("**Scores ({} total)**\n```\n", rows.len());
    out.push_str(&format!(
        "{:<22} {:<16} {:<12} {:<18} {:>6}\n",
        "User ID", "Username", "Date", "Mode", "Score"
    ));
    out.push_str(&"-".repeat(76));
    out.push('\n');
    for row in rows {
        let username = truncate_username(&row.username, 16);
        out.push_str(&format!(
            "{:<22} {:<16} {:<12} {:<18} {:>6}\n",
            row.user_id, username, row.date, row.mode, row.final_score,
        ));
    }
    out.push_str("```");
    truncate_message(out)
}

/// Discord messages have a 2000 character limit.
/// If the message exceeds it, truncate and add a note.
fn truncate_message(msg: String) -> String {
    const MAX: usize = 2000;
    if msg.len() <= MAX {
        return msg;
    }
    // Find how many rows we can keep. Cut before the closing ``` and add a note.
    let suffix = "\n... (truncated)\n```";
    let budget = MAX - suffix.len();
    // Find the last newline within budget.
    let cut = msg[..budget].rfind('\n').unwrap_or(budget);
    let mut truncated = msg[..cut].to_string();
    truncated.push_str(suffix);
    truncated
}

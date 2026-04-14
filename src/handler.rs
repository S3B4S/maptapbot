use std::collections::HashMap;

use serenity::async_trait;
use serenity::builder::{
    CreateActionRow, CreateButton, CreateCommand, CreateCommandOption, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage, CreateMessage, CreateThread,
};
use serenity::model::application::{ButtonStyle, CommandOptionType, Interaction};
use serenity::model::channel::{ChannelType, Message};
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, MessageId};
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::db::{Database, LeaderboardRow, ScoreRow};
use crate::parser::{parse_challenge_message, parse_maptap_message};

pub struct Handler {
    db: std::sync::Mutex<Database>,
    /// Tracks the last posted leaderboard message per (guild_id, command_name).
    /// Stores (channel_id, message_id, invoker_user_id) so the button handler
    /// can verify who invoked the command and find the message to delete.
    leaderboard_msgs:
        std::sync::Mutex<HashMap<(u64, &'static str), (ChannelId, MessageId, u64)>>,
    /// Tracks the last posted full-leaderboard message per (guild_id, command_name).
    /// Used by the "Remove full leaderboard" button.
    full_leaderboard_msgs:
        std::sync::Mutex<HashMap<(u64, &'static str), (ChannelId, MessageId)>>,
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
            full_leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
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
    ) -> Option<(ChannelId, MessageId, u64)> {
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
        invoker_id: u64,
    ) {
        if let Ok(mut map) = self.leaderboard_msgs.lock() {
            map.insert((guild_id, cmd), (channel_id, message_id, invoker_id));
        }
    }

    /// Store the full-leaderboard message for (guild_id, cmd).
    fn store_full_leaderboard_msg(
        &self,
        guild_id: u64,
        cmd: &'static str,
        channel_id: ChannelId,
        message_id: MessageId,
    ) {
        if let Ok(mut map) = self.full_leaderboard_msgs.lock() {
            map.insert((guild_id, cmd), (channel_id, message_id));
        }
    }

    /// Look up and remove the full-leaderboard message for (guild_id, cmd).
    fn take_full_leaderboard_msg(
        &self,
        guild_id: u64,
        cmd: &'static str,
    ) -> Option<(ChannelId, MessageId)> {
        self.full_leaderboard_msgs
            .lock()
            .ok()?
            .remove(&(guild_id, cmd))
    }

    /// Build the leaderboard summary embed for the given command.
    /// Returns `Ok(embed)` on success, or `Err(empty_state_message)` when there are no entries.
    fn build_leaderboard_embed(&self, name: &str, gid: u64) -> Result<CreateEmbed, String> {
        let db = self.db.lock().unwrap();
        match name {
            "leaderboard_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                db.get_daily_leaderboard(gid, &today)
                    .map(|rows| {
                        if rows.is_empty() {
                            Err("No scores recorded for today yet!".to_string())
                        } else {
                            Ok(build_summary_embed(
                                "Daily Leaderboard",
                                &rows,
                                false,
                                false,
                            ))
                        }
                    })
                    .unwrap_or_else(|e| {
                        error!("DB error: {}", e);
                        Err("Internal error fetching leaderboard.".to_string())
                    })
            }
            "leaderboard_permanent" => db
                .get_permanent_leaderboard(gid)
                .map(|rows| {
                    if rows.is_empty() {
                        Err("No scores recorded yet!".to_string())
                    } else {
                        Ok(build_summary_embed(
                            "Permanent Leaderboard",
                            &rows,
                            true,
                            false,
                        ))
                    }
                })
                .unwrap_or_else(|e| {
                    error!("DB error: {}", e);
                    Err("Internal error fetching leaderboard.".to_string())
                }),
            "leaderboard_challenge_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                db.get_daily_challenge_leaderboard(gid, &today)
                    .map(|rows| {
                        if rows.is_empty() {
                            Err("No challenge scores recorded for today yet!".to_string())
                        } else {
                            Ok(build_summary_embed(
                                "Daily Challenge Leaderboard",
                                &rows,
                                false,
                                true,
                            ))
                        }
                    })
                    .unwrap_or_else(|e| {
                        error!("DB error: {}", e);
                        Err("Internal error fetching leaderboard.".to_string())
                    })
            }
            "leaderboard_challenge_permanent" => db
                .get_permanent_challenge_leaderboard(gid)
                .map(|rows| {
                    if rows.is_empty() {
                        Err("No challenge scores recorded yet!".to_string())
                    } else {
                        Ok(build_summary_embed(
                            "Permanent Challenge Leaderboard",
                            &rows,
                            true,
                            true,
                        ))
                    }
                })
                .unwrap_or_else(|e| {
                    error!("DB error: {}", e);
                    Err("Internal error fetching leaderboard.".to_string())
                }),
            _ => Err("Unknown leaderboard command.".to_string()),
        }
    }

    /// Build the full leaderboard embed (all entries) for the thread view.
    fn build_full_leaderboard_embed(&self, name: &str, gid: u64) -> Result<CreateEmbed, String> {
        let db = self.db.lock().unwrap();
        let (title, rows, is_permanent, is_challenge) = match name {
            "leaderboard_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let rows = db.get_daily_leaderboard(gid, &today).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Daily Leaderboard — Full", rows, false, false)
            }
            "leaderboard_permanent" => {
                let rows = db.get_permanent_leaderboard(gid).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Permanent Leaderboard — Full", rows, true, false)
            }
            "leaderboard_challenge_daily" => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let rows = db
                    .get_daily_challenge_leaderboard(gid, &today)
                    .map_err(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })?;
                ("Daily Challenge Leaderboard — Full", rows, false, true)
            }
            "leaderboard_challenge_permanent" => {
                let rows = db
                    .get_permanent_challenge_leaderboard(gid)
                    .map_err(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })?;
                (
                    "Permanent Challenge Leaderboard — Full",
                    rows,
                    true,
                    true,
                )
            }
            _ => return Err("Unknown leaderboard command.".to_string()),
        };

        if rows.is_empty() {
            return Err("No scores to display.".to_string());
        }

        Ok(build_full_embed(title, &rows, is_permanent, is_challenge))
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
        // Thread messages have their own channel ID, so we also check whether the
        // thread's parent channel is in the allowlist.
        if let Some(ref ids) = self.channel_ids {
            if !ids.contains(&msg.channel_id.get()) {
                let parent_allowed = 'parent: {
                    // Try cache first (no API call).
                    // Threads live in guild.threads (Vec), not guild.channels (HashMap).
                    if let Some(guild_id) = msg.guild_id {
                        if let Some(guild) = ctx.cache.guild(guild_id) {
                            let cached = guild
                                .channels
                                .get(&msg.channel_id)
                                .or_else(|| {
                                    guild
                                        .threads
                                        .iter()
                                        .find(|t| t.id == msg.channel_id)
                                });
                            if let Some(channel) = cached {
                                break 'parent channel
                                    .parent_id
                                    .map_or(false, |pid| ids.contains(&pid.get()));
                            }
                        }
                    }
                    // Fallback: fetch from Discord API.
                    match msg.channel_id.to_channel(&ctx.http).await {
                        Ok(channel) => {
                            if let Some(guild_channel) = channel.guild() {
                                guild_channel
                                    .parent_id
                                    .map_or(false, |pid| ids.contains(&pid.get()))
                            } else {
                                false
                            }
                        }
                        Err(e) => {
                            warn!("Failed to resolve channel {}: {}", msg.channel_id, e);
                            false
                        }
                    }
                };
                if !parent_allowed {
                    return;
                }
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

                    let embed = match self.build_leaderboard_embed(name, gid) {
                        Ok(e) => e,
                        Err(msg) => {
                            // Empty state — respond ephemeral, no buttons.
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(msg)
                                    .ephemeral(true),
                            );
                            let _ = cmd.create_response(&ctx.http, response).await;
                            return;
                        }
                    };

                    let cmd_key = cmd_name_key(name);

                    // Delete the previous leaderboard message for this command, if any.
                    if let Some((ch_id, msg_id, _)) =
                        self.take_prev_leaderboard_msg(gid, cmd_key)
                    {
                        let _ = ctx.http.delete_message(ch_id, msg_id, None).await;
                    }

                    // Post the public embed.
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().embed(embed),
                    );
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Failed to respond to /{}: {}", name, e);
                        return;
                    }

                    // Retrieve the posted message so we can store its ID for later deletion.
                    match cmd.get_response(&ctx.http).await {
                        Ok(posted) => {
                            self.store_leaderboard_msg(
                                gid,
                                cmd_key,
                                posted.channel_id,
                                posted.id,
                                invoker_id,
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to retrieve response message for /{}: {}",
                                name, e
                            );
                        }
                    }

                    // Send ephemeral follow-up with buttons (only the invoker sees this).
                    let buttons = CreateActionRow::Buttons(vec![
                        CreateButton::new(format!("full_lb:{}:{}", name, gid))
                            .label("Full leaderboard")
                            .style(ButtonStyle::Primary),
                        CreateButton::new(format!("remove_lb:{}:{}", name, gid))
                            .label("Remove")
                            .style(ButtonStyle::Danger),
                    ]);
                    let followup = CreateInteractionResponseFollowup::new()
                        .content("Leaderboard actions:")
                        .components(vec![buttons])
                        .ephemeral(true);
                    if let Err(e) = cmd.create_followup(&ctx.http, followup).await {
                        error!("Failed to send button follow-up for /{}: {}", name, e);
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
        } else if let Interaction::Component(component) = interaction {
            let custom_id = component.data.custom_id.clone();

            if let Some(rest) = custom_id.strip_prefix("full_lb:") {
                // "Full leaderboard" button — create a thread and post the full list.
                let Some((cmd_name, gid_str)) = rest.split_once(':') else {
                    warn!("Malformed full_lb custom_id: {}", custom_id);
                    return;
                };
                let Ok(gid) = gid_str.parse::<u64>() else {
                    warn!("Invalid guild_id in full_lb custom_id: {}", custom_id);
                    return;
                };

                // Build the full embed (re-queries the DB for fresh data).
                let embed = match self.build_full_leaderboard_embed(cmd_name, gid) {
                    Ok(e) => e,
                    Err(msg) => {
                        let ack = CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new().content(msg),
                        );
                        let _ = component.create_response(&ctx.http, ack).await;
                        return;
                    }
                };

                // Look up the public summary message to create a thread on.
                let cmd_key = cmd_name_key(cmd_name);
                let stored = self
                    .leaderboard_msgs
                    .lock()
                    .ok()
                    .and_then(|map| map.get(&(gid, cmd_key)).copied());

                let Some((ch_id, msg_id, _)) = stored else {
                    let ack = CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("The leaderboard message is no longer available."),
                    );
                    let _ = component.create_response(&ctx.http, ack).await;
                    return;
                };

                let in_thread = component
                    .channel
                    .as_ref()
                    .map(|ch| {
                        matches!(
                            ch.kind,
                            ChannelType::PublicThread | ChannelType::PrivateThread
                        )
                    })
                    .unwrap_or(false);

                // Replace the old 2-button ephemeral with a 3-button version.
                let three_buttons = CreateActionRow::Buttons(vec![
                    CreateButton::new(format!("full_lb:{}:{}", cmd_name, gid))
                        .label("Full leaderboard")
                        .style(ButtonStyle::Primary),
                    CreateButton::new(format!("remove_lb:{}:{}", cmd_name, gid))
                        .label("Remove")
                        .style(ButtonStyle::Danger),
                    CreateButton::new(format!("remove_full_lb:{}:{}", cmd_name, gid))
                        .label("Remove full leaderboard")
                        .style(ButtonStyle::Danger),
                ]);
                let update = CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::new()
                        .content("Leaderboard actions:")
                        .components(vec![three_buttons]),
                );
                if let Err(e) = component.create_response(&ctx.http, update).await {
                    error!("Failed to update ephemeral with 3 buttons: {}", e);
                    return;
                }

                // Post the full leaderboard and track the message.
                if in_thread {
                    // Already in a thread — post the full embed directly here.
                    let msg = CreateMessage::new().embed(embed);
                    match ch_id.send_message(&ctx.http, msg).await {
                        Ok(posted) => {
                            self.store_full_leaderboard_msg(
                                gid,
                                cmd_key,
                                posted.channel_id,
                                posted.id,
                            );
                        }
                        Err(e) => {
                            error!("Failed to send full leaderboard in thread: {}", e);
                        }
                    }
                } else {
                    // Create a public thread on the summary message.
                    let thread_name = format!("{} — Full", leaderboard_title(cmd_name));
                    let thread_builder = CreateThread::new(&thread_name);
                    match ch_id
                        .create_thread_from_message(&ctx.http, msg_id, thread_builder)
                        .await
                    {
                        Ok(thread) => {
                            let thread_ch = thread.id;
                            let msg = CreateMessage::new().embed(embed);
                            match thread_ch.send_message(&ctx.http, msg).await {
                                Ok(posted) => {
                                    self.store_full_leaderboard_msg(
                                        gid,
                                        cmd_key,
                                        posted.channel_id,
                                        posted.id,
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to send full leaderboard to thread: {}",
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to create thread for full leaderboard: {}", e);
                        }
                    }
                }
            } else if let Some(rest) = custom_id.strip_prefix("remove_lb:") {
                // "Remove" button — delete the public summary message.
                let Some((cmd_name, gid_str)) = rest.split_once(':') else {
                    warn!("Malformed remove_lb custom_id: {}", custom_id);
                    return;
                };
                let Ok(gid) = gid_str.parse::<u64>() else {
                    warn!("Invalid guild_id in remove_lb custom_id: {}", custom_id);
                    return;
                };

                let cmd_key = cmd_name_key(cmd_name);
                let stored = self.take_prev_leaderboard_msg(gid, cmd_key);

                match stored {
                    Some((ch_id, msg_id, _)) => {
                        match ctx.http.delete_message(ch_id, msg_id, None).await {
                            Ok(_) => {
                                let ack = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Leaderboard removed.")
                                        .ephemeral(true),
                                );
                                let _ = component.create_response(&ctx.http, ack).await;
                            }
                            Err(e) => {
                                warn!("Failed to delete leaderboard message: {}", e);
                                let ack = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(
                                            "Could not delete the message \
                                             (it may have already been removed).",
                                        )
                                        .ephemeral(true),
                                );
                                let _ = component.create_response(&ctx.http, ack).await;
                            }
                        }
                    }
                    None => {
                        let ack = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(
                                    "The leaderboard message is no longer tracked \
                                     (it may have already been removed).",
                                )
                                .ephemeral(true),
                        );
                        let _ = component.create_response(&ctx.http, ack).await;
                    }
                }
            } else if let Some(rest) = custom_id.strip_prefix("remove_full_lb:") {
                // "Remove full leaderboard" button — delete the full leaderboard message.
                let Some((cmd_name, gid_str)) = rest.split_once(':') else {
                    warn!("Malformed remove_full_lb custom_id: {}", custom_id);
                    return;
                };
                let Ok(gid) = gid_str.parse::<u64>() else {
                    warn!("Invalid guild_id in remove_full_lb custom_id: {}", custom_id);
                    return;
                };

                let cmd_key = cmd_name_key(cmd_name);
                let stored = self.take_full_leaderboard_msg(gid, cmd_key);

                match stored {
                    Some((ch_id, full_msg_id)) => {
                        match ctx.http.delete_message(ch_id, full_msg_id, None).await {
                            Ok(_) => {
                                let ack = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Full leaderboard removed.")
                                        .ephemeral(true),
                                );
                                let _ = component.create_response(&ctx.http, ack).await;
                            }
                            Err(e) => {
                                warn!("Failed to delete full leaderboard message: {}", e);
                                let ack = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(
                                            "Could not delete the full leaderboard \
                                             (it may have already been removed).",
                                        )
                                        .ephemeral(true),
                                );
                                let _ = component.create_response(&ctx.http, ack).await;
                            }
                        }
                    }
                    None => {
                        let ack = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(
                                    "The full leaderboard message is no longer tracked \
                                     (it may have already been removed).",
                                )
                                .ephemeral(true),
                        );
                        let _ = component.create_response(&ctx.http, ack).await;
                    }
                }
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

/// Human-readable title for a leaderboard command.
fn leaderboard_title(name: &str) -> &'static str {
    match name {
        "leaderboard_daily" => "Daily Leaderboard",
        "leaderboard_permanent" => "Permanent Leaderboard",
        "leaderboard_challenge_daily" => "Daily Challenge Leaderboard",
        "leaderboard_challenge_permanent" => "Permanent Challenge Leaderboard",
        _ => "Leaderboard",
    }
}

// ── Embed constants ─────────────────────────────────────────────────────

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
fn build_description(count: usize, is_permanent: bool, is_challenge: bool) -> String {
    let url = leaderboard_url(is_challenge);
    if is_permanent {
        format!("All-time \u{00b7} {} players \u{00b7} {}", count, url)
    } else {
        let now = chrono::Utc::now();
        let day = now.format("%A, %B %-d").to_string();
        format!(
            "{} \u{00b7} {} players submitted \u{00b7} {}",
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
fn build_summary_embed(
    title: &str,
    rows: &[LeaderboardRow],
    is_permanent: bool,
    is_challenge: bool,
) -> CreateEmbed {
    let color = embed_color(is_challenge);
    let desc = build_description(rows.len(), is_permanent, is_challenge);
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
fn build_full_embed(
    title: &str,
    rows: &[LeaderboardRow],
    is_permanent: bool,
    is_challenge: bool,
) -> CreateEmbed {
    let color = embed_color(is_challenge);
    let desc = build_description(rows.len(), is_permanent, is_challenge);

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

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serenity::async_trait;
use serenity::builder::{
    CreateActionRow, CreateButton, CreateCommand, CreateCommandOption, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseFollowup,
    CreateInteractionResponseMessage, CreateMessage, CreateThread,
};
use serenity::model::application::{ButtonStyle, CommandOptionType, Interaction, ResolvedValue};
use serenity::model::channel::{ChannelType, Message};
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, GuildId, MessageId};
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::admin::{admin_commands, handle_admin_cmd};
use crate::db::{Database};
use crate::embed::{build_full_embed, build_summary_embed};
use crate::formatting::{daily_position_reactions, leaderboard_title};
use crate::models::GameMode;
use crate::parser::{parse_challenge_message, parse_date_str, parse_maptap_message};
use crate::help::build_help_text;

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
    pub(crate) channel_ids: Option<Vec<u64>>,
    /// Discord user IDs that have admin privileges.
    admin_ids: Vec<u64>,
    /// Optional guild ID where admin-only commands (e.g. /backup) are registered.
    /// When set, these commands are guild-specific and invisible to other servers.
    admin_guild_id: Option<u64>,
    /// Path to the SQLite database file, used for deriving backup paths.
    db_path: String,
}

fn handle_today_cmd() -> CreateInteractionResponse {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content("Today's challenge: https://maptap.gg/")
            .ephemeral(true),
    )
}

impl Handler {
    pub fn new(
        db: Database,
        channel_ids: Option<Vec<u64>>,
        admin_ids: Vec<u64>,
        admin_guild_id: Option<u64>,
        db_path: String,
    ) -> Self {
        Self {
            db: std::sync::Mutex::new(db),
            leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
            full_leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
            channel_ids,
            admin_ids,
            admin_guild_id,
            db_path,
        }
    }

    /// Check whether a Discord user ID is in the admin list.
    pub(crate) fn is_admin(&self, user_id: u64) -> bool {
        self.admin_ids.contains(&user_id)
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
    /// `date` is used for daily commands; `None` defaults to today (UTC).
    /// Returns `Ok(embed)` on success, or `Err(empty_state_message)` when there are no entries.
    fn build_leaderboard_embed(&self, name: &str, gid: u64, date: Option<NaiveDate>) -> Result<CreateEmbed, String> {
        let db = self.db.lock().unwrap();
        match name {
            "leaderboard_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                db.get_daily_leaderboard(gid, &date_str)
                    .map(|rows| {
                        if rows.is_empty() {
                            Err("No scores recorded for that day yet!".to_string())
                        } else {
                            Ok(build_summary_embed(
                                "Daily Leaderboard",
                                &rows,
                                false,
                                false,
                                Some(d),
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
                            None,
                        ))
                    }
                })
                .unwrap_or_else(|e| {
                    error!("DB error: {}", e);
                    Err("Internal error fetching leaderboard.".to_string())
                }),
            "leaderboard_challenge_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                db.get_daily_challenge_leaderboard(gid, &date_str)
                    .map(|rows| {
                        if rows.is_empty() {
                            Err("No challenge scores recorded for that day yet!".to_string())
                        } else {
                            Ok(build_summary_embed(
                                "Daily Challenge Leaderboard",
                                &rows,
                                false,
                                true,
                                Some(d),
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
                            None,
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
    /// `date` is used for daily commands; `None` defaults to today (UTC).
    fn build_full_leaderboard_embed(&self, name: &str, gid: u64, date: Option<NaiveDate>) -> Result<CreateEmbed, String> {
        let db = self.db.lock().unwrap();
        let (title, rows, is_permanent, is_challenge, resolved_date) = match name {
            "leaderboard_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                let rows = db.get_daily_leaderboard(gid, &date_str).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Daily Leaderboard — Full", rows, false, false, Some(d))
            }
            "leaderboard_permanent" => {
                let rows = db.get_permanent_leaderboard(gid).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Permanent Leaderboard — Full", rows, true, false, None)
            }
            "leaderboard_challenge_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                let rows = db
                    .get_daily_challenge_leaderboard(gid, &date_str)
                    .map_err(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })?;
                ("Daily Challenge Leaderboard — Full", rows, false, true, Some(d))
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
                    None,
                )
            }
            _ => return Err("Unknown leaderboard command.".to_string()),
        };

        if rows.is_empty() {
            return Err("No scores to display.".to_string());
        }

        Ok(build_full_embed(title, &rows, is_permanent, is_challenge, resolved_date))
    }

    /// Parse and store a single Discord message through the normal score pipeline.
    ///
    /// Returns:
    /// - `None`         — message doesn't match any maptap format (no-op)
    /// - `Some(Err(e))` — message matched but failed validation or DB write
    /// - `Some(Ok((user_id, final_score, mode, date)))` — score saved successfully
    pub(crate) async fn process_score_message(
        &self,
        user_id: u64,
        username: &str,
        guild_id: Option<u64>,
        channel_id: u64,
        channel_parent_id: Option<u64>,
        message_id: u64,
        posted_at: DateTime<Utc>,
        content: &str,
    ) -> Option<Result<(u64, u32, GameMode, NaiveDate), String>> {
        let result = parse_maptap_message(user_id, guild_id, content)
            .or_else(|| parse_challenge_message(user_id, guild_id, content))?;

        Some(match result {
            Ok(mut score) => {
                score.message_id = message_id;
                score.channel_id = channel_id;
                score.channel_parent_id = channel_parent_id;
                score.posted_at = posted_at;

                let score_date = score.date;
                let date_str = score_date.format("%Y-%m-%d").to_string();
                let final_score = score.final_score;
                let mode_label = match score.mode {
                    GameMode::DailyDefault => "default",
                    GameMode::DailyChallenge => "challenge",
                };
                let mode = score.mode.clone();

                let db_result = self
                    .db
                    .lock()
                    .map_err(|e| format!("Failed to lock DB: {}", e))
                    .and_then(|db| {
                        db.upsert_user(score.user_id, username)
                            .map_err(|e| format!("DB error (user): {}", e))?;
                        db.insert_score(&score)
                            .map_err(|e| format!("DB error (score): {}", e))
                    });

                if let Err(e) = db_result {
                    error!("{}", e);
                    return Some(Err(e));
                }

                info!(
                    "Saved {} score for user {} on {}: {}",
                    mode_label, username, date_str, final_score
                );

                Ok((user_id, final_score, mode, score_date))
            }
            Err(e) => Err(e),
        })
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
                    if  let Some(guild_id) = msg.guild_id
                        && let Some(guild) = ctx.cache.guild(guild_id) 
                    {
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

        // Detect thread parent: Some(parent_channel_id) if the message came from a thread.
        let channel_parent_id: Option<u64> = 'cpi: {
            if let Some(guild_id) = msg.guild_id {
                // Try cache first (no API call).
                if let Some(guild) = ctx.cache.guild(guild_id) {
                    let cached = guild
                        .channels
                        .get(&msg.channel_id)
                        .or_else(|| guild.threads.iter().find(|t| t.id == msg.channel_id));
                    if let Some(channel) = cached {
                        break 'cpi channel.parent_id.map(|pid| pid.get());
                    }
                    // Channel not found in guild cache — fall through to API.
                }
                // Fallback: fetch from Discord API.
                match msg.channel_id.to_channel(&ctx.http).await {
                    Ok(channel) => {
                        break 'cpi channel.guild().and_then(|gc| gc.parent_id).map(|pid| pid.get());
                    }
                    Err(e) => {
                        warn!("Failed to resolve channel {} for parent detection: {}", msg.channel_id, e);
                    }
                }
            }
            None
        };

        let posted_at: DateTime<Utc> = *msg.timestamp;

        let result = self
            .process_score_message(
                user_id,
                &msg.author.name,
                guild_id,
                msg.channel_id.get(),
                channel_parent_id,
                msg.id.get(),
                posted_at,
                &msg.content,
            )
            .await;

        match result {
            None => {} // Not a maptap message, ignore silently
            Some(Err(e)) => {
                warn!("Invalid maptap message from {}: {}", msg.author.name, e);
                let reply = format!("Invalid maptap score: {}", e);
                let _ = msg.reply(&ctx.http, reply).await;
            }
            Some(Ok((_, final_score, mode, score_date))) => {
                // Check if this user is on the hit list and suspiciously good.
                let on_hit_list = self
                    .db
                    .lock()
                    .ok()
                    .and_then(|db| db.is_on_hit_list(user_id).ok())
                    .unwrap_or(false);

                if on_hit_list && final_score > 800 {
                    let taunts = [
                        format!(
                            "Okay {} … {} points? OBVIOUSLY cheating. \
                            Did you just speedrun the map with your eyes closed or did you \
                            have the answers tattooed on your hand? Either way, suspicious. \
                            Very, VERY suspicious. 🕵️",
                            msg.author.name, final_score
                        ),
                        format!(
                            "{} scored {}?? Sure, totally believable. \
                            And I suppose you also just *happen* to know every capital city \
                            by heart, yeah? 🙄",
                            msg.author.name, final_score
                        ),
                        format!(
                            "Wow, {} points from {}! That's… impressive. \
                            Almost like someone had a little sneak peek before submitting. \
                            Not naming names. But it's you. It's definitely you. 👀",
                            final_score, msg.author.name
                        ),
                        format!(
                            "Breaking news: {} allegedly scores {} without any funny business. \
                            Sources describe the claim as 'laughable', 'deeply sus', \
                            and 'we're not buying it'. More at 11. 📰",
                            msg.author.name, final_score
                        ),
                        format!(
                            "{} really thought they could slide a {} past us. \
                            Honey, the audacity. The SHEER audacity. \
                            Your map knowledge isn't THAT good. 💅",
                            msg.author.name, final_score
                        ),
                    ];
                    let idx = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as usize)
                        .unwrap_or(0)
                        % taunts.len();
                    let _ = msg.reply(&ctx.http, &taunts[idx]).await;
                } else {
                    // React with 🗺️ to confirm the score was recorded.
                    let _ = msg.react(&ctx.http, '🗺').await;

                    // React with an additional emoji reflecting the player's daily rank.
                    if let Some(gid) = guild_id {
                        let date_str = score_date.format("%Y-%m-%d").to_string();
                        let uid_str = user_id.to_string();
                        let pos = self.db.lock().ok().and_then(|db| {
                            let rows = match mode {
                                GameMode::DailyDefault => {
                                    db.get_daily_leaderboard(gid, &date_str).ok()?
                                }
                                GameMode::DailyChallenge => {
                                    db.get_daily_challenge_leaderboard(gid, &date_str).ok()?
                                }
                            };
                            rows.iter().position(|r| r.user_id == uid_str).map(|i| i + 1)
                        });
                        if let Some(pos) = pos {
                            for reaction in daily_position_reactions(pos) {
                                let _ = msg.react(&ctx.http, reaction).await;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands = vec![
            // User-facing commands
            CreateCommand::new("today").description("Get a link to today's maptap challenge"),
            CreateCommand::new("leaderboard_daily")
                .description("Show a day's leaderboard for this server")
                .add_option(
                    CreateCommandOption::new(CommandOptionType::String, "date",
                        "DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, tomorrow/tmro/t — defaults to today")
                        .required(false),
                ),
            CreateCommand::new("leaderboard_permanent")
                .description("Show the all-time average leaderboard for this server"),
            CreateCommand::new("leaderboard_challenge_daily")
                .description("Show a day's challenge leaderboard for this server")
                .add_option(
                    CreateCommandOption::new(CommandOptionType::String, "date",
                        "DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, tomorrow/tmro/t — defaults to today")
                        .required(false),
                ),
            CreateCommand::new("leaderboard_challenge_permanent")
                .description("Show the all-time challenge leaderboard for this server"),
            CreateCommand::new("help").description("Show available commands"),
        ];

        if let Err(e) =
            serenity::model::application::Command::set_global_commands(&ctx.http, commands).await
        {
            error!("Failed to register slash commands: {}", e);
        } else {
            info!("Slash commands registered");
        }

        // Register admin-only commands as guild-specific on ADMIN_GUILD.
        if let Some(gid) = self.admin_guild_id {
            let guild_id = GuildId::new(gid);
            let admin_commands = admin_commands();
            if let Err(e) = guild_id.set_commands(&ctx.http, admin_commands).await {
                error!("Failed to register admin guild commands on {}: {}", gid, e);
            } else {
                info!("Admin guild commands registered on {}", gid);
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(cmd) => {
                let guild_id = cmd.guild_id.map(|g| g.get());
                let invoker_id = cmd.user.id.get();

                match cmd.data.name.as_str() {
                    "today" => {
                        let response = handle_today_cmd();
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

                        // Resolve optional date param for daily commands.
                        let is_daily = name == "leaderboard_daily" || name == "leaderboard_challenge_daily";
                        let resolved_date: Option<NaiveDate> = if is_daily {
                            let options = cmd.data.options();
                            let raw_date = options.iter().find_map(|o| {
                                if o.name == "date" {
                                    if let ResolvedValue::String(s) = o.value {
                                        return Some(s);
                                    }
                                }
                                None
                            });
                            let today = Utc::now().date_naive();
                            let date = match raw_date {
                                None => today,
                                Some("yesterday" | "yest" | "y") => today - chrono::Duration::days(1),
                                Some("tomorrow" | "tmro" | "t") => today + chrono::Duration::days(1),
                                Some(s) => match parse_date_str(s, today) {
                                    Some(d) => d,
                                    None => {
                                        let _ = cmd.create_response(
                                            &ctx.http,
                                            CreateInteractionResponse::Message(
                                                CreateInteractionResponseMessage::new()
                                                    .content("Unrecognised date format. Try DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, or tomorrow/tmro/t.")
                                                    .ephemeral(true),
                                            ),
                                        ).await;
                                        return;
                                    }
                                },
                            };
                            let max_date = today + chrono::Duration::weeks(1);
                            if date > max_date {
                                let _ = cmd.create_response(
                                    &ctx.http,
                                    CreateInteractionResponse::Message(
                                        CreateInteractionResponseMessage::new()
                                            .content("That date is too far in the future.")
                                            .ephemeral(true),
                                    ),
                                ).await;
                                return;
                            }
                            Some(date)
                        } else {
                            None
                        };

                        // Encode the resolved date (or today) into the button ID so that the
                        // "Full leaderboard" button shows the same day's data.
                        let date_str_for_button = resolved_date
                            .unwrap_or_else(|| Utc::now().date_naive())
                            .format("%Y-%m-%d")
                            .to_string();

                        let embed = match self.build_leaderboard_embed(name, gid, resolved_date) {
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
                        // The full_lb button encodes the date so the full view shows the same day.
                        let buttons = CreateActionRow::Buttons(vec![
                            CreateButton::new(format!("full_lb:{}:{}:{}", name, gid, date_str_for_button))
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
                        let response: CreateInteractionResponse = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(content)
                                .ephemeral(true),
                        );
                        if let Err(e) = cmd.create_response(&ctx.http, response).await {
                            error!("Failed to respond to /help: {}", e);
                        }
                    }
                    // ── Admin commands ───────────────────────────────────────
                    name @ ("delete_score"
                    | "invalidate_score"
                    | "list_scores"
                    | "list_all_scores"
                    | "list_users"
                    | "raw_score"
                    | "stats"
                    | "hit_list"
                    | "backup") => {
                        if !self.is_admin(invoker_id) {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("You do not have permission to use this command.")
                                    .ephemeral(true),
                            );
                            let _ = cmd.create_response(&ctx.http, response).await;
                            return;
                        }

                        let content = handle_admin_cmd(name, &cmd.data.options(), invoker_id, &self.db, &self.db_path);
                        let response = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(content)
                                .ephemeral(true),
                        );
                        if let Err(e) = cmd.create_response(&ctx.http, response).await {
                            error!("Failed to respond to /{}: {}", name, e);
                        }
                    }
                    "parse" => self.handle_parse_cmd(&ctx, &cmd).await,
                    _ => {}
                }
            }
            Interaction::Component(cmd) => {
                let custom_id = cmd.data.custom_id.clone();

                match custom_id.splitn(2, ":").collect::<Vec<_>>().as_slice() {
                    ["full_lb", rest] => {
                        // "Full leaderboard" button — create a thread and post the full list.
                        // Custom ID format: "full_lb:{cmd_name}:{gid}:{date}" (date optional for compat).
                        let Some((cmd_name, rest2)) = rest.split_once(':') else {
                            warn!("Malformed full_lb custom_id: {}", custom_id);
                            return;
                        };
                        let (gid_str, date_str) = rest2.split_once(':').unwrap_or((rest2, ""));
                        let Ok(gid) = gid_str.parse::<u64>() else {
                            warn!("Invalid guild_id in full_lb custom_id: {}", custom_id);
                            return;
                        };
                        let btn_date: Option<NaiveDate> = if date_str.is_empty() {
                            None
                        } else {
                            NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
                        };

                        // Build the full embed (re-queries the DB for the same date).
                        let embed = match self.build_full_leaderboard_embed(cmd_name, gid, btn_date) {
                            Ok(e) => e,
                            Err(msg) => {
                                let ack = CreateInteractionResponse::UpdateMessage(
                                    CreateInteractionResponseMessage::new().content(msg),
                                );
                                let _ = cmd.create_response(&ctx.http, ack).await;
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
                            let _ = cmd.create_response(&ctx.http, ack).await;
                            return;
                        };

                        let in_thread = cmd
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
                        // Carry the date forward so the button continues to show the same day.
                        let three_buttons = CreateActionRow::Buttons(vec![
                            CreateButton::new(format!("full_lb:{}:{}:{}", cmd_name, gid, date_str))
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
                        if let Err(e) = cmd.create_response(&ctx.http, update).await {
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
                    }
                    ["remove_lb", rest] => {
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
                                        let _ = cmd.create_response(&ctx.http, ack).await;
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
                                        let _ = cmd.create_response(&ctx.http, ack).await;
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
                                let _ = cmd.create_response(&ctx.http, ack).await;
                            }
                        }
                    }
                    ["remove_full_lb", rest] => {
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
                                        let _ = cmd.create_response(&ctx.http, ack).await;
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
                                        let _ = cmd.create_response(&ctx.http, ack).await;
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
                                let _ = cmd.create_response(&ctx.http, ack).await;
                            }
                        }
                    }
                    _ => warn!("Unknown component: {}", custom_id)
                }
            }
            _ => {}
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

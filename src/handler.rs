use chrono::{DateTime, NaiveDate, Utc};
use serenity::async_trait;
use serenity::builder::{
    CreateCommand, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage,
};
use serenity::model::application::Interaction;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::id::{ChannelId, GuildId};
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::discord_command_options::{DiscordCommandOption, channel_id_option, message_id_option};
use crate::db::Database;
use crate::formatting::daily_position_reactions;
use crate::models::GameMode;
use crate::parser::{parse_challenge_message, parse_maptap_message};
use crate::help::build_help_text;
use crate::plugin::Plugin;
use crate::sqlite_repo::SqliteRepository;

pub struct Handler {
    pub(crate) db: std::sync::Mutex<Database>,
    /// Optional allowlist of channel IDs. When `Some`, only messages from these
    /// channels are parsed. When `None`, all channels are processed.
    pub(crate) channel_ids: Option<Vec<u64>>,
    /// Discord user IDs that have admin privileges.
    admin_ids: Vec<u64>,
    /// Optional guild ID where admin-only commands (e.g. /backup) are registered.
    /// When set, these commands are guild-specific and invisible to other servers.
    admin_guild_id: Option<u64>,
    /// Optional channel ID (within the admin guild) where the bot writes log messages.
    /// Requires `DISCORD_ADMIN_GUILD_ID` to be set. When `None`, logging is suppressed.
    logging_channel_id: Option<u64>,
    /// Path to the SQLite database file, used for deriving backup paths.
    db_path: String,
    /// PostgreSQL connection URL for /sync_to_postgres. None if unconfigured.
    pub(crate) pg_url: Option<String>,

    plugins: Vec<Box<dyn Plugin>>,
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
        logging_channel_id: Option<u64>,
        db_path: String,
        pg_url: Option<String>,
        plugins: Vec<Box<dyn Plugin>>,
    ) -> Self {
        Self {
            db: std::sync::Mutex::new(db),
            channel_ids,
            admin_ids,
            admin_guild_id,
            logging_channel_id,
            db_path,
            pg_url,
            plugins,
        }
    }

    /// Check whether a Discord user ID is in the admin list.
    pub(crate) fn is_admin(&self, user_id: u64) -> bool {
        self.admin_ids.contains(&user_id)
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
                let _ = msg.react(&ctx.http, '❌').await;
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

        // Send message to discord logging channel if it exists stating it's ready to go
        if let Some(channel_id) = self.logging_channel_id {
            let _ = ChannelId::new(channel_id)
                .send_message(&ctx.http, CreateMessage::new().content("Ready to go!")).await;
        }

        // Non-admin plugins are registered globally alongside the builtin commands.
        let non_admin_plugin_cmds: Vec<CreateCommand> = self.plugins
            .iter()
            .filter(|p| !p.is_admin_plugin())
            .flat_map(|p| p.commands().into_iter().map(|pc| pc.command))
            .collect();

        let builtin_commands = vec![
            CreateCommand::new("today").description("Get a link to today's maptap challenge"),
            CreateCommand::new("help").description("Show available commands"),
        ];

        let all_global_commands: Vec<CreateCommand> = builtin_commands
            .into_iter()
            .chain(non_admin_plugin_cmds)
            .collect();

        match serenity::model::application::Command::set_global_commands(&ctx.http, all_global_commands).await {
            Err(e) => error!("Failed to register slash commands: {}", e),
            _ => info!("Slash commands registered"),
        }

        // Admin plugins + legacy admin commands are registered guild-specifically.
        if let Some(gid) = self.admin_guild_id {
            let guild_id = GuildId::new(gid);

            let admin_plugin_cmds: Vec<CreateCommand> = self.plugins
                .iter()
                .filter(|p| p.is_admin_plugin())
                .flat_map(|p| p.commands().into_iter().map(|pc| pc.command))
                .collect();

            // Legacy commands not yet migrated to a plugin.
            let legacy_guild_cmds = vec![
                CreateCommand::new("parse")
                    .description("Re-process an existing Discord message through the score pipeline")
                    .add_option(channel_id_option(DiscordCommandOption::IsRequired))
                    .add_option(message_id_option(DiscordCommandOption::IsRequired)),
                CreateCommand::new("sync_to_postgres")
                    .description("Copy all SQLite data to PostgreSQL (SQLite wins on conflicts)"),
            ];

            let all_guild_cmds: Vec<CreateCommand> = admin_plugin_cmds
                .into_iter()
                .chain(legacy_guild_cmds)
                .collect();

            match guild_id.set_commands(&ctx.http, all_guild_cmds).await {
                Err(e) => error!("Failed to register admin guild commands on {}: {}", gid, e),
                _ => info!("Admin guild commands registered on {}", gid),
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(cmd) => {
                let invoker_id = cmd.user.id.get();
                let cmd_name = cmd.data.name.as_str();
                let repo = SqliteRepository::new(&self.db);

                for plugin in &self.plugins {
                    if plugin.commands().iter().any(|pc| pc.name == cmd_name) {
                        if plugin.is_admin_plugin() && !self.is_admin(invoker_id) {
                            let _ = cmd.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("You do not have permission to use this command.")
                                        .ephemeral(true),
                                ),
                            ).await;
                            return;
                        }
                        plugin.handle_command(&ctx, &cmd, &repo).await;
                        return;
                    }
                }

                match cmd_name {
                    "today" => {
                        let response = handle_today_cmd();
                        if let Err(e) = cmd.create_response(&ctx.http, response).await {
                            error!("Failed to respond to /today: {}", e);
                        }
                    }
                    "help" => {
                        let user_cmds: Vec<(&str, &str)> = {
                            let mut cmds = vec![
                                ("today", "Get a link to today's maptap challenge"),
                                ("help", "Show available commands"),
                            ];
                            cmds.extend(
                                self.plugins.iter()
                                    .filter(|p| !p.is_admin_plugin())
                                    .flat_map(|p| p.commands().into_iter().map(|pc| (pc.name, pc.description)))
                            );
                            cmds
                        };
                        let admin_cmds: Vec<(&str, &str)> = self.plugins.iter()
                            .filter(|p| p.is_admin_plugin())
                            .flat_map(|p| p.commands().into_iter().map(|pc| (pc.name, pc.description)))
                            .collect();
                        let content = build_help_text(&user_cmds, &admin_cmds, self.is_admin(invoker_id));
                        let response: CreateInteractionResponse = CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(content)
                                .ephemeral(true),
                        );
                        if let Err(e) = cmd.create_response(&ctx.http, response).await {
                            error!("Failed to respond to /help: {}", e);
                        }
                    }
                    "parse" => self.handle_parse_cmd(&ctx, &cmd).await,
                    "sync_to_postgres" => self.handle_sync_to_postgres_cmd(&ctx, &cmd).await,
                    _ => {}
                }
            }
            Interaction::Component(interaction) => {
                let custom_id = &interaction.data.custom_id;
                let prefix = custom_id.split(':').next().unwrap_or("");
                let repo = SqliteRepository::new(&self.db);

                for plugin in &self.plugins {
                    if plugin.component_prefixes().contains(&prefix) {
                        if plugin.is_admin_plugin() && !self.is_admin(interaction.user.id.get()) {
                            let _ = interaction.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("You do not have permission to use this command.")
                                        .ephemeral(true),
                                ),
                            ).await;
                            return;
                        }
                        plugin.handle_component(&ctx, &interaction, &repo).await;
                        return;
                    }
                }

                warn!("Unknown component: {}", custom_id);
            }
            _ => {}
        }
    }
}

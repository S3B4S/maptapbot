use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc, Weekday};
use serenity::all::{
    CommandInteraction, ComponentInteraction, Context, CreateActionRow, CreateButton, CreateCommand,
    CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateMessage,
    CreateThread,
};
use serenity::async_trait;
use serenity::model::application::{ButtonStyle, CommandOptionType, ResolvedValue};
use serenity::model::channel::ChannelType;
use serenity::model::id::{ChannelId, MessageId};
use tracing::{error, warn};

use crate::embed::{
    build_full_embed, build_summary_embed, build_weekly_full_embed, build_weekly_summary_embed,
};
use crate::formatting::leaderboard_title;
use crate::parser::parse_date_str;
use crate::plugin::{Plugin, PluginCommand};
use crate::repository::Repository;

pub struct LeaderboardPlugin {
    leaderboard_msgs:
        std::sync::Mutex<HashMap<(u64, &'static str), (ChannelId, MessageId, u64)>>,
    full_leaderboard_msgs:
        std::sync::Mutex<HashMap<(u64, &'static str), (ChannelId, MessageId)>>,
}

impl LeaderboardPlugin {
    pub fn new() -> Self {
        Self {
            leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
            full_leaderboard_msgs: std::sync::Mutex::new(HashMap::new()),
        }
    }

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

    fn build_leaderboard_embed(
        &self,
        name: &str,
        gid: u64,
        date: Option<NaiveDate>,
        repo: &dyn Repository,
    ) -> Result<CreateEmbed, String> {
        match name {
            "leaderboard_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                repo.get_daily_leaderboard(gid, &date_str)
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
            "leaderboard_permanent" => repo
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
                repo.get_daily_challenge_leaderboard(gid, &date_str)
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
            "leaderboard_challenge_permanent" => repo
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

    fn build_full_leaderboard_embed(
        &self,
        name: &str,
        gid: u64,
        date: Option<NaiveDate>,
        repo: &dyn Repository,
    ) -> Result<CreateEmbed, String> {
        let (title, rows, is_permanent, is_challenge, resolved_date) = match name {
            "leaderboard_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                let rows = repo.get_daily_leaderboard(gid, &date_str).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Daily Leaderboard — Full", rows, false, false, Some(d))
            }
            "leaderboard_permanent" => {
                let rows = repo.get_permanent_leaderboard(gid).map_err(|e| {
                    error!("DB error: {}", e);
                    "Internal error fetching leaderboard.".to_string()
                })?;
                ("Permanent Leaderboard — Full", rows, true, false, None)
            }
            "leaderboard_challenge_daily" => {
                let d = date.unwrap_or_else(|| Utc::now().date_naive());
                let date_str = d.format("%Y-%m-%d").to_string();
                let rows = repo
                    .get_daily_challenge_leaderboard(gid, &date_str)
                    .map_err(|e| {
                        error!("DB error: {}", e);
                        "Internal error fetching leaderboard.".to_string()
                    })?;
                ("Daily Challenge Leaderboard — Full", rows, false, true, Some(d))
            }
            "leaderboard_challenge_permanent" => {
                let rows = repo
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

        Ok(build_full_embed(
            title,
            &rows,
            is_permanent,
            is_challenge,
            resolved_date,
        ))
    }

    fn build_weekly_leaderboard_embed(
        &self,
        gid: u64,
        week_start: NaiveDate,
        week_end: NaiveDate,
        week_year: i32,
        week_num: u32,
        is_current_week: bool,
        use_sum: bool,
        repo: &dyn Repository,
    ) -> Result<CreateEmbed, String> {
        let week_start_str = week_start.format("%Y-%m-%d").to_string();
        let week_end_str = week_end.format("%Y-%m-%d").to_string();
        repo.get_weekly_leaderboard(gid, &week_start_str, &week_end_str, use_sum)
            .map_err(|e| {
                error!("DB error: {}", e);
                "Internal error fetching leaderboard.".to_string()
            })
            .and_then(|rows| {
                if rows.is_empty() {
                    Err("No scores recorded for that week yet!".to_string())
                } else {
                    let title = if use_sum {
                        "Weekly Leaderboard (Sum)"
                    } else {
                        "Weekly Leaderboard"
                    };
                    Ok(build_weekly_summary_embed(
                        title,
                        &rows,
                        week_year,
                        week_num,
                        week_start,
                        week_end,
                        is_current_week,
                        use_sum,
                    ))
                }
            })
    }

    fn build_full_weekly_leaderboard_embed(
        &self,
        gid: u64,
        week_start: NaiveDate,
        week_end: NaiveDate,
        week_year: i32,
        week_num: u32,
        is_current_week: bool,
        use_sum: bool,
        repo: &dyn Repository,
    ) -> Result<CreateEmbed, String> {
        let week_start_str = week_start.format("%Y-%m-%d").to_string();
        let week_end_str = week_end.format("%Y-%m-%d").to_string();
        let rows = repo
            .get_weekly_leaderboard(gid, &week_start_str, &week_end_str, use_sum)
            .map_err(|e| {
                error!("DB error: {}", e);
                "Internal error fetching leaderboard.".to_string()
            })?;
        if rows.is_empty() {
            return Err("No scores to display.".to_string());
        }
        let title = if use_sum {
            "Weekly Leaderboard (Sum) — Full"
        } else {
            "Weekly Leaderboard — Full"
        };
        Ok(build_weekly_full_embed(
            title,
            &rows,
            week_year,
            week_num,
            week_start,
            week_end,
            is_current_week,
            use_sum,
        ))
    }

    async fn handle_daily_permanent(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
        name: &str,
        gid: u64,
        invoker_id: u64,
        repo: &dyn Repository,
    ) {
        // Resolve optional date param for daily commands.
        let is_daily =
            name == "leaderboard_daily" || name == "leaderboard_challenge_daily";
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
                        let _ = cmd
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Unrecognised date format. Try DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, or tomorrow/tmro/t.")
                                        .ephemeral(true),
                                ),
                            )
                            .await;
                        return;
                    }
                },
            };
            let max_date = today + chrono::Duration::weeks(1);
            if date > max_date {
                let _ = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("That date is too far in the future.")
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }
            Some(date)
        } else {
            None
        };

        let date_str_for_button = resolved_date
            .unwrap_or_else(|| Utc::now().date_naive())
            .format("%Y-%m-%d")
            .to_string();

        let embed = match self.build_leaderboard_embed(name, gid, resolved_date, repo) {
            Ok(e) => e,
            Err(msg) => {
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
        if let Some((ch_id, msg_id, _)) = self.take_prev_leaderboard_msg(gid, cmd_key) {
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
            CreateButton::new(format!(
                "full_lb:{}:{}:{}",
                name, gid, date_str_for_button
            ))
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

    async fn handle_weekly(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
        gid: u64,
        invoker_id: u64,
        repo: &dyn Repository,
    ) {
        let today = Utc::now().date_naive();
        let options = cmd.data.options();

        // Parse optional `week` parameter.
        let raw_week: Option<&str> = options.iter().find_map(|o| {
            if o.name == "week" {
                if let ResolvedValue::String(s) = o.value {
                    return Some(s);
                }
            }
            None
        });

        // Determine the Monday of the target ISO week.
        let week_monday: Option<NaiveDate> = match raw_week {
            None => {
                let iso = today.iso_week();
                NaiveDate::from_isoywd_opt(iso.year(), iso.week(), Weekday::Mon)
            }
            Some("last" | "l") => {
                let iso = today.iso_week();
                NaiveDate::from_isoywd_opt(iso.year(), iso.week(), Weekday::Mon)
                    .map(|m| m - chrono::Duration::weeks(1))
            }
            Some(s) => {
                let parsed = if let Some((n_str, y_str)) = s.split_once('-') {
                    let week_num = n_str.parse::<u32>().ok();
                    let year = y_str.parse::<i32>().ok();
                    week_num.zip(year).and_then(|(w, y)| {
                        NaiveDate::from_isoywd_opt(y, w, Weekday::Mon)
                    })
                } else {
                    s.parse::<u32>().ok().and_then(|w| {
                        NaiveDate::from_isoywd_opt(today.iso_week().year(), w, Weekday::Mon)
                    })
                };
                if parsed.is_none() {
                    let _ = cmd
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Unrecognised week format. Try a week number (16), N-YYYY (16-2026), or last/l.")
                                    .ephemeral(true),
                            ),
                        )
                        .await;
                    return;
                }
                parsed
            }
        };

        let Some(week_start) = week_monday else {
            let _ = cmd
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Invalid week number.")
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        };

        let week_sunday = week_start + chrono::Duration::days(6);
        let week_end = if week_sunday >= today {
            today
        } else {
            week_sunday
        };
        let is_current_week =
            week_end == today && today >= week_start && today <= week_sunday;
        let iso = week_start.iso_week();

        // Parse optional `scoring` parameter.
        let use_sum = options
            .iter()
            .any(|o| o.name == "scoring" && matches!(o.value, ResolvedValue::String("sum")));
        let scoring_str = if use_sum { "sum" } else { "avg" };

        let embed = match self.build_weekly_leaderboard_embed(
            gid,
            week_start,
            week_end,
            iso.year(),
            iso.week(),
            is_current_week,
            use_sum,
            repo,
        ) {
            Ok(e) => e,
            Err(msg) => {
                let _ = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(msg)
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }
        };

        let cmd_key = cmd_name_key("leaderboard_weekly");

        // Delete the previous leaderboard message for this command, if any.
        if let Some((ch_id, msg_id, _)) = self.take_prev_leaderboard_msg(gid, cmd_key) {
            let _ = ctx.http.delete_message(ch_id, msg_id, None).await;
        }

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed),
        );
        if let Err(e) = cmd.create_response(&ctx.http, response).await {
            error!("Failed to respond to /leaderboard_weekly: {}", e);
            return;
        }

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
                    "Failed to retrieve response message for /leaderboard_weekly: {}",
                    e
                );
            }
        }

        let week_start_str = week_start.format("%Y-%m-%d").to_string();
        let buttons = CreateActionRow::Buttons(vec![
            CreateButton::new(format!(
                "full_lb:leaderboard_weekly:{}:{}:{}",
                gid, week_start_str, scoring_str
            ))
            .label("Full leaderboard")
            .style(ButtonStyle::Primary),
            CreateButton::new(format!("remove_lb:leaderboard_weekly:{}", gid))
                .label("Remove")
                .style(ButtonStyle::Danger),
        ]);
        let followup = CreateInteractionResponseFollowup::new()
            .content("Leaderboard actions:")
            .components(vec![buttons])
            .ephemeral(true);
        if let Err(e) = cmd.create_followup(&ctx.http, followup).await {
            error!(
                "Failed to send button follow-up for /leaderboard_weekly: {}",
                e
            );
        }
    }

    async fn handle_full_lb(
        &self,
        ctx: &Context,
        interaction: &ComponentInteraction,
        repo: &dyn Repository,
    ) {
        let custom_id = &interaction.data.custom_id;
        let rest = custom_id.strip_prefix("full_lb:").unwrap_or("");

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

        // Build the full embed (re-queries the DB for the same date/week).
        let embed = if cmd_name == "leaderboard_weekly" {
            let (ws_str, agg_str) = date_str.split_once(':').unwrap_or((date_str, "avg"));
            let week_start = NaiveDate::parse_from_str(ws_str, "%Y-%m-%d")
                .unwrap_or_else(|_| Utc::now().date_naive());
            let use_sum = agg_str == "sum";
            let today = Utc::now().date_naive();
            let week_sunday = week_start + chrono::Duration::days(6);
            let week_end = if week_sunday >= today {
                today
            } else {
                week_sunday
            };
            let is_current_week =
                week_end == today && today >= week_start && today <= week_sunday;
            let iso = week_start.iso_week();
            match self.build_full_weekly_leaderboard_embed(
                gid,
                week_start,
                week_end,
                iso.year(),
                iso.week(),
                is_current_week,
                use_sum,
                repo,
            ) {
                Ok(e) => e,
                Err(msg) => {
                    let ack = CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content(msg),
                    );
                    let _ = interaction.create_response(&ctx.http, ack).await;
                    return;
                }
            }
        } else {
            match self.build_full_leaderboard_embed(cmd_name, gid, btn_date, repo) {
                Ok(e) => e,
                Err(msg) => {
                    let ack = CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content(msg),
                    );
                    let _ = interaction.create_response(&ctx.http, ack).await;
                    return;
                }
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
            let _ = interaction.create_response(&ctx.http, ack).await;
            return;
        };

        let in_thread = interaction
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
        if let Err(e) = interaction.create_response(&ctx.http, update).await {
            error!("Failed to update ephemeral with 3 buttons: {}", e);
            return;
        }

        // Post the full leaderboard and track the message.
        if in_thread {
            let msg = CreateMessage::new().embed(embed);
            match ch_id.send_message(&ctx.http, msg).await {
                Ok(posted) => {
                    self.store_full_leaderboard_msg(gid, cmd_key, posted.channel_id, posted.id);
                }
                Err(e) => {
                    error!("Failed to send full leaderboard in thread: {}", e);
                }
            }
        } else {
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
                            error!("Failed to send full leaderboard to thread: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create thread for full leaderboard: {}", e);
                }
            }
        }
    }

    async fn handle_remove_lb(
        &self,
        ctx: &Context,
        interaction: &ComponentInteraction,
    ) {
        let custom_id = &interaction.data.custom_id;
        let rest = custom_id.strip_prefix("remove_lb:").unwrap_or("");

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
                        let _ = interaction.create_response(&ctx.http, ack).await;
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
                        let _ = interaction.create_response(&ctx.http, ack).await;
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
                let _ = interaction.create_response(&ctx.http, ack).await;
            }
        }
    }

    async fn handle_remove_full_lb(
        &self,
        ctx: &Context,
        interaction: &ComponentInteraction,
    ) {
        let custom_id = &interaction.data.custom_id;
        let rest = custom_id.strip_prefix("remove_full_lb:").unwrap_or("");

        let Some((cmd_name, gid_str)) = rest.split_once(':') else {
            warn!("Malformed remove_full_lb custom_id: {}", custom_id);
            return;
        };
        let Ok(gid) = gid_str.parse::<u64>() else {
            warn!(
                "Invalid guild_id in remove_full_lb custom_id: {}",
                custom_id
            );
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
                        let _ = interaction.create_response(&ctx.http, ack).await;
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
                        let _ = interaction.create_response(&ctx.http, ack).await;
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
                let _ = interaction.create_response(&ctx.http, ack).await;
            }
        }
    }
}

#[async_trait]
impl Plugin for LeaderboardPlugin {
    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand {
                name: "leaderboard_daily",
                command: CreateCommand::new("leaderboard_daily")
                    .description("Show a day's leaderboard for this server")
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "date",
                            "DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, tomorrow/tmro/t — defaults to today",
                        )
                        .required(false),
                    ),
            },
            PluginCommand {
                name: "leaderboard_weekly",
                command: CreateCommand::new("leaderboard_weekly")
                    .description("Show a week's leaderboard for this server")
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "week",
                            "16, 16-2026, last/l — defaults to current week",
                        )
                        .required(false),
                    )
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "scoring",
                            "How to aggregate scores across the week",
                        )
                        .required(false)
                        .add_string_choice("Average", "avg")
                        .add_string_choice("Sum", "sum"),
                    ),
            },
            PluginCommand {
                name: "leaderboard_permanent",
                command: CreateCommand::new("leaderboard_permanent")
                    .description("Show the all-time average leaderboard for this server"),
            },
            PluginCommand {
                name: "leaderboard_challenge_daily",
                command: CreateCommand::new("leaderboard_challenge_daily")
                    .description("Show a day's challenge leaderboard for this server")
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "date",
                            "DD, DD-MM, DD-MM-YYYY, yesterday/yest/y, tomorrow/tmro/t — defaults to today",
                        )
                        .required(false),
                    ),
            },
            PluginCommand {
                name: "leaderboard_challenge_permanent",
                command: CreateCommand::new("leaderboard_challenge_permanent")
                    .description("Show the all-time challenge leaderboard for this server"),
            },
        ]
    }

    async fn handle_command(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
        repo: &dyn Repository,
    ) {
        let guild_id = cmd.guild_id.map(|g| g.get());
        let invoker_id = cmd.user.id.get();

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

        match cmd.data.name.as_str() {
            name @ ("leaderboard_daily"
            | "leaderboard_permanent"
            | "leaderboard_challenge_daily"
            | "leaderboard_challenge_permanent") => {
                self.handle_daily_permanent(ctx, cmd, name, gid, invoker_id, repo)
                    .await;
            }
            "leaderboard_weekly" => {
                self.handle_weekly(ctx, cmd, gid, invoker_id, repo).await;
            }
            _ => {}
        }
    }

    fn component_prefixes(&self) -> Vec<&'static str> {
        vec!["full_lb", "remove_lb", "remove_full_lb"]
    }

    async fn handle_component(
        &self,
        ctx: &Context,
        interaction: &ComponentInteraction,
        repo: &dyn Repository,
    ) {
        let custom_id = &interaction.data.custom_id;
        let prefix = custom_id.split(':').next().unwrap_or("");

        match prefix {
            "full_lb" => self.handle_full_lb(ctx, interaction, repo).await,
            "remove_lb" => self.handle_remove_lb(ctx, interaction).await,
            "remove_full_lb" => self.handle_remove_full_lb(ctx, interaction).await,
            _ => warn!("Unknown component prefix: {}", prefix),
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
        "leaderboard_weekly" => "leaderboard_weekly",
        _ => unreachable!("cmd_name_key called with unexpected name: {}", name),
    }
}

use chrono::{DateTime, Utc};
use serenity::all::{ChannelId, CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage, MessageId};
use tracing::{error, warn};

use crate::handler::Handler;

impl Handler {
    pub(crate) async fn handle_parse_cmd(&self, ctx: &Context, cmd: &CommandInteraction) {
        let invoker_id = cmd.user.id.get();

        if !self.is_admin(invoker_id) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You do not have permission to use this command.")
                    .ephemeral(true),
            );
            let _ = cmd.create_response(&ctx.http, response).await;
            return;
        }

        let options = cmd.data.options();
        let get_str = |key: &str| -> Option<&str> {
            options.iter().find_map(|o| {
                if o.name == key {
                    if let serenity::model::application::ResolvedValue::String(s) =
                        o.value
                    {
                        return Some(s);
                    }
                }
                None
            })
        };

        let Some(channel_id_str) = get_str("channel_id") else {
            let _ = cmd
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Missing required parameter: channel_id")
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        };
        let Some(message_id_str) = get_str("message_id") else {
            let _ = cmd
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("Missing required parameter: message_id")
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        };

        let ch_id = match channel_id_str.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                let _ = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "Invalid channel_id `{}`: must be a numeric ID.",
                                    channel_id_str
                                ))
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }
        };
        let msg_id = match message_id_str.parse::<u64>() {
            Ok(id) => id,
            Err(_) => {
                let _ = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "Invalid message_id `{}`: must be a numeric ID.",
                                    message_id_str
                                ))
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }
        };

        // Fetch the message from Discord.
        let fetched_msg = match ctx
            .http
            .get_message(ChannelId::new(ch_id), MessageId::new(msg_id))
            .await
        {
            Ok(m) => m,
            Err(e) => {
                let _ = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "Could not fetch message `{}` in channel `{}`: {}",
                                    msg_id, ch_id, e
                                ))
                                .ephemeral(true),
                        ),
                    )
                    .await;
                return;
            }
        };

        // Channel allowlist check — same rules as live message processing.
        if let Some(ref ids) = self.channel_ids {
            if !ids.contains(&ch_id) {
                let parent_allowed = match ChannelId::new(ch_id)
                    .to_channel(&ctx.http)
                    .await
                {
                    Ok(channel) => channel
                        .guild()
                        .and_then(|gc| gc.parent_id)
                        .map_or(false, |pid| ids.contains(&pid.get())),
                    Err(e) => {
                        warn!("Failed to resolve channel {}: {}", ch_id, e);
                        false
                    }
                };
                if !parent_allowed {
                    let _ = cmd
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(format!(
                                        "Channel `{}` is not in the allowed list.",
                                        ch_id
                                    ))
                                    .ephemeral(true),
                            ),
                        )
                        .await;
                    return;
                }
            }
        }

        // Derive guild_id: use the field on the fetched message if present,
        // otherwise resolve via the channel.
        let msg_guild_id = if let Some(gid) = fetched_msg.guild_id {
            Some(gid.get())
        } else {
            match ChannelId::new(ch_id).to_channel(&ctx.http).await {
                Ok(channel) => channel.guild().map(|gc| gc.guild_id.get()),
                Err(_) => None,
            }
        };

        // Detect thread parent for the parsed channel.
        let parse_channel_parent_id: Option<u64> =
            match ChannelId::new(ch_id).to_channel(&ctx.http).await {
                Ok(channel) => {
                    channel.guild().and_then(|gc| gc.parent_id).map(|pid| pid.get())
                }
                Err(_) => None,
            };

        let parse_posted_at: DateTime<Utc> = *fetched_msg.timestamp;

        let parse_result = self
            .process_score_message(
                fetched_msg.author.id.get(),
                &fetched_msg.author.name,
                msg_guild_id,
                ch_id,
                parse_channel_parent_id,
                msg_id,
                parse_posted_at,
                &fetched_msg.content,
            )
            .await;

        let content = match parse_result {
            None => "No maptap score found in that message.".to_string(),
            Some(Err(e)) => format!("Failed to process score: {}", e),
            Some(Ok((_, final_score))) => format!(
                "Score processed successfully (final score: {}).",
                final_score
            ),
        };

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        );
        if let Err(e) = cmd.create_response(&ctx.http, response).await {
            error!("Failed to respond to /parse: {}", e);
        }
    }
}

use serenity::all::{CommandInteraction, Context, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::async_trait;

use crate::plugin::{Plugin, PluginCommand};
use crate::repository::Repository;

pub struct TodayPlugin;

#[async_trait]
impl Plugin for TodayPlugin {
    fn commands(&self) -> Vec<PluginCommand> {
        vec![PluginCommand {
            name: "today",
            description: "Get a link to today's maptap challenge",
            command: CreateCommand::new("today").description("Get a link to today's maptap challenge"),
        }]
    }

    async fn handle_command(&self, ctx: &Context, cmd: &CommandInteraction, _repo: &dyn Repository) {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("Today's challenge: https://maptap.gg/")
                .ephemeral(true),
        );
        if let Err(e) = cmd.create_response(&ctx.http, response).await {
            tracing::error!("Plugin /today failed to respond: {}", e);
        }
    }
}

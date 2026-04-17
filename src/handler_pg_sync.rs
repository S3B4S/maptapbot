use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
};
use tracing::error;

use crate::handler::Handler;

impl Handler {
    pub(crate) async fn handle_sync_to_postgres_cmd(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
    ) {
        let invoker_id = cmd.user.id.get();

        if !self.is_admin(invoker_id) {
            let _ = cmd
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("You do not have permission to use this command.")
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        }

        let Some(pg_url) = self.pg_url.as_deref() else {
            let _ = cmd
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("POSTGRES_URL is not configured on this instance.")
                            .ephemeral(true),
                    ),
                )
                .await;
            return;
        };

        // Defer immediately — sync can exceed Discord's 3-second window.
        if let Err(e) = cmd
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                ),
            )
            .await
        {
            error!("Failed to defer /sync_to_postgres: {}", e);
            return;
        }

        let result = crate::pg_db::sync_sqlite_to_postgres(&self.db, pg_url).await;

        if let Err(e) = cmd
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .content(result)
                    .ephemeral(true),
            )
            .await
        {
            error!("Failed to send /sync_to_postgres followup: {}", e);
        }
    }
}

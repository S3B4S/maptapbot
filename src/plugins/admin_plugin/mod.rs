mod admin;

use serenity::all::{
    CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::async_trait;

use crate::discord_command_options::{DiscordCommandOption, message_id_option, user_id_option};
use crate::plugin::{Plugin, PluginCommand};
use crate::repository::Repository;

pub struct AdminPlugin {
    db_path: String,
}

impl AdminPlugin {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }
}

#[async_trait]
impl Plugin for AdminPlugin {
    fn is_admin_plugin(&self) -> bool {
        true
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![
            PluginCommand {
                name: "delete_score",
                description: "Delete a specific score entry",
                command: CreateCommand::new("delete_score")
                    .description("Delete a specific score entry")
                    .add_option(message_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "invalidate_score",
                description: "Mark a score entry invalid (soft-delete; prior valid score becomes effective)",
                command: CreateCommand::new("invalidate_score")
                    .description("Mark a score entry invalid (soft-delete; prior valid score becomes effective)")
                    .add_option(message_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "list_scores",
                description: "Show all scores for a given user",
                command: CreateCommand::new("list_scores")
                    .description("Show all scores for a given user")
                    .add_option(user_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "list_all_scores",
                description: "Dump all scores in the database",
                command: CreateCommand::new("list_all_scores")
                    .description("Dump all scores in the database"),
            },
            PluginCommand {
                name: "list_users",
                description: "List all known users",
                command: CreateCommand::new("list_users")
                    .description("List all known users"),
            },
            PluginCommand {
                name: "raw_score",
                description: "Show the raw stored message for a score entry",
                command: CreateCommand::new("raw_score")
                    .description("Show the raw stored message for a score entry")
                    .add_option(message_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "stats",
                description: "Show aggregate DB stats",
                command: CreateCommand::new("stats")
                    .description("Show aggregate DB stats"),
            },
            PluginCommand {
                name: "backup",
                description: "Create a timestamped backup of the database",
                command: CreateCommand::new("backup")
                    .description("Create a timestamped backup of the database"),
            },
            PluginCommand {
                name: "hit_list",
                description: "Manage the hit list of users to mess with",
                command: CreateCommand::new("hit_list")
                    .description("Manage the hit list of users to mess with")
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "action",
                            "read | add | delete",
                        )
                        .add_string_choice("read", "read")
                        .add_string_choice("add", "add")
                        .add_string_choice("delete", "delete")
                        .required(true),
                    )
                    .add_option(user_id_option(DiscordCommandOption::IsOptional)),
            },
            PluginCommand {
                name: "ban_user",
                description: "Ban a user (scores stored silently, hidden from leaderboards)",
                command: CreateCommand::new("ban_user")
                    .description("Ban a user (scores stored silently, hidden from leaderboards)")
                    .add_option(user_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "unban_user",
                description: "Unban a previously banned user",
                command: CreateCommand::new("unban_user")
                    .description("Unban a previously banned user")
                    .add_option(user_id_option(DiscordCommandOption::IsRequired)),
            },
            PluginCommand {
                name: "list_banned",
                description: "List all banned users",
                command: CreateCommand::new("list_banned")
                    .description("List all banned users"),
            },
        ]
    }

    async fn handle_command(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
        repo: &dyn Repository,
    ) {
        let content = admin::handle_admin_cmd(
            cmd.data.name.as_str(),
            &cmd.data.options(),
            cmd.user.id.get(),
            repo,
            &self.db_path,
        );
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        );
        if let Err(e) = cmd.create_response(&ctx.http, response).await {
            tracing::error!("AdminPlugin /{} failed to respond: {}", cmd.data.name, e);
        }
    }
}

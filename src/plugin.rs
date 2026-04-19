use serenity::all::{CommandInteraction, Context, CreateCommand, CreateInteractionResponse};

use crate::repository::Repository;

type CommandID = String;

pub type CommandHandler = fn(ctx: &Context, cmd: &CommandInteraction, repo: &dyn Repository) -> Result<CreateInteractionResponse, String>;
pub struct PluginCommand {
    pub command_name: String,
    pub command_description: String,
    pub handle_interaction: CommandHandler,
}

pub trait Plugin: Send + Sync {
    fn commands(&self) -> Vec<CreateCommand>;
    fn register_commands(&self) -> Vec<PluginCommand>;
}

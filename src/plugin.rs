use serenity::all::{CommandInteraction, ComponentInteraction, Context, CreateCommand};
use serenity::async_trait;

use crate::repository::Repository;

pub struct PluginCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub command: CreateCommand,
}

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Slash commands this plugin provides (used for both registration and dispatch).
    fn commands(&self) -> Vec<PluginCommand>;

    /// Handle a slash command interaction.
    async fn handle_command(
        &self,
        ctx: &Context,
        cmd: &CommandInteraction,
        repo: &dyn Repository,
    );

    /// Whether this plugin's commands are admin-only.
    /// When true:
    /// - Commands are registered guild-specifically on `admin_guild_id` (not globally).
    /// - Every command and component interaction is gated behind the admin ID check.
    /// Default: false (globally registered, no extra gate).
    fn is_admin_plugin(&self) -> bool {
        false
    }

    /// Button/component ID prefixes this plugin handles (e.g., "full_lb").
    /// Default: empty (no components).
    fn component_prefixes(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Handle a component (button) interaction.
    /// Default: no-op.
    async fn handle_component(
        &self,
        _ctx: &Context,
        _interaction: &ComponentInteraction,
        _repo: &dyn Repository,
    ) {
    }
}

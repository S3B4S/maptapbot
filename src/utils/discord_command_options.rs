use serenity::all::{CommandOptionType, CreateCommandOption};

pub enum DiscordCommandOption {
    IsRequired,
    IsOptional,
}

impl Default for DiscordCommandOption {
    fn default() -> Self {
        DiscordCommandOption::IsRequired
    }
}

pub fn user_id_option(required: DiscordCommandOption) -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::String,
        "user_id",
        "Discord user ID"
    )
    .required(matches!(required, DiscordCommandOption::IsRequired))
}

pub fn channel_id_option(required: DiscordCommandOption) -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::String,
        "channel_id",
        "Discord channel ID where the message lives",
    )
    .required(matches!(required, DiscordCommandOption::IsRequired))
}

pub fn message_id_option(required: DiscordCommandOption) -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::String,
        "message_id",
        "Discord message ID",
    )
    .required(matches!(required, DiscordCommandOption::IsRequired))
}

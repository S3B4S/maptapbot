use serenity::all::{CommandOptionType, CreateCommandOption};

pub fn user_id_option(required: bool) -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::String,
        "user_id",
        "Discord user ID"
    )
    .required(required)
}

pub fn message_id_option() -> CreateCommandOption {
    CreateCommandOption::new(
        CommandOptionType::String,
        "message_id",
        "Discord message ID of the score entry",
    )
    .required(true)
}

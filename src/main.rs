mod db;
mod handler;
mod models;
mod parser;

use handler::Handler;
use serenity::prelude::*;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenvy::dotenv().ok();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set in .env");

    let db = db::Database::open("maptap.db").expect("Failed to open database");
    info!("Database initialized");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(db))
        .await
        .expect("Failed to create Discord client");

    info!("Starting bot...");
    if let Err(e) = client.start().await {
        eprintln!("Client error: {e}");
    }
}

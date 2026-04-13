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

    let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "maptap.db".to_string());
    let db = db::Database::open(&db_path).expect("Failed to open database");
    info!("Database initialized at {}", db_path);

    // Parse optional comma-separated channel ID allowlist.
    let channel_ids: Option<Vec<u64>> = std::env::var("DISCORD_CHANNEL_IDS")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.split(',')
                .filter_map(|id| id.trim().parse::<u64>().ok())
                .collect()
        })
        .filter(|v: &Vec<u64>| !v.is_empty());

    if let Some(ref ids) = channel_ids {
        info!("Channel filter active: {:?}", ids);
    } else {
        info!("No channel filter set — processing messages from all channels");
    }

    // Parse optional comma-separated admin user IDs.
    let admin_ids: Vec<u64> = std::env::var("ADMIN_IDS")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.split(',')
                .filter_map(|id| id.trim().parse::<u64>().ok())
                .collect()
        })
        .unwrap_or_default();

    if admin_ids.is_empty() {
        info!("No admin IDs configured");
    } else {
        info!("Admin IDs: {:?}", admin_ids);
    }

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(db, channel_ids, admin_ids))
        .await
        .expect("Failed to create Discord client");

    info!("Starting bot...");
    if let Err(e) = client.start().await {
        eprintln!("Client error: {e}");
    }
}

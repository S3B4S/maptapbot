use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{error, info, warn};

use crate::db::Database;
use crate::parser::parse_maptap_message;

pub struct Handler {
    db: std::sync::Mutex<Database>,
}

impl Handler {
    pub fn new(db: Database) -> Self {
        Self {
            db: std::sync::Mutex::new(db),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Sanitize control characters (ANSI escape sequences, etc.) to prevent log injection
        let sanitized: String = msg.content.chars()
            .map(|c| if c.is_control() && c != '\n' { '?' } else { c })
            .collect();
        
        println!("{}", sanitized);

        // Ignore messages from bots (including ourselves)
        if msg.author.bot {
            return;
        }

        let user_id = msg.author.id.get();
        let content = &msg.content;

        let result = match parse_maptap_message(user_id, content) {
            Some(r) => r,
            None => return, // Not a maptap message, ignore silently
        };

        match result {
            Ok(score) => {
                let date_str = score.date.format("%Y-%m-%d").to_string();
                let final_score = score.final_score;

                // Scope the lock so it's dropped before any .await
                let db_result = self
                    .db
                    .lock()
                    .map_err(|e| format!("Failed to lock DB: {}", e))
                    .and_then(|db| {
                        db.upsert_score(&score)
                            .map_err(|e| format!("DB error: {}", e))
                    });

                if let Err(e) = db_result {
                    error!("{}", e);
                    let _ = msg
                        .reply(&ctx.http, "Internal error saving your score.")
                        .await;
                    return;
                }

                info!(
                    "Saved score for user {} on {}: {}",
                    msg.author.name, date_str, final_score
                );

                let reply = format!(
                    "Recorded! {} scored **{}** on {}",
                    msg.author.name, final_score, date_str
                );
                let _ = msg.reply(&ctx.http, reply).await;
            }
            Err(e) => {
                warn!(
                    "Invalid maptap message from {}: {}",
                    msg.author.name, e
                );
                let reply = format!("Invalid maptap score: {}", e);
                let _ = msg.reply(&ctx.http, reply).await;
            }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

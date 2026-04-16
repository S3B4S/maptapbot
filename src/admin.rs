use crate::db::{Database, DbStats, LeaderboardRow, ScoreRow, StatsDelta, StatsSnapshot};
use crate::formatting::truncate_username;

/// Format score rows into a code-block table, truncated to Discord's message limit.
/// Shows posted_at and an "INV" marker for invalidated rows so admins can see history.
fn format_score_rows(rows: &[ScoreRow]) -> String {
    let mut out = format!("**Scores ({} total)**\n```\n", rows.len());
    out.push_str(&format!(
        "{:<20} {:<14} {:<10} {:<16} {:>5} {:<19} {:<3}\n",
        "Message ID", "Username", "Date", "Mode", "Score", "Posted At", "Inv"
    ));
    out.push_str(&"-".repeat(94));
    out.push('\n');
    for row in rows {
        let username = truncate_username(&row.username, 14);
        let inv = if row.invalid { "X" } else { "" };
        out.push_str(&format!(
            "{:<20} {:<14} {:<10} {:<16} {:>5} {:<19} {:<3}\n",
            row.message_id, username, row.date, row.mode, row.final_score, row.posted_at, inv,
        ));
    }
    out.push_str("```");
    truncate_message(out)
}

/// Format a `DbStats` as the current-stats code block shown by `/stats`.
fn format_stats_block(stats: &DbStats) -> String {
    let date_range = match (&stats.min_date, &stats.max_date) {
        (Some(min), Some(max)) => format!("{} to {}", min, max),
        _ => "N/A".to_string(),
    };
    format!(
        "**DB Stats**\n```\n\
         Total entries:    {}\n\
         Invalidated:      {}\n\
         Unique users:     {}\n\
         Date range:       {}\n\
         daily_default:    {}\n\
         daily_challenge:  {}\n\
         ```",
        stats.total_entries,
        stats.invalid_entries,
        stats.unique_users,
        date_range,
        stats.daily_default_count,
        stats.daily_challenge_count,
    )
}

/// Format the "Since your last /stats" delta block.
fn format_delta_block(prev: &StatsSnapshot, current: &DbStats, delta: &StatsDelta) -> String {
    let elapsed = format_elapsed_since(&prev.taken_at);
    let signed = |d: i64| -> String {
        if d >= 0 {
            format!("+{}", d)
        } else {
            d.to_string()
        }
    };

    let d_total = current.total_entries - prev.stats.total_entries;
    let d_users = current.unique_users - prev.stats.unique_users;
    let d_default = current.daily_default_count - prev.stats.daily_default_count;
    let d_challenge = current.daily_challenge_count - prev.stats.daily_challenge_count;

    let mut out = format!(
        "**Since your last /stats** ({} ago)\n```\n\
         Total entries:    {}\n\
         Unique users:     {}\n\
         daily_default:    {}\n\
         daily_challenge:  {}\n",
        elapsed,
        signed(d_total),
        signed(d_users),
        signed(d_default),
        signed(d_challenge),
    );

    if prev.stats.min_date != current.min_date {
        out.push_str(&format!(
            "Min date:         {} → {}\n",
            prev.stats.min_date.as_deref().unwrap_or("N/A"),
            current.min_date.as_deref().unwrap_or("N/A"),
        ));
    }
    if prev.stats.max_date != current.max_date {
        out.push_str(&format!(
            "Max date:         {} → {}\n",
            prev.stats.max_date.as_deref().unwrap_or("N/A"),
            current.max_date.as_deref().unwrap_or("N/A"),
        ));
    }
    out.push_str("```");

    if delta.touched_count == 0 {
        out.push_str("\n_No new or updated submissions in that window._");
        return out;
    }

    out.push_str(&format!(
        "\n**{} new/updated submission{}**",
        delta.touched_count,
        if delta.touched_count == 1 { "" } else { "s" },
    ));

    if !delta.affected_dates.is_empty() {
        out.push_str(&format!(
            " across date{}: `{}`",
            if delta.affected_dates.len() == 1 { "" } else { "s" },
            delta.affected_dates.join("`, `"),
        ));
    }

    if !delta.new_users.is_empty() {
        let names: Vec<&str> = delta.new_users.iter().map(|(_, n)| n.as_str()).collect();
        out.push_str(&format!("\nNew users: `{}`", names.join("`, `")));
    }

    out
}

/// Discord messages have a 2000 character limit.
/// If the message exceeds it, truncate and add a note.
fn truncate_message(msg: String) -> String {
    const MAX: usize = 2000;
    if msg.len() <= MAX {
        return msg;
    }
    // Find how many rows we can keep. Cut before the closing ``` and add a note.
    let suffix = "\n... (truncated)\n```";
    let budget = MAX - suffix.len();
    // Find the last newline within budget.
    let cut = msg[..budget].rfind('\n').unwrap_or(budget);
    let mut truncated = msg[..cut].to_string();
    truncated.push_str(suffix);
    truncated
}


/// Render a humanized "X ago" string for a SQLite-formatted UTC timestamp
/// (`YYYY-MM-DD HH:MM:SS`). Falls back to the raw string if parsing fails.
fn format_elapsed_since(taken_at: &str) -> String {
    use chrono::{NaiveDateTime, Utc};
    let Ok(prev) = NaiveDateTime::parse_from_str(taken_at, "%Y-%m-%d %H:%M:%S") else {
        return format!("at {}", taken_at);
    };
    let now = Utc::now().naive_utc();
    let secs = (now - prev).num_seconds().max(0);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

pub fn handle_admin_cmd(
    name: &str,
    options: &[serenity::model::application::ResolvedOption<'_>],
    invoker_id: u64,
    db_param: &std::sync::Mutex<Database>,
    db_path: &str) -> String {
    let get_str = |key: &str| -> Option<&str> {
        options.iter().find_map(|o| {
            if o.name == key {
                if let serenity::model::application::ResolvedValue::String(s) = o.value {
                    return Some(s);
                }
            }
            None
        })
    };

    let db = match db_param.lock() {
        Ok(db) => db,
        Err(e) => return format!("Internal error: failed to lock DB: {}", e),
    };

    match name {
        "delete_score" => {
            let Some(message_id) = get_str("message_id") else {
                return "Missing required parameter: message_id".to_string();
            };
            match db.delete_score(message_id) {
                Ok(0) => format!("No score found for message_id `{}`.", message_id),
                Ok(n) => format!("Deleted {} score(s) for message_id `{}`.", n, message_id),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "invalidate_score" => {
            let Some(message_id) = get_str("message_id") else {
                return "Missing required parameter: message_id".to_string();
            };
            match db.invalidate_score(message_id) {
                Ok(0) => format!("No score found for message_id `{}`.", message_id),
                Ok(n) => format!(
                    "Marked {} score(s) as invalid for message_id `{}`. \
                        The prior valid score (if any) is now effective.",
                    n, message_id
                ),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "list_scores" => {
            let Some(user_id) = get_str("user_id") else {
                return "Missing required parameter: user_id".to_string();
            };
            match db.list_scores(user_id) {
                Ok(rows) if rows.is_empty() => format!("No scores found for user `{}`.", user_id),
                Ok(rows) => format_score_rows(&rows),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "list_all_scores" => match db.list_all_scores() {
            Ok(rows) if rows.is_empty() => "No scores in the database.".to_string(),
            Ok(rows) => format_score_rows(&rows),
            Err(e) => format!("DB error: {}", e),
        },
        "list_users" => match db.list_users() {
            Ok(rows) if rows.is_empty() => "No users in the database.".to_string(),
            Ok(rows) => {
                let mut out = format!("**Users ({} total)**\n```\n", rows.len());
                out.push_str(&format!("{:<22} {}\n", "User ID", "Username"));
                out.push_str(&"-".repeat(42));
                out.push('\n');
                for row in &rows {
                    out.push_str(&format!("{:<22} {}\n", row.user_id, row.username));
                }
                out.push_str("```");
                truncate_message(out)
            }
            Err(e) => format!("DB error: {}", e),
        },
        "raw_score" => {
            let Some(message_id) = get_str("message_id") else {
                return "Missing required parameter: message_id".to_string();
            };
            match db.raw_score(message_id) {
                Ok(Some(raw)) => format!(
                    "Raw message for message_id `{}`:\n```\n{}\n```",
                    message_id, raw
                ),
                Ok(None) => format!("No score found for message_id `{}`.", message_id),
                Err(e) => format!("DB error: {}", e),
            }
        }
        "stats" => {
            let current = match db.stats() {
                Ok(s) => s,
                Err(e) => return format!("DB error: {}", e),
            };
            let invoker_key = invoker_id.to_string();
            let prev = match db.get_stats_snapshot(&invoker_key) {
                Ok(p) => p,
                Err(e) => return format!("DB error: {}", e),
            };

            // Build the "current stats" block (same format as before).
            let current_block = format_stats_block(&current);

            // Build the "since your last /stats" block, if we have a baseline.
            let delta_block = match prev.as_ref() {
                Some(prev) => {
                    let delta = match db.scores_since(&prev.taken_at) {
                        Ok(d) => d,
                        Err(e) => return format!("DB error: {}", e),
                    };
                    format_delta_block(prev, &current, &delta)
                }
                None => {
                    "_No previous snapshot for you — baseline saved._".to_string()
                }
            };

            // Persist the new snapshot *after* reading the previous one.
            // Use SQLite's datetime format so lexical comparison against
            // `scores.created_at` works in future `scores_since` calls.
            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
            if let Err(e) = db.upsert_stats_snapshot(&invoker_key, &current, &now) {
                return format!("DB error: {}", e);
            }

            format!("{}\n{}", current_block, delta_block)
        }
        "backup" => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let backup_path = format!("{}.backup_{}", db_path, timestamp);

            match db.backup(&backup_path) {
                Ok(()) => format!("Backup created: `{}`", backup_path),
                Err(e) => format!("Backup failed: {}", e),
            }
        }
        "hit_list" => {
            let action = get_str("action").unwrap_or("");
            let user_id = get_str("user_id");
            match action {
                "read" => match db.get_hit_list() {
                    Ok(list) if list.is_empty() => "Hit list is empty.".to_string(),
                    Ok(list) => {
                        let lines: Vec<String> = list
                            .iter()
                            .map(|(id, name)| format!("{} ({})", name, id))
                            .collect();
                        format!("**Hit list ({}):**\n{}", list.len(), lines.join("\n"))
                    }
                    Err(e) => format!("DB error: {}", e),
                },
                "add" => match user_id {
                    None => "Provide a `user_id` to add.".to_string(),
                    Some(id) => match db.add_to_hit_list(id) {
                        Ok(()) => {
                            let name = db
                                .get_hit_list()
                                .ok()
                                .and_then(|l| l.into_iter().find(|(uid, _)| uid == id))
                                .map(|(_, n)| n)
                                .unwrap_or_else(|| id.to_string());
                            format!("Added {} ({}) to the hit list.", name, id)
                        }
                        Err(e) => format!("DB error: {}", e),
                    },
                },
                "delete" => match user_id {
                    None => "Provide a `user_id` to delete.".to_string(),
                    Some(id) => match db.remove_from_hit_list(id) {
                        Ok(0) => format!("User `{}` was not on the hit list.", id),
                        Ok(_) => format!("Removed `{}` from the hit list.", id),
                        Err(e) => format!("DB error: {}", e),
                    },
                },
                _ => "Unknown action. Use `read`, `add`, or `delete`.".to_string(),
            }
        }
        _ => "Unknown admin command.".to_string(),
    }
}

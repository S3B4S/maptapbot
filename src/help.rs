/// Build the /help response text. Admin commands are included only when `is_admin` is true.
pub fn build_help_text(is_admin: bool) -> String {
    let mut text = String::from("**Available Commands**\n\n");
    text.push_str("`/today` — Get a link to today's maptap challenge\n");
    text.push_str("`/leaderboard_daily` — Show today's scores for this server\n");
    text.push_str("`/leaderboard_permanent` — Show the all-time average scores for this server\n");
    text.push_str(
        "`/leaderboard_challenge_daily` — Show today's challenge scores for this server\n",
    );
    text.push_str("`/leaderboard_challenge_permanent` — Show the all-time challenge averages for this server\n");
    text.push_str("`/help` — Show this help message\n");

    if is_admin {
        text.push_str("\n**Admin Commands** (registered on the admin guild only)\n\n");
        text.push_str(
            "`/delete_score <message_id>` — Delete a specific score entry by message_id\n",
        );
        text.push_str(
            "`/list_scores <user_id>` — Show all scores for a given user across all dates and modes\n",
        );
        text.push_str("`/list_all_scores` — Dump the full contents of the scores table\n");
        text.push_str("`/list_users` — List all users known to the bot\n");
        text.push_str(
            "`/raw_score <message_id>` — Show the raw stored message for a score entry by message_id\n",
        );
        text.push_str(
            "`/invalidate_score <message_id>` — Soft-delete a score entry; prior valid score becomes effective\n",
        );
        text.push_str("`/stats` — Show aggregate DB stats\n");
        text.push_str("`/backup` — Create a timestamped backup of the database\n");
    }

    text
}

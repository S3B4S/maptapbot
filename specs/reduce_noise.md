# Reduce noise

To reduce the noise of the bot, I want to introduce 2 changes:
- Do not reply with a message to an user to confirm that the maptap score is recorded. Rather, just react with an emoji `:map:`.
- Whenever someone calls for a leaderboard, the previous leaderboard message should be deleted.
    - This is scoped per leaderboard, that is to say, `leaderboard_daily`/`leaderboard_permanent`/...etc each only has 1 active message up.

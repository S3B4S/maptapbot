# RF maptap leaderboard

## Description
We are creating a discord bot that will scan messages from RF, #random channel.

It needs to activate if it parses a message in the form of 

```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

Example:
```
www.maptap.gg April 13
93🏆 90👑 83😁 61🫢 97🔥
Final score: 823
```

Constraints;
- `score`: must be between `0-100` (inclusive both ends)
- `final-score`: can not exceed `1000`. The way final score is calculated is;
    - (first score + second score) * 1
    - third score * 2
    - (fourth score + fifth score) * 3

Through discord you can get an ID for an user. Use this ID to match the user to a score they posted.
A user can only post 1 score for each day. If an user posts multiple (valid) scores for 1 day, assume that the latest post wins and overwrites the previous score.

Let's start by just storing these scores in a small local DB.

## Tech stack
- Rust
- Discord API library in Rust: https://github.com/serenity-rs/serenity
- Small lightweight DB, please recommend me some

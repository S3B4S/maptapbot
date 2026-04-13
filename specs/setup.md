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

## Message
The message to parse must include 
```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

However, it does _not_ need to be the sole text in the message.

Text before it is allowed
```
this is horrible
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

Even on the same line
```
this is horrible www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

Text after it is allowed
```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
this is amazing
```

And once again, also on the same line as last line 
```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score> this is amazing
```

But the 3 lines cannot be interrupted, these are all invalid examples;
```
www.maptap.gg <month> <day> This sucks
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

```
www.maptap.gg <month> <day>
<score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji> wow I did so well today
Final score: <final-score>
```

```
www.maptap.gg <month> <day>
nahh I'm ebarassed <score><emoji> <score><emoji> <score><emoji> <score><emoji> <score><emoji>
Final score: <final-score>
```

## Database

Through discord you can get an ID for an user. Use this ID to match the user to a score they posted.
A user can only post 1 score for each day. If an user posts multiple (valid) scores for 1 day, assume that the latest post wins and overwrites the previous score.

Let's start by just storing these scores in a small local DB.

## Tech stack
- Rust
- Discord API library in Rust: https://github.com/serenity-rs/serenity
- Small lightweight DB, please recommend me some

# Challenge mode
There's another kind of maptap that people can do, the challenge mode. The message for this looks slightly different:

```
⚡ MapTap Challenge Round - Apr 12
www.maptap.gg/challenge
89🎉 82✨ 94🏆 88🎓 97🏅
Score: 914 in 21.1s (4.0s to spare!)
```

As you can see this introduces some changes.

The `scores` table should also include a `mode` column.
- `daily_default` is the default daily mode that is played when the user visits `https://maptap.gg`.
- `daily_challenge` is the daily challenge that users can play at `https://maptap.gg/challenge`.

An extra column should be added for `time_spent`. This represents an integer in miliseconds `21.1s` => `21100`.

The `(4.0s to spare!)` can be ignored when storing in DB, as it's just `25s - time_spent`, so we can derive it.

`daily_default` has no `time_spent` so value can be `NULL` there.

We need a migration for the DB as it is, as I don't want to destroy the current existing data.

This will also add 2 more commands

```
/leaderboard_challenge_daily
/leaderboard_challenge_permanent
```

These should report the leaderboard only for the challenge mode.

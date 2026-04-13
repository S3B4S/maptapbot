use chrono::{Datelike, NaiveDate, Utc};

use crate::models::MaptapScore;

/// Attempt to parse a maptap score message.
///
/// The 3-line maptap block can appear anywhere in a message:
/// - Text before `www.maptap.gg` is allowed (even on the same line)
/// - Text after `Final score: <N>` is allowed (even on the same line)
/// - But: nothing may follow `www.maptap.gg <month> <day>` on line 1
/// - And: nothing may follow the scores+emojis on line 2
/// - The 3 lines must be consecutive (no interruptions)
///
/// Returns None if the message doesn't contain a maptap block.
/// Returns Some(Err) if it looks like a maptap block but has validation issues.
/// Returns Some(Ok) if parsing and validation both succeed.
pub fn parse_maptap_message(
    user_id: u64,
    guild_id: Option<u64>,
    content: &str,
) -> Option<Result<MaptapScore, String>> {
    let lines: Vec<&str> = content.trim().lines().collect();
    if lines.len() < 3 {
        return None;
    }

    // Find the line containing "www.maptap.gg" and try to parse the header from it.
    // Text before "www.maptap.gg" on the same line is allowed, but nothing after the date.
    let mut header_idx = None;
    let mut header_portion = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(hp) = extract_header_portion(line) {
            header_idx = Some(i);
            header_portion = Some(hp);
            break;
        }
    }
    let header_idx = header_idx?;
    let header_portion = header_portion?;

    // Need at least 2 more lines after the header
    if header_idx + 2 >= lines.len() {
        return None;
    }

    let date = parse_header(&header_portion)?;

    // Line 2 (scores): must be the entire line — no extra text allowed
    let scores_line = lines[header_idx + 1].trim();
    let scores = match parse_scores_line(scores_line) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };

    // Line 3 (final score): "Final score: <N>" possibly followed by trailing text
    let final_line = lines[header_idx + 2].trim();
    let final_score = match parse_final_score(final_line) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };

    let raw_message = format!("{}\n{}\n{}", header_portion, scores_line, final_line);
    let score = MaptapScore {
        user_id,
        guild_id,
        date,
        scores,
        final_score,
        raw_message,
    };

    Some(score.validate().map(|_| score))
}

/// Extract the "www.maptap.gg <month> <day>" portion from a line.
/// Text before "www.maptap.gg" is allowed.
/// Nothing may follow "<day>" — the header must end the line.
fn extract_header_portion(line: &str) -> Option<String> {
    let idx = line.find("www.maptap.gg")?;
    let portion = line[idx..].trim();
    // Verify it's exactly "www.maptap.gg <month> <day>" with nothing trailing
    let after_prefix = portion.strip_prefix("www.maptap.gg ")?;
    let parts: Vec<&str> = after_prefix.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    // Verify the day part has no trailing non-whitespace
    // (split_whitespace already handles this, but confirm month is valid)
    parse_month(parts[0])?;
    parts[1].parse::<u32>().ok()?;
    Some(portion.to_string())
}

fn parse_month(s: &str) -> Option<u32> {
    match s.to_lowercase().as_str() {
        "january" => Some(1),
        "february" => Some(2),
        "march" => Some(3),
        "april" => Some(4),
        "may" => Some(5),
        "june" => Some(6),
        "july" => Some(7),
        "august" => Some(8),
        "september" => Some(9),
        "october" => Some(10),
        "november" => Some(11),
        "december" => Some(12),
        _ => None,
    }
}

fn parse_header(line: &str) -> Option<NaiveDate> {
    // Expect: "www.maptap.gg April 13"
    let rest = line.strip_prefix("www.maptap.gg ")?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let month = parse_month(parts[0])?;
    let day: u32 = parts[1].parse().ok()?;
    let year = Utc::now().year();

    NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_scores_line(line: &str) -> Result<[u32; 5], String> {
    // Line like: "93🏆 90👑 83😁 61🫢 97🔥"
    // Each token is <digits><emoji(s)>. No text allowed before or after the score tokens.
    //
    // Reject leading text: line must start with a digit (after trimming whitespace).
    if line.is_empty() || !line.chars().next().unwrap().is_ascii_digit() {
        return Err("Scores line must start with a digit".to_string());
    }
    // Strategy: extract numeric sequences. After the last digit->emoji transition,
    // only whitespace (or end of string) is allowed — no ASCII letters.
    let mut scores = Vec::new();
    let mut current_num = String::new();
    let mut last_num_end = 0; // byte index after last number was consumed

    for (i, ch) in line.char_indices() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else if !current_num.is_empty() {
            scores.push(
                current_num
                    .parse::<u32>()
                    .map_err(|e| format!("Failed to parse score '{}': {}", current_num, e))?,
            );
            last_num_end = i;
            current_num.clear();
        }
    }
    if !current_num.is_empty() {
        scores.push(
            current_num
                .parse::<u32>()
                .map_err(|e| format!("Failed to parse score '{}': {}", current_num, e))?,
        );
        last_num_end = line.len();
    }

    if scores.len() != 5 {
        return Err(format!("Expected 5 scores, found {}", scores.len()));
    }

    // Check that nothing after the 5th score's emoji is alphabetic text.
    // We find where the 5th score ended and scan the remainder.
    let remainder = &line[last_num_end..];
    if remainder.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err("Unexpected text after scores".to_string());
    }

    Ok([scores[0], scores[1], scores[2], scores[3], scores[4]])
}

fn parse_final_score(line: &str) -> Result<u32, String> {
    // Must start with "Final score:" (case-insensitive on Score)
    // Trailing text after the number is allowed: "Final score: 823 this is amazing"
    let rest = line
        .strip_prefix("Final score:")
        .or_else(|| line.strip_prefix("Final Score:"))
        .ok_or_else(|| "Expected line starting with 'Final score:'")?;

    let rest = rest.trim_start();
    // Extract leading digits
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if num_str.is_empty() {
        return Err("No number found after 'Final score:'".to_string());
    }

    num_str
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse final score: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Option<u64> = Some(100);

    // ── Basic valid ──────────────────────────────────────────────

    #[test]
    fn test_parse_valid_message() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let result = parse_maptap_message(12345, G, msg);
        assert!(result.is_some());
        let score = result.unwrap().unwrap();
        assert_eq!(score.scores, [93, 90, 83, 61, 97]);
        assert_eq!(score.final_score, 823);
        assert_eq!(score.date.month(), 4);
        assert_eq!(score.date.day(), 13);
        assert_eq!(score.guild_id, G);
    }

    #[test]
    fn test_parse_not_maptap() {
        let msg = "hello world";
        assert!(parse_maptap_message(1, G, msg).is_none());
    }

    // ── Validation errors ────────────────────────────────────────

    #[test]
    fn test_parse_wrong_score_count() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁\nFinal score: 823";
        let result = parse_maptap_message(1, G, msg);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_final_score_mismatch() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 999";
        let result = parse_maptap_message(1, G, msg);
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_parse_score_out_of_range() {
        let msg = "www.maptap.gg April 13\n150🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let result = parse_maptap_message(1, G, msg);
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert!(err.contains("must be 0-100"));
    }

    // ── Text BEFORE the block (allowed) ──────────────────────────

    #[test]
    fn test_text_before_on_separate_line() {
        let msg =
            "this is horrible\nwww.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert_eq!(score.final_score, 823);
    }

    #[test]
    fn test_text_before_on_same_line() {
        let msg =
            "this is horrible www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert_eq!(score.final_score, 823);
    }

    // ── Text AFTER the block (allowed) ───────────────────────────

    #[test]
    fn test_text_after_on_separate_line() {
        let msg =
            "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823\nthis is amazing";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert_eq!(score.final_score, 823);
    }

    #[test]
    fn test_text_after_on_same_line_as_final_score() {
        let msg =
            "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823 this is amazing";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert_eq!(score.final_score, 823);
    }

    // ── Invalid: text interrupting the 3 lines ──────────────────

    #[test]
    fn test_invalid_text_after_header_on_same_line() {
        let msg = "www.maptap.gg April 13 This sucks\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        assert!(parse_maptap_message(1, G, msg).is_none());
    }

    #[test]
    fn test_invalid_text_after_scores_on_same_line() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥 wow I did so well today\nFinal score: 823";
        let result = parse_maptap_message(1, G, msg);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_invalid_text_before_scores_on_same_line() {
        let msg = "www.maptap.gg April 13\nnahh I'm embarrassed 93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let result = parse_maptap_message(1, G, msg);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    // ── Raw message safety ───────────────────────────────────────

    #[test]
    fn test_raw_message_exact_content() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert_eq!(
            score.raw_message,
            "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823"
        );
    }

    #[test]
    fn test_raw_message_excludes_surrounding_text() {
        let msg = "Hey everyone!\nwww.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823\nSee you tomorrow";
        let score = parse_maptap_message(1, G, msg).unwrap().unwrap();
        assert!(!score.raw_message.contains("Hey everyone"));
        assert!(!score.raw_message.contains("See you tomorrow"));
    }

    // ── Edge: not enough lines after header ──────────────────────

    #[test]
    fn test_header_not_enough_lines_after() {
        let msg = "some text\nmore text\nwww.maptap.gg April 13";
        assert!(parse_maptap_message(1, G, msg).is_none());
    }
}

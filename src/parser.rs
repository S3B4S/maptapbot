use chrono::{Datelike, NaiveDate, Utc};

use crate::models::MaptapScore;

/// Attempt to parse a maptap score message.
/// Returns None if the message doesn't match the expected format.
/// Returns Some(Err) if it looks like a maptap message but has validation issues.
/// Returns Some(Ok) if parsing and validation both succeed.
pub fn parse_maptap_message(user_id: u64, content: &str) -> Option<Result<MaptapScore, String>> {
    let lines: Vec<&str> = content.trim().lines().map(|l| l.trim()).collect();
    if lines.len() < 3 {
        return None;
    }

    // Line 1: "www.maptap.gg <month> <day>"
    let date = match parse_header(lines[0]) {
        Some(d) => d,
        None => return None,
    };

    // Line 2: scores with emojis
    let scores = match parse_scores_line(lines[1]) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };

    // Line 3: "Final score: <N>"
    let final_score = match parse_final_score(lines[2]) {
        Ok(s) => s,
        Err(e) => return Some(Err(e)),
    };

    let score = MaptapScore {
        user_id,
        date,
        scores,
        final_score,
        raw_message: content.to_string(),
    };

    Some(score.validate().map(|_| score))
}

fn parse_header(line: &str) -> Option<NaiveDate> {
    // Expect: "www.maptap.gg April 13"
    let rest = line.strip_prefix("www.maptap.gg ")?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let month = match parts[0].to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None,
    };

    let day: u32 = parts[1].parse().ok()?;
    let year = Utc::now().year();

    NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_scores_line(line: &str) -> Result<[u32; 5], String> {
    // Line like: "93🏆 90👑 83😁 61🫢 97🔥"
    // Strategy: extract all numeric sequences from the line
    let mut scores = Vec::new();
    let mut current_num = String::new();

    for ch in line.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else if !current_num.is_empty() {
            scores.push(
                current_num
                    .parse::<u32>()
                    .map_err(|e| format!("Failed to parse score '{}': {}", current_num, e))?,
            );
            current_num.clear();
        }
    }
    // Handle trailing number (shouldn't happen with emoji suffix, but just in case)
    if !current_num.is_empty() {
        scores.push(
            current_num
                .parse::<u32>()
                .map_err(|e| format!("Failed to parse score '{}': {}", current_num, e))?,
        );
    }

    if scores.len() != 5 {
        return Err(format!("Expected 5 scores, found {}", scores.len()));
    }

    Ok([scores[0], scores[1], scores[2], scores[3], scores[4]])
}

fn parse_final_score(line: &str) -> Result<u32, String> {
    let rest = line
        .strip_prefix("Final score:")
        .or_else(|| line.strip_prefix("Final Score:"))
        .ok_or_else(|| "Expected line starting with 'Final score:'")?;

    rest.trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse final score: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_message() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let result = parse_maptap_message(12345, msg);
        assert!(result.is_some());
        let score = result.unwrap().unwrap();
        assert_eq!(score.scores, [93, 90, 83, 61, 97]);
        assert_eq!(score.final_score, 823);
        assert_eq!(score.date.month(), 4);
        assert_eq!(score.date.day(), 13);
    }

    #[test]
    fn test_parse_not_maptap() {
        let msg = "hello world";
        assert!(parse_maptap_message(1, msg).is_none());
    }

    #[test]
    fn test_parse_wrong_score_count() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁\nFinal score: 823";
        let result = parse_maptap_message(1, msg);
        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    #[test]
    fn test_parse_final_score_mismatch() {
        let msg = "www.maptap.gg April 13\n93🏆 90👑 83😁 61🫢 97🔥\nFinal score: 999";
        let result = parse_maptap_message(1, msg);
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert!(err.contains("mismatch"));
    }

    #[test]
    fn test_parse_score_out_of_range() {
        let msg = "www.maptap.gg April 13\n150🏆 90👑 83😁 61🫢 97🔥\nFinal score: 823";
        let result = parse_maptap_message(1, msg);
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert!(err.contains("must be 0-100"));
    }
}

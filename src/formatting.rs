/// Truncate a username to `max_len` characters, appending ".." if truncated.
pub fn truncate_username(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        let mut truncated = name[..max_len - 2].to_string();
        truncated.push_str("..");
        truncated
    }
}

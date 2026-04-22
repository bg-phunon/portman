pub fn truncate_chars(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }

    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }

    let take_len = max.saturating_sub(1);
    let mut truncated: String = s.chars().take(take_len).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::truncate_chars;

    #[test]
    fn truncate_is_unicode_safe() {
        assert_eq!(truncate_chars("microservice", 5), "micr…");
        assert_eq!(truncate_chars("บริการไทย", 5), "บริก…");
        assert_eq!(truncate_chars("ok", 5), "ok");
    }
}

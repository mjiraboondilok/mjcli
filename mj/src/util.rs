pub(crate) fn nonempty_trimmed(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn nonempty_arg(s: &str) -> Result<String, String> {
    nonempty_trimmed(s).ok_or_else(|| "value cannot be empty".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonempty_trimmed_trims_and_rejects_empty() {
        assert_eq!(nonempty_trimmed("  rnd_abc  "), Some("rnd_abc".to_owned()));
        assert_eq!(nonempty_trimmed("   "), None);
        assert_eq!(nonempty_trimmed(""), None);
    }

    #[test]
    fn nonempty_arg_trims_value_and_errors_on_empty() {
        assert_eq!(nonempty_arg("  rnd_abc  "), Ok("rnd_abc".to_owned()));
        assert!(nonempty_arg("   ").is_err());
        assert!(nonempty_arg("").is_err());
    }
}

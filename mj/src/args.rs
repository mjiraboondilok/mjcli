pub(crate) fn nonempty_trimmed(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn parse_single_value_flag(
    args: &[String],
    flag: &str,
) -> Result<Option<String>, String> {
    let mut rest = args.iter();
    let mut value: Option<String> = None;

    while let Some(arg) = rest.next() {
        let raw = if arg == flag {
            rest.next()
                .ok_or_else(|| format!("error: {flag} requires a value"))?
                .as_str()
        } else if let Some(v) = arg.strip_prefix(flag).and_then(|r| r.strip_prefix('=')) {
            v
        } else {
            return Err(format!("error: unexpected argument '{arg}'"));
        };

        if value.is_some() {
            return Err(format!("error: {flag} given more than once"));
        }
        value = Some(nonempty_trimmed(raw).ok_or_else(|| format!("error: {flag} value is empty"))?);
    }

    Ok(value)
}

pub(crate) fn parse_single_value_flag_or_usage(
    args: &[String],
    flag: &str,
    usage: &str,
) -> Result<Option<String>, std::process::ExitCode> {
    parse_single_value_flag(args, flag).map_err(|e| {
        eprintln!("{e}");
        eprintln!("{usage}");
        std::process::ExitCode::FAILURE
    })
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

    fn parse(args: &[&str], flag: &str) -> Result<Option<String>, String> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_single_value_flag(&owned, flag)
    }

    #[test]
    fn no_args_yields_no_value() {
        assert_eq!(parse(&[], "--api-key"), Ok(None));
    }

    #[test]
    fn separate_and_inline_forms_both_parse() {
        assert_eq!(
            parse(&["--api-key", "rnd_abc"], "--api-key"),
            Ok(Some("rnd_abc".into()))
        );
        assert_eq!(
            parse(&["--api-key=rnd_abc"], "--api-key"),
            Ok(Some("rnd_abc".into()))
        );
    }

    #[test]
    fn value_is_trimmed() {
        assert_eq!(
            parse(&["--api-key", "  rnd_abc  "], "--api-key"),
            Ok(Some("rnd_abc".into()))
        );
    }

    #[test]
    fn missing_value_is_an_error() {
        assert!(parse(&["--api-key"], "--api-key").is_err());
    }

    #[test]
    fn empty_value_is_an_error() {
        assert!(parse(&["--api-key="], "--api-key").is_err());
        assert!(parse(&["--api-key", "   "], "--api-key").is_err());
    }

    #[test]
    fn duplicate_flag_is_an_error() {
        assert!(parse(&["--api-key", "a", "--api-key", "b"], "--api-key").is_err());
    }

    #[test]
    fn unexpected_argument_is_an_error() {
        assert!(parse(&["--nope"], "--api-key").is_err());
        assert!(parse(&["rnd_abc"], "--api-key").is_err());
    }
}

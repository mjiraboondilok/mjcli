pub(crate) fn nonempty_trimmed(s: &str) -> Option<String> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn parse_value_flags<const N: usize>(
    args: &[String],
    flags: &[&str; N],
) -> Result<[Option<String>; N], String> {
    let mut values: [Option<String>; N] = std::array::from_fn(|_| None);
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        let Some((idx, flag, inline_value)) = flags.iter().enumerate().find_map(|(i, flag)| {
            if arg == flag {
                Some((i, *flag, None))
            } else {
                arg.strip_prefix(flag)
                    .and_then(|r| r.strip_prefix('='))
                    .map(|v| (i, *flag, Some(v)))
            }
        }) else {
            return Err(format!("error: unexpected argument '{arg}'"));
        };

        let raw = match inline_value {
            Some(v) => v,
            None => rest
                .next()
                .ok_or_else(|| format!("error: {flag} requires a value"))?
                .as_str(),
        };

        if values[idx].is_some() {
            return Err(format!("error: {flag} given more than once"));
        }
        values[idx] =
            Some(nonempty_trimmed(raw).ok_or_else(|| format!("error: {flag} value is empty"))?);
    }

    Ok(values)
}

pub(crate) fn parse_single_value_flag_or_usage(
    args: &[String],
    flag: &str,
    usage: &str,
) -> Result<Option<String>, std::process::ExitCode> {
    let [value] = parse_value_flags_or_usage(args, &[flag], usage)?;
    Ok(value)
}

pub(crate) fn parse_value_flags_or_usage<const N: usize>(
    args: &[String],
    flags: &[&str; N],
    usage: &str,
) -> Result<[Option<String>; N], std::process::ExitCode> {
    parse_value_flags(args, flags).map_err(|e| {
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
        let [value] = parse_value_flags(&owned, &[flag])?;
        Ok(value)
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

    fn parse_multi<const N: usize>(
        args: &[&str],
        flags: &[&str; N],
    ) -> Result<[Option<String>; N], String> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse_value_flags(&owned, flags)
    }

    #[test]
    fn multi_flag_parses_each_flag_independent_of_order() {
        assert_eq!(
            parse_multi(
                &["--connection", "postgres://x", "--email", "a@example.com"],
                &["--email", "--connection"]
            ),
            Ok([Some("a@example.com".into()), Some("postgres://x".into())])
        );
    }

    #[test]
    fn multi_flag_allows_some_flags_to_be_absent() {
        assert_eq!(
            parse_multi(&["--email=a@example.com"], &["--email", "--connection"]),
            Ok([Some("a@example.com".into()), None])
        );
    }

    #[test]
    fn multi_flag_rejects_argument_matching_no_known_flag() {
        assert!(parse_multi(&["--nope", "x"], &["--email", "--connection"]).is_err());
    }
}

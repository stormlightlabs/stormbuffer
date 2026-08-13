use crate::Record;

pub(crate) fn contains_likely_secret(record: &Record) -> bool {
    text_contains_likely_secret(&record.title)
        || text_contains_likely_secret(&record.body)
        || record
            .tags
            .iter()
            .any(|value| text_contains_likely_secret(value))
        || record
            .aliases
            .iter()
            .any(|value| text_contains_likely_secret(value))
        || record.sources.iter().any(|source| {
            text_contains_likely_secret(&source.reference)
                || text_contains_likely_secret(&source.actor)
        })
}

fn text_contains_likely_secret(text: &str) -> bool {
    contains_private_key(text)
        || contains_authorization_header(text)
        || contains_prefixed_token(text)
        || contains_credential_url(text)
}

fn contains_private_key(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("-----BEGIN ") && line.ends_with(" PRIVATE KEY-----")
    })
}

fn contains_authorization_header(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        if !name.eq_ignore_ascii_case("authorization")
            && !name.eq_ignore_ascii_case("proxy-authorization")
        {
            return false;
        }
        let value = value.trim();
        let credential = strip_prefix_ignore_ascii_case(value, "bearer ")
            .or_else(|| strip_prefix_ignore_ascii_case(value, "basic "))
            .unwrap_or(value);
        is_substantive_credential(credential)
    }) || text.split_whitespace().any(|word| {
        strip_prefix_ignore_ascii_case(word, "bearer=").is_some_and(is_substantive_credential)
    })
}

fn contains_prefixed_token(text: &str) -> bool {
    text.split(|character: char| {
        character.is_whitespace()
            || matches!(character, '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '=')
    })
    .map(|token| token.trim_matches(|character: char| matches!(character, '[' | ']' | '{' | '}')))
    .any(|token| {
        let token = token.trim_end_matches(['.', ':', '!', '?']);
        if is_placeholder(token) {
            return false;
        }
        [
            ("github_pat_", 24),
            ("ghp_", 20),
            ("gho_", 20),
            ("ghu_", 20),
            ("ghs_", 20),
            ("glpat-", 20),
            ("sk_live_", 20),
            ("rk_live_", 20),
            ("sk-", 24),
            ("xoxb-", 24),
            ("xoxp-", 24),
            ("xoxa-", 24),
            ("AIza", 30),
        ]
        .iter()
        .any(|(prefix, minimum)| {
            token.starts_with(prefix)
                && token.len() >= *minimum
                && token[prefix.len()..].chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                })
        }) || (token.len() == 20
            && token.starts_with("AKIA")
            && token
                .chars()
                .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit()))
    })
}

fn contains_credential_url(text: &str) -> bool {
    let mut rest = text;
    while let Some(scheme_end) = rest.find("://") {
        let after_scheme = &rest[scheme_end + 3..];
        let authority_end = after_scheme
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '/' | '?' | '#')
            })
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if let Some((userinfo, _host)) = authority.rsplit_once('@')
            && let Some((_username, password)) = userinfo.split_once(':')
            && is_substantive_credential(password)
        {
            return true;
        }
        rest = &after_scheme[authority_end..];
    }
    false
}

fn strip_prefix_ignore_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
        .then(|| &value[prefix.len()..])
}

fn is_substantive_credential(value: &str) -> bool {
    let value = value.trim_matches(|character: char| {
        character.is_ascii_whitespace() || matches!(character, '"' | '\'' | '`')
    });
    value.len() >= 8 && !is_placeholder(value)
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "password" | "passwd" | "token" | "secret" | "api_key" | "apikey"
    ) || normalized.starts_with('<')
        || normalized.starts_with("${")
        || normalized.starts_with("{{")
        || normalized
            .chars()
            .all(|character| character == '*' || character == 'x')
        || [
            "example",
            "placeholder",
            "redacted",
            "your_token",
            "your-token",
            "your_password",
            "your-password",
        ]
        .iter()
        .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::text_contains_likely_secret;

    #[test]
    fn detects_high_confidence_secret_shapes() {
        for value in [
            "-----BEGIN RSA PRIVATE KEY-----\nmaterial",
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.signature",
            "proxy-authorization: Basic dXNlcjphLXJlYWwtcGFzc3dvcmQ=",
            "token ghp_0123456789abcdefghijklmnop",
            "token gho_0123456789abcdefghijklmnop",
            "token ghu_0123456789abcdefghijklmnop",
            "token ghs_0123456789abcdefghijklmnop",
            "token github_pat_0123456789abcdefghijklmnop",
            "token glpat-0123456789abcdefghijklmnop",
            "token sk_live_0123456789abcdefghijklmnop",
            "token rk_live_0123456789abcdefghijklmnop",
            "token sk-proj-0123456789abcdefghijklmnop",
            "token xoxb-0123456789abcdefghijklmnop",
            "token xoxp-0123456789abcdefghijklmnop",
            "token xoxa-0123456789abcdefghijklmnop",
            "token AIza0123456789abcdefghijklmnopqrstuv",
            "AWS_ACCESS_KEY_ID=AKIA0123456789ABCDEF",
            "postgres://service:correct-horse-battery@db.internal/app",
        ] {
            assert!(text_contains_likely_secret(value), "missed secret shape");
        }
    }

    #[test]
    fn permits_ordinary_identifiers_and_placeholders() {
        for value in [
            "commit 0123456789abcdef0123456789abcdef01234567",
            "record 01989af2-4305-7b19-88b1-e8ae4ea9a099",
            "use Authorization: Bearer ${YOUR_TOKEN}",
            "set token to github_pat_example_placeholder",
            "open https://user:<password>@example.com/docs",
            "open https://user:password@example.com/docs",
            "the sk- prefix is discussed without a credential",
        ] {
            assert!(!text_contains_likely_secret(value), "rejected safe example");
        }
    }
}

use once_cell::sync::Lazy;
use regex::Regex;
use ulid::Ulid;

/// `{{TYPE_ULID}}` where TYPE is [A-Z][A-Z0-9]{0,31} and ULID is 26 Crockford chars.
pub static PLACEHOLDER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{\{([A-Z][A-Z0-9]{0,31})_([0-9A-HJKMNP-TV-Z]{26})\}\}").unwrap());

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    pub type_prefix: String,
    pub id: Ulid,
}

impl Placeholder {
    pub fn new(type_prefix: impl Into<String>, id: Ulid) -> Self {
        Self {
            type_prefix: type_prefix.into(),
            id,
        }
    }

    pub fn fresh(type_prefix: impl Into<String>) -> Self {
        Self::new(type_prefix, Ulid::new())
    }

    pub fn format(&self) -> String {
        "{{".to_string() + &self.type_prefix + "_" + &self.id.to_string() + "}}"
    }
}

/// Parse a complete placeholder token. The entire string must match.
pub fn parse_placeholder(token: &str) -> Option<Placeholder> {
    let caps = PLACEHOLDER_REGEX.captures(token)?;
    if caps.get(0)?.as_str() != token {
        return None;
    }
    let type_prefix = caps.get(1)?.as_str().to_string();
    let id = Ulid::from_string(caps.get(2)?.as_str()).ok()?;
    Some(Placeholder { type_prefix, id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_roundtrip() {
        let p = Placeholder::new("PHONE", "01JQC3K7N8R2T4V6W8X0Y2Z4AA".parse().unwrap());
        let s = p.format();
        assert_eq!(s, "{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4AA}}");
        let back = parse_placeholder(&s).unwrap();
        assert_eq!(back.type_prefix, "PHONE");
        assert_eq!(back.format(), s);
    }

    #[test]
    fn rejects_docs_examples_and_mustache() {
        assert!(parse_placeholder("{{PHONE_xxxx}}").is_none());
        assert!(parse_placeholder("{{TODO_1}}").is_none());
        assert!(parse_placeholder("{{phone_01JQC3K7N8R2T4V6W8X0Y2Z4AA}}").is_none());
    }

    #[test]
    fn regex_does_not_match_inside_longer_braces_partial() {
        let text = "see {{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4AA}} please";
        let caps: Vec<_> = PLACEHOLDER_REGEX
            .find_iter(text)
            .map(|m| m.as_str())
            .collect();
        assert_eq!(caps, vec!["{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4AA}}"]);
    }
}

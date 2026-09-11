use std::net::{IpAddr, Ipv4Addr};

/// True for characters that always break IP candidate tokens.
///
/// Letters are never delimiters, so `v1.2.3.4` stays one non-IP token.
fn is_token_delim(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '/' | ','
                | ';'
                | '|'
                | '('
                | ')'
                | '['
                | ']'
                | '='
                | '@'
                | '"'
                | '\''
                | '<'
                | '>'
                | '{'
                | '}'
        )
}

fn is_trailing_punct(c: char) -> bool {
    matches!(c, '.' | ',' | ':' | ';' | '!' | '?' | '。' | '，' | '：')
}

fn trim_trailing_punct(text: &str, start: usize, mut end: usize) -> usize {
    while end > start {
        let Some(c) = text[..end].chars().next_back() else {
            break;
        };
        if !is_trailing_punct(c) {
            break;
        }
        end -= c.len_utf8();
    }
    end
}

/// Find byte spans of parseable IPv4/IPv6 addresses in `text`.
///
/// Tokenizes on whitespace and common delimiters (`/ , ; | []() = @ "' <> {}`),
/// then parses each token with [`IpAddr`]. Colon is kept inside tokens so IPv6
/// still parses; if the whole token is not an address, trailing sentence
/// punctuation is stripped and retried, then colon-separated pieces are tried
/// as IPv4 (covering `host:port`).
pub fn find_ip_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut i = 0usize;
    let len = text.len();

    while i < len {
        let rest = &text[i..];
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if is_token_delim(ch) {
            i += ch.len_utf8();
            continue;
        }
        let start = i;
        i += ch.len_utf8();
        while i < len {
            let Some(c) = text[i..].chars().next() else {
                break;
            };
            if is_token_delim(c) {
                break;
            }
            i += c.len_utf8();
        }
        consider_token(text, start, i, &mut spans);
    }

    spans
}

fn consider_token(text: &str, start: usize, end: usize, spans: &mut Vec<(usize, usize)>) {
    if start >= end {
        return;
    }
    if try_parse_ip(text, start, end, spans) {
        return;
    }

    let trimmed = trim_trailing_punct(text, start, end);
    if trimmed != end {
        if try_parse_ip(text, start, trimmed, spans) {
            return;
        }
        consider_colon_split(text, start, trimmed, spans);
        return;
    }

    consider_colon_split(text, start, end, spans);
}

fn try_parse_ip(text: &str, start: usize, end: usize, spans: &mut Vec<(usize, usize)>) -> bool {
    if start >= end {
        return false;
    }
    if text[start..end].parse::<IpAddr>().is_ok() {
        spans.push((start, end));
        true
    } else {
        false
    }
}

fn consider_colon_split(text: &str, start: usize, end: usize, spans: &mut Vec<(usize, usize)>) {
    if start >= end {
        return;
    }
    // IPv4 among colon-delimited pieces (e.g. `127.0.0.1:18787`).
    let token = &text[start..end];
    let mut sub_start = start;
    for part in token.split(':') {
        let sub_end = sub_start + part.len();
        if part.parse::<Ipv4Addr>().is_ok() {
            spans.push((sub_start, sub_end));
        }
        sub_start = sub_end + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<&str> {
        find_ip_spans(text)
            .into_iter()
            .map(|(s, e)| &text[s..e])
            .collect()
    }

    #[test]
    fn delimited_and_trimmed_forms() {
        assert_eq!(texts("src_ip=10.0.0.5"), vec!["10.0.0.5"]);
        assert_eq!(texts("Reach 10.0.0.5. Then stop."), vec!["10.0.0.5"]);
        assert_eq!(texts("host '8.8.8.8'"), vec!["8.8.8.8"]);
        assert_eq!(texts(r#""10.1.2.3""#), vec!["10.1.2.3"]);
        assert_eq!(texts("admin@10.0.0.5"), vec!["10.0.0.5"]);
        assert_eq!(texts(r#"{"ip":"8.8.8.8"}"#), vec!["8.8.8.8"]);
        assert_eq!(texts("见 10.0.0.5。"), vec!["10.0.0.5"]);
    }

    #[test]
    fn letters_keep_version_from_looking_like_ip() {
        assert!(texts("v1.2.3.4").is_empty());
        assert!(texts("commit deadbeef").is_empty());
    }
}

use std::net::{IpAddr, Ipv4Addr};

/// True for characters that always break IP candidate tokens.
fn is_token_delim(c: char) -> bool {
    c.is_whitespace() || matches!(c, '/' | ',' | ';' | '|' | '(' | ')' | '[' | ']')
}

/// Find byte spans of parseable IPv4/IPv6 addresses in `text`.
///
/// Tokenizes on whitespace and common delimiters (`/ , ; | []()`), then
/// parses each token with [`IpAddr`]. Colon is kept inside tokens so IPv6
/// still parses; if the whole token is not an address, colon-separated
/// pieces are tried as IPv4 (covering `host:port`).
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
    let token = &text[start..end];
    if token.parse::<IpAddr>().is_ok() {
        spans.push((start, end));
        return;
    }

    // IPv4 among colon-delimited pieces (e.g. `127.0.0.1:18787`).
    let mut sub_start = start;
    for part in token.split(':') {
        let sub_end = sub_start + part.len();
        if part.parse::<Ipv4Addr>().is_ok() {
            spans.push((sub_start, sub_end));
        }
        sub_start = sub_end + 1;
    }
}

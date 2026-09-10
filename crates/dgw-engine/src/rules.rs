use std::collections::HashSet;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use regex::Regex;

const DEFAULT_TIMEOUT_MS: u64 = 50;

/// A user-manageable matcher that detects one class of sensitive value.
#[derive(Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub type_prefix: String,
    pub enabled: bool,
    pub matcher: Matcher,
    pub priority: i32,
    pub timeout_ms: u64,
}

/// How a rule locates candidate spans in plaintext.
#[derive(Clone, Debug)]
pub enum Matcher {
    Regex(Regex),
    Dictionary(Vec<String>),
    #[cfg(test)]
    BlockUntilTimeout,
}

/// Exact strings that must never be replaced.
#[derive(Clone, Debug, Default)]
pub struct Allowlist {
    entries: HashSet<String>,
}

/// A match on the original text (pre-replace).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub type_prefix: String,
    pub plaintext: String,
}

/// Ordered, non-overlapping rule evaluation over a string.
#[derive(Clone, Debug)]
pub struct RuleSet {
    rules: Vec<Rule>,
    allowlist: Allowlist,
}

impl Rule {
    pub fn regex(
        id: impl Into<String>,
        type_prefix: impl Into<String>,
        pattern: &str,
        priority: i32,
    ) -> Self {
        let id = id.into();
        let re = Regex::new(pattern).unwrap_or_else(|err| {
            panic!("invalid regex for rule {id}: {err}");
        });
        let name = id.clone();
        Self {
            id,
            name,
            type_prefix: type_prefix.into(),
            enabled: true,
            matcher: Matcher::Regex(re),
            priority,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    pub fn dictionary(
        id: impl Into<String>,
        type_prefix: impl Into<String>,
        words: Vec<String>,
        priority: i32,
    ) -> Self {
        let id = id.into();
        let name = id.clone();
        Self {
            id,
            name,
            type_prefix: type_prefix.into(),
            enabled: true,
            matcher: Matcher::Dictionary(words),
            priority,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

impl Allowlist {
    pub fn contains(&self, s: &str) -> bool {
        self.entries.contains(s)
    }
}

impl<S, I> From<I> for Allowlist
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    fn from(iter: I) -> Self {
        Self {
            entries: iter.into_iter().map(Into::into).collect(),
        }
    }
}

impl RuleSet {
    pub fn new(mut rules: Vec<Rule>, allowlist: Allowlist) -> Self {
        rules.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Self { rules, allowlist }
    }

    pub fn find_hits(&self, text: &str) -> Vec<Hit> {
        let mut occupied = Occupied::default();
        let mut hits = Vec::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            let Some(spans) = match_rule(rule, text, &occupied) else {
                continue;
            };
            commit_spans(
                text,
                &rule.type_prefix,
                spans,
                &self.allowlist,
                &mut occupied,
                &mut hits,
            );
        }

        hits.sort_by_key(|hit| hit.start);
        hits
    }
}

#[derive(Default)]
struct Occupied {
    spans: Vec<(usize, usize)>,
}

impl Occupied {
    fn gaps(&self, len: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut cursor = 0usize;
        for &(start, end) in &self.spans {
            if cursor < start {
                gaps.push((cursor, start));
            }
            cursor = cursor.max(end);
        }
        if cursor < len {
            gaps.push((cursor, len));
        }
        gaps
    }

    fn overlaps(&self, start: usize, end: usize) -> bool {
        self.spans.iter().any(|&(s, e)| start < e && end > s)
    }

    fn insert(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        self.spans.push((start, end));
        self.spans.sort_unstable_by_key(|&(s, _)| s);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in self.spans.drain(..) {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            merged.push((s, e));
        }
        self.spans = merged;
    }
}

fn match_rule(rule: &Rule, text: &str, occupied: &Occupied) -> Option<Vec<(usize, usize)>> {
    match &rule.matcher {
        Matcher::Regex(re) => {
            let timeout = Duration::from_millis(rule.timeout_ms);
            let re = re.clone();
            let haystack = text.to_owned();
            let gaps = occupied.gaps(haystack.len());
            run_with_timeout(timeout, move || regex_spans(&re, &haystack, &gaps))
        }
        Matcher::Dictionary(words) => Some(dictionary_spans(text, words, occupied)),
        #[cfg(test)]
        Matcher::BlockUntilTimeout => {
            let timeout = Duration::from_millis(rule.timeout_ms.max(1));
            let blocked = timeout + Duration::from_millis(50);
            run_with_timeout(timeout, move || {
                thread::sleep(blocked);
                Vec::new()
            })
        }
    }
}

fn regex_spans(re: &Regex, text: &str, gaps: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for &(gs, ge) in gaps {
        if gs >= ge || ge > text.len() {
            continue;
        }
        for m in re.find_iter(&text[gs..ge]) {
            if m.start() == m.end() {
                continue;
            }
            spans.push((gs + m.start(), gs + m.end()));
        }
    }
    spans
}

fn dictionary_spans(text: &str, words: &[String], occupied: &Occupied) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for (gs, ge) in occupied.gaps(text.len()) {
        if gs >= ge {
            continue;
        }
        let slice = &text[gs..ge];
        for word in words {
            if word.is_empty() {
                continue;
            }
            for (offset, _) in slice.match_indices(word) {
                let start = gs + offset;
                spans.push((start, start + word.len()));
            }
        }
    }
    spans.sort_by_key(|&(s, e)| (s, std::cmp::Reverse(e)));
    spans
}

fn commit_spans(
    text: &str,
    type_prefix: &str,
    spans: Vec<(usize, usize)>,
    allowlist: &Allowlist,
    occupied: &mut Occupied,
    hits: &mut Vec<Hit>,
) {
    let mut taken: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if start >= end || end > text.len() {
            continue;
        }
        if occupied.overlaps(start, end) || taken.iter().any(|&(s, e)| start < e && end > s) {
            continue;
        }
        let plaintext = match text.get(start..end) {
            Some(p) => p,
            None => continue,
        };
        if allowlist.contains(plaintext) {
            continue;
        }
        taken.push((start, end));
        hits.push(Hit {
            start,
            end,
            type_prefix: type_prefix.to_string(),
            plaintext: plaintext.to_string(),
        });
    }
    for (start, end) in taken {
        occupied.insert(start, end);
    }
}

/// Run `work` on a worker thread and treat `recv_timeout` expiry as a miss.
fn run_with_timeout<T, F>(timeout: Duration, work: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = tx.send(work());
    });
    match rx.recv_timeout(timeout) {
        Ok(value) => {
            let _ = handle.join();
            Some(value)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruleset(rules: Vec<Rule>) -> RuleSet {
        RuleSet::new(
            rules,
            Allowlist::from(["127.0.0.1", "localhost", "0.0.0.0", "::1"]),
        )
    }

    #[test]
    fn higher_priority_wins_and_consumes_span() {
        let conn = Rule::regex("conn", "CONNSTR", r"postgres://\S+", 100);
        let ip = Rule::regex("ip", "IP", r"\d+\.\d+\.\d+\.\d+", 10);
        let set = ruleset(vec![conn, ip]);
        let hits = set.find_hits("postgres://u:p@10.0.0.5/db");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_prefix, "CONNSTR");
        assert_eq!(hits[0].plaintext, "postgres://u:p@10.0.0.5/db");
    }

    #[test]
    fn allowlist_skips_exact_string() {
        let ip = Rule::regex("ip", "IP", r"127\.0\.0\.1", 10);
        let set = ruleset(vec![ip]);
        assert!(set.find_hits("talk to 127.0.0.1 please").is_empty());
    }

    #[test]
    fn disabled_rules_do_not_match() {
        let mut r = Rule::regex("phone", "PHONE", r"1[3-9]\d{9}", 50);
        r.enabled = false;
        let set = ruleset(vec![r]);
        assert!(set.find_hits("call 13800138000").is_empty());
    }

    #[test]
    fn dictionary_matcher_is_literal() {
        let r = Rule::dictionary("words", "SECRET", vec!["AKIAEXAMPLE".into()], 80);
        let set = ruleset(vec![r]);
        assert_eq!(
            set.find_hits("AKIAEXAMPLE in text")[0].plaintext,
            "AKIAEXAMPLE"
        );
    }

    #[test]
    fn regex_timeout_counts_as_miss_for_that_rule_only() {
        let mut evil = Rule::regex("evil", "X", r"a+", 1).with_timeout_ms(5);
        evil.matcher = Matcher::BlockUntilTimeout;
        let phone = Rule::regex("phone", "PHONE", r"1[3-9]\d{9}", 50);
        let set = ruleset(vec![evil, phone]);
        let hits = set.find_hits(&format!("{}13800138000", "a".repeat(20)));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_prefix, "PHONE");
    }
}

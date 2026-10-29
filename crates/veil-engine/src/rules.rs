use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use fancy_regex::{Regex, RegexBuilder};

const DEFAULT_TIMEOUT_MS: u64 = 50;
const REGEX_BACKTRACK_LIMIT: usize = 1_000_000;
static NEXT_RULESET_REVISION: AtomicU64 = AtomicU64::new(1);

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
    /// Original regex source, when `matcher` is regex.
    pub pattern: Option<String>,
}

/// How a rule locates candidate spans in plaintext.
#[derive(Clone, Debug)]
pub enum Matcher {
    Regex(Regex),
    Dictionary(Vec<String>),
    /// Tokenize and parse with [`std::net::IpAddr`].
    Ip,
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
    cache_revision: u64,
}

impl Rule {
    pub fn regex(
        id: impl Into<String>,
        type_prefix: impl Into<String>,
        pattern: &str,
        priority: i32,
    ) -> Self {
        let id = id.into();
        let re = RegexBuilder::new(pattern)
            .backtrack_limit(REGEX_BACKTRACK_LIMIT)
            .build()
            .unwrap_or_else(|err| {
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
            pattern: Some(pattern.to_string()),
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
            pattern: None,
        }
    }

    pub fn words(&self) -> Option<&[String]> {
        match &self.matcher {
            Matcher::Dictionary(w) => Some(w),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self.matcher {
            Matcher::Regex(_) => "regex",
            Matcher::Dictionary(_) => "dictionary",
            Matcher::Ip => "ip",
            #[cfg(test)]
            Matcher::BlockUntilTimeout => "timeout",
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Parse-based IPv4/IPv6 matcher.
    pub fn ip(id: impl Into<String>, type_prefix: impl Into<String>, priority: i32) -> Self {
        let id = id.into();
        let name = id.clone();
        Self {
            id,
            name,
            type_prefix: type_prefix.into(),
            enabled: true,
            matcher: Matcher::Ip,
            priority,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pattern: None,
        }
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
    /// Identity of the immutable compiled policy. Clones share the identity;
    /// rebuilding also covers regex builder options not visible in source text.
    pub fn cache_revision(&self) -> u64 {
        self.cache_revision
    }

    /// Fingerprint configured rules and allowlists; pair with cache_revision for
    /// compiled matcher options that are not represented in pattern strings.
    pub fn cache_fingerprint(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        fn field(hash: &mut Sha256, value: &[u8]) {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value);
        }
        let mut hash = Sha256::new();
        hash.update((self.rules.len() as u64).to_le_bytes());
        for rule in &self.rules {
            field(&mut hash, rule.id.as_bytes());
            field(&mut hash, rule.type_prefix.as_bytes());
            hash.update([u8::from(rule.enabled)]);
            hash.update(rule.priority.to_le_bytes());
            hash.update(rule.timeout_ms.to_le_bytes());
            match &rule.matcher {
                Matcher::Regex(re) => {
                    hash.update([0]);
                    field(&mut hash, re.as_str().as_bytes());
                }
                Matcher::Dictionary(words) => {
                    hash.update([1]);
                    hash.update((words.len() as u64).to_le_bytes());
                    for word in words {
                        field(&mut hash, word.as_bytes());
                    }
                }
                Matcher::Ip => hash.update([2]),
                #[cfg(test)]
                Matcher::BlockUntilTimeout => hash.update([3]),
            }
        }
        let mut allowlist: Vec<_> = self.allowlist.entries.iter().collect();
        allowlist.sort();
        hash.update((allowlist.len() as u64).to_le_bytes());
        for entry in allowlist {
            field(&mut hash, entry.as_bytes());
        }
        hash.finalize().into()
    }

    pub fn new(mut rules: Vec<Rule>, allowlist: Allowlist) -> Self {
        rules.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Self {
            rules,
            allowlist,
            cache_revision: NEXT_RULESET_REVISION.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
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
        let index = self.spans.partition_point(|&(_, e)| e <= start);
        self.spans.get(index).is_some_and(|&(s, _)| s < end)
    }

    /// Merge one rule's accepted spans, which are sorted and non-overlapping.
    fn extend_sorted(&mut self, spans: Vec<(usize, usize)>) {
        if spans.is_empty() {
            return;
        }
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(self.spans.len() + spans.len());
        let mut existing = std::mem::take(&mut self.spans).into_iter().peekable();
        let mut incoming = spans.into_iter().peekable();
        while existing.peek().is_some() || incoming.peek().is_some() {
            let take_existing = match (existing.peek(), incoming.peek()) {
                (Some(&(s, _)), Some(&(next, _))) => s <= next,
                (Some(_), None) => true,
                _ => false,
            };
            let (s, e) = if take_existing {
                existing.next().unwrap()
            } else {
                incoming.next().unwrap()
            };
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
            let gaps = occupied.gaps(text.len());
            regex_spans(re, text, &gaps)
        }
        Matcher::Dictionary(words) => Some(dictionary_spans(text, words, occupied)),
        Matcher::Ip => Some(ip_spans(text, occupied)),
        #[cfg(test)]
        Matcher::BlockUntilTimeout => {
            // Deterministic miss without spawning a worker thread.
            let _ = rule.timeout_ms;
            None
        }
    }
}

fn regex_spans(re: &Regex, text: &str, gaps: &[(usize, usize)]) -> Option<Vec<(usize, usize)>> {
    let mut spans = Vec::new();
    for &(gs, ge) in gaps {
        if gs >= ge || ge > text.len() {
            continue;
        }
        for result in re.find_iter(&text[gs..ge]) {
            let m = match result {
                Ok(m) => m,
                Err(_) => return None,
            };
            if m.start() == m.end() {
                continue;
            }
            spans.push((gs + m.start(), gs + m.end()));
        }
    }
    Some(spans)
}

fn ip_spans(text: &str, occupied: &Occupied) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for (gs, ge) in occupied.gaps(text.len()) {
        if gs >= ge {
            continue;
        }
        for (s, e) in crate::ip::find_ip_spans(&text[gs..ge]) {
            spans.push((gs + s, gs + e));
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
    // All matchers return candidates in ascending start order. Dictionary
    // candidates at the same start retain their longest-first ordering.
    debug_assert!(spans.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    let mut taken: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if start >= end || end > text.len() {
            continue;
        }
        if occupied.overlaps(start, end) || taken.last().is_some_and(|&(_, e)| start < e) {
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
    occupied.extend_sorted(taken);
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
    fn cache_revision_tracks_rebuilt_compiled_matchers_and_survives_cloning() {
        let sensitive = Rule::regex("secret", "SECRET", "secret", 1);
        let mut insensitive = sensitive.clone();
        insensitive.matcher = Matcher::Regex(
            RegexBuilder::new("secret")
                .case_insensitive(true)
                .build()
                .unwrap(),
        );
        let first = ruleset(vec![sensitive]);
        let second = ruleset(vec![insensitive]);
        assert_eq!(first.cache_fingerprint(), second.cache_fingerprint());
        assert_ne!(first.cache_revision(), second.cache_revision());
        assert_eq!(first.cache_revision(), first.clone().cache_revision());
        assert!(first.find_hits("SECRET").is_empty());
        assert_eq!(second.find_hits("SECRET").len(), 1);
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
    fn dictionary_overlap_allowlist_and_adjacency_preserve_candidate_order() {
        let rule = Rule::dictionary(
            "words",
            "WORD",
            ["ab", "abc", "bc", "c", "白", "白🙂", "🙂", "ab"]
                .into_iter()
                .map(String::from)
                .collect(),
            1,
        );
        let set = RuleSet::new(vec![rule], Allowlist::from(["abc", "白🙂"]));
        let text = "abc白🙂abc";
        let hits = set.find_hits(text);
        let plaintexts: Vec<_> = hits.iter().map(|h| h.plaintext.as_str()).collect();
        // Allowlisted long candidates must not block shorter candidates; duplicates
        // and nested matches must be rejected, while adjacent matches survive.
        assert_eq!(plaintexts, ["ab", "c", "白", "🙂", "ab", "c"]);
        for hit in hits {
            assert_eq!(&text[hit.start..hit.end], hit.plaintext);
        }
    }

    #[test]
    fn merged_intervals_preserve_gap_boundary_matching() {
        let set = ruleset(vec![
            Rule::regex("prefix", "PREFIX", r"^9", 2),
            Rule::regex("phone", "PHONE", r"(?<!\d)1[3-9]\d{9}(?!\d)", 1),
        ]);
        let hits = set.find_hits("913800138000");
        assert_eq!(hits.len(), 2);
        assert_eq!((hits[0].start, hits[0].end), (0, 1));
        assert_eq!(hits[0].type_prefix, "PREFIX");
        assert_eq!((hits[1].start, hits[1].end), (1, 12));
        assert_eq!(hits[1].type_prefix, "PHONE");
    }

    #[test]
    fn batch_commit_matches_legacy_filter_and_merge() {
        // Keep an independent reference of the old candidate filtering and
        // per-span merging to check observable results, including invalid UTF-8
        // ranges and allowlisted candidates that must not reserve an interval.
        fn legacy(
            text: &str,
            spans: &[(usize, usize)],
            allowlist: &Allowlist,
            occupied: &mut Vec<(usize, usize)>,
        ) -> Vec<Hit> {
            let mut taken = Vec::new();
            let mut hits = Vec::new();
            for &(start, end) in spans {
                if start >= end
                    || end > text.len()
                    || occupied.iter().any(|&(s, e)| start < e && end > s)
                    || taken.iter().any(|&(s, e)| start < e && end > s)
                {
                    continue;
                }
                let Some(plaintext) = text.get(start..end) else {
                    continue;
                };
                if allowlist.contains(plaintext) {
                    continue;
                }
                taken.push((start, end));
                hits.push(Hit {
                    start,
                    end,
                    type_prefix: "TEST".into(),
                    plaintext: plaintext.into(),
                });
            }
            for span in taken {
                occupied.push(span);
                occupied.sort_unstable_by_key(|&(s, _)| s);
                let mut merged: Vec<(usize, usize)> = Vec::new();
                for &(s, e) in occupied.iter() {
                    if let Some(last) = merged.last_mut() {
                        if s <= last.1 {
                            last.1 = last.1.max(e);
                            continue;
                        }
                    }
                    merged.push((s, e));
                }
                *occupied = merged;
            }
            hits
        }
        let text = "a中🙂bc白";
        let allowlist = Allowlist::from(["中", "bc"]);
        let mut seed = 47u64;
        for initial in [vec![], vec![(1, 4), (8, 9)], vec![(0, 1), (9, 13)]] {
            for case in 0..200 {
                let mut candidates = vec![(0, 0), (0, 1), (1, 4), (4, 8), (8, 10), (10, 13)];
                if case % 5 == 0 {
                    candidates.push((0, text.len()));
                }
                for _ in 0..20 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let start = (seed >> 32) as usize % (text.len() + 2);
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let end = (seed >> 32) as usize % (text.len() + 2);
                    candidates.push((start, end));
                }
                candidates.sort_by_key(|&(s, _)| s);
                let mut expected_occupied = initial.clone();
                let expected = legacy(text, &candidates, &allowlist, &mut expected_occupied);
                let mut occupied = Occupied {
                    spans: initial.clone(),
                };
                let mut hits = Vec::new();
                commit_spans(
                    text,
                    "TEST",
                    candidates,
                    &allowlist,
                    &mut occupied,
                    &mut hits,
                );
                assert_eq!(hits, expected);
                assert_eq!(occupied.spans, expected_occupied);
            }
        }
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

    #[test]
    fn regex_backtrack_limit_is_miss_for_that_rule_only() {
        let re = RegexBuilder::new(r"(?i)(a|b|ab)*(?>c)")
            .backtrack_limit(1)
            .seek(false)
            .build()
            .expect("pathological regex compiles");
        let hay = "ab".repeat(40);
        assert_eq!(regex_spans(&re, &hay, &[(0, hay.len())]), None);

        let evil = Rule {
            id: "evil".into(),
            name: "evil".into(),
            type_prefix: "X".into(),
            enabled: true,
            matcher: Matcher::Regex(re),
            priority: 1,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pattern: None,
        };
        let phone = Rule::regex("phone", "PHONE", r"1[3-9]\d{9}", 50);
        let set = ruleset(vec![evil, phone]);
        let hits = set.find_hits(&format!("{hay}13800138000"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_prefix, "PHONE");
    }
}

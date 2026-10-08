use veil_engine::builtin::{builtin_ruleset_with_packs, PackFlags};
use veil_engine::rules::{Allowlist, Rule, RuleSet};

use crate::config::{Config, RuleConfig};

/// Build the live rule set from pack flags and optional config overrides.
pub fn ruleset_from_config(cfg: &Config) -> RuleSet {
    let packs = PackFlags {
        secrets: cfg.packs.secrets,
        region: cfg.packs.region,
    };
    let base = builtin_ruleset_with_packs(packs);
    let allowlist = Allowlist::from(cfg.allowlist.clone());

    if cfg.rules.is_empty() {
        // Rebuild with config allowlist while keeping built-in rules.
        return RuleSet::new(base.rules().to_vec(), allowlist);
    }

    let mut rules = Vec::new();
    for row in &cfg.rules {
        if let Some(rule) = rule_from_config(row) {
            rules.push(rule);
        }
    }
    // If conversion yielded nothing, fall back to packs.
    if rules.is_empty() {
        return RuleSet::new(base.rules().to_vec(), allowlist);
    }
    RuleSet::new(rules, allowlist)
}

fn rule_from_config(row: &RuleConfig) -> Option<Rule> {
    let mut rule = if row.id == "ip" || row.type_prefix.eq_ignore_ascii_case("IP") {
        // Prefer parse-based IP matcher for the built-in ip row.
        if row.pattern.is_none() && row.words.as_ref().map(|w| w.is_empty()).unwrap_or(true) {
            Rule::ip(&row.id, &row.type_prefix, row.priority)
        } else if let Some(words) = &row.words {
            if !words.is_empty() {
                Rule::dictionary(&row.id, &row.type_prefix, words.clone(), row.priority)
            } else if let Some(pat) = &row.pattern {
                Rule::regex(&row.id, &row.type_prefix, pat, row.priority)
            } else {
                Rule::ip(&row.id, &row.type_prefix, row.priority)
            }
        } else if let Some(pat) = &row.pattern {
            Rule::regex(&row.id, &row.type_prefix, pat, row.priority)
        } else {
            Rule::ip(&row.id, &row.type_prefix, row.priority)
        }
    } else if let Some(words) = &row.words {
        if !words.is_empty() {
            Rule::dictionary(&row.id, &row.type_prefix, words.clone(), row.priority)
        } else if let Some(pat) = &row.pattern {
            Rule::regex(&row.id, &row.type_prefix, pat, row.priority)
        } else {
            return None;
        }
    } else if let Some(pat) = &row.pattern {
        Rule::regex(&row.id, &row.type_prefix, pat, row.priority)
    } else {
        return None;
    };
    rule.enabled = row.enabled;
    let _ = &row.source;
    Some(rule)
}

#[cfg(test)]
mod tests {
    use super::ruleset_from_config;
    use crate::config::Config;

    #[test]
    fn default_config_includes_region() {
        let cfg = Config::default();
        let set = ruleset_from_config(&cfg);
        let hits = set.find_hits("Asia/Shanghai and zh-CN");
        assert!(hits.iter().any(|h| h.type_prefix == "TZ"));
        assert!(hits.iter().any(|h| h.type_prefix == "LOCALE"));
    }
}

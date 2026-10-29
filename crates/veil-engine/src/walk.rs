use serde_json::Value;
use thiserror::Error;

use crate::binary::should_skip_desensitize;
use crate::creator::Creator;
use crate::mapping::{Lookup, MappingStore, StoreError};
use crate::placeholder::{parse_placeholder, PLACEHOLDER_REGEX};
use crate::rules::RuleSet;

#[derive(Debug, Error)]
pub enum WalkError {
    #[error("mapping write failed")]
    MappingWrite,
    #[error("store unavailable")]
    StoreUnavailable,
}
// 遍历里面所有字符串字段，按规则判断要不要脱敏
pub fn desensitize_json(
    value: &Value,
    rules: &RuleSet,
    store: &(impl MappingStore + ?Sized),
    creator: &Creator,
) -> Result<Value, WalkError> {
    map_string_values(None, value, &mut |key, text| {
        if should_skip_desensitize(key, text) {
            return Ok(text.to_string());
        }
        desensitize_string(text, rules, store, creator)
    })
}

pub fn restore_json(
    value: &Value,
    store: &(impl MappingStore + ?Sized),
    creator: &Creator,
) -> Result<Value, WalkError> {
    map_string_values(None, value, &mut |_key, text| {
        restore_string(text, store, creator)
    })
}

fn map_string_values(
    key: Option<&str>,
    value: &Value,
    f: &mut impl FnMut(Option<&str>, &str) -> Result<String, WalkError>,
) -> Result<Value, WalkError> {
    match value {
        Value::String(text) => Ok(Value::String(f(key, text)?)),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(map_string_values(None, item, f)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            let source_is_b64 = map
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|t| t.eq_ignore_ascii_case("base64"))
                || map
                    .get("media_type")
                    .and_then(|v| v.as_str())
                    .is_some_and(|m| {
                        let m = m.to_ascii_lowercase();
                        m.starts_with("image/")
                            || m.starts_with("audio/")
                            || m.starts_with("video/")
                    });
            for (child_key, child) in map {
                let effective = if source_is_b64 && child_key == "data" {
                    Some("data")
                } else {
                    Some(child_key.as_str())
                };
                out.insert(child_key.clone(), map_string_values(effective, child, f)?);
            }
            Ok(Value::Object(out))
        }
        other => Ok(other.clone()),
    }
}

fn desensitize_string(
    text: &str,
    rules: &RuleSet,
    store: &(impl MappingStore + ?Sized),
    creator: &Creator,
) -> Result<String, WalkError> {
    let hits = rules.find_hits(text);
    if hits.is_empty() {
        return Ok(text.to_string());
    }
    let mut replacements = Vec::with_capacity(hits.len());
    // Preserve mapping access order: the first insertion determines the type
    // reused for identical plaintext, even across different matching rules.
    for hit in hits.into_iter().rev() {
        let placeholder = store
            .get_or_insert(creator, &hit.type_prefix, &hit.plaintext)
            .map_err(|_| WalkError::MappingWrite)?;
        replacements.push((hit.start, hit.end, placeholder.format()));
    }
    Ok(append_replacements(text, &replacements))
}

fn restore_string(
    text: &str,
    store: &(impl MappingStore + ?Sized),
    creator: &Creator,
) -> Result<String, WalkError> {
    let matches: Vec<_> = PLACEHOLDER_REGEX.find_iter(text).collect();
    if matches.is_empty() {
        return Ok(text.to_string());
    }
    let mut replacements = Vec::with_capacity(matches.len());
    for m in matches.into_iter().rev() {
        let Some(placeholder) = parse_placeholder(m.as_str()) else {
            continue;
        };
        match store.lookup(creator, &placeholder) {
            Ok(Lookup::Hit(plaintext)) => {
                replacements.push((m.start(), m.end(), plaintext));
            }
            Ok(Lookup::Miss) => {}
            Err(StoreError::Unavailable) | Err(StoreError::Internal(_)) => {
                return Err(WalkError::StoreUnavailable);
            }
        }
    }
    Ok(append_replacements(text, &replacements))
}

/// Spans must be disjoint UTF-8 byte ranges in descending start order.
/// Resolve mappings first, then copy each unchanged segment only once.
fn append_replacements(text: &str, replacements: &[(usize, usize, String)]) -> String {
    let capacity = replacements
        .iter()
        .fold(text.len(), |len, (start, end, value)| {
            len - (end - start) + value.len()
        });
    let mut out = String::with_capacity(capacity);
    let mut cursor = 0;
    for (start, end, value) in replacements.iter().rev() {
        out.push_str(&text[cursor..*start]);
        out.push_str(value);
        cursor = *end;
    }
    out.push_str(&text[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::builtin_ruleset;
    use crate::creator::anonymous_creator;
    use crate::mapping::MemoryStore;
    use serde_json::json;

    #[test]
    fn replaces_only_string_values_not_keys() {
        let store = MemoryStore::new();
        let rules = builtin_ruleset();
        let c = anonymous_creator();
        let input = json!({"13800138000": "13800138000"});
        let out = desensitize_json(&input, &rules, &store, &c).unwrap();
        assert!(out.as_object().unwrap().contains_key("13800138000"));
        let v = out["13800138000"].as_str().unwrap();
        assert!(v.starts_with("{{PHONE_"));
        assert!(!v.contains("13800138000"));
    }

    #[test]
    fn nested_and_array_strings() {
        let store = MemoryStore::new();
        let out = desensitize_json(
            &json!({"messages":[{"content":"ip 8.8.8.8"}]}),
            &builtin_ruleset(),
            &store,
            &anonymous_creator(),
        )
        .unwrap();
        let s = out["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("{{IP_"));
        assert!(!s.contains("8.8.8.8"));
    }

    #[test]
    fn password_with_quotes_survives_roundtrip() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let secret = r#"postgres://u:p"a\b@10.1.2.3/db"#;
        let input = json!({"s": secret});
        let redacted = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
        let restored = restore_json(&redacted, &store, &c).unwrap();
        assert_eq!(restored["s"].as_str().unwrap(), secret);
    }

    #[test]
    fn does_not_rescan_restored_text() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let input = json!({"s": "13800138000"});
        let redacted = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
        let restored = restore_json(&redacted, &store, &c).unwrap();
        assert_eq!(restored["s"], "13800138000");
    }

    #[test]
    fn same_creator_reuses_placeholder() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let a = desensitize_json(&json!("13800138000"), &builtin_ruleset(), &store, &c).unwrap();
        let b = desensitize_json(&json!("13800138000"), &builtin_ruleset(), &store, &c).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn adjacent_unicode_hits_and_repeated_values_roundtrip() {
        use crate::rules::{Allowlist, Rule};

        let store = MemoryStore::new();
        let creator = anonymous_creator();
        let rules = RuleSet::new(
            vec![Rule::dictionary(
                "words",
                "WORD",
                vec!["甲乙".into(), "丙丁".into()],
                1,
            )],
            Allowlist::default(),
        );
        let input = json!({"text": "甲乙丙丁🙂甲乙\n尾部丙丁"});
        let masked = desensitize_json(&input, &rules, &store, &creator).unwrap();
        let first = store
            .get_or_insert(&creator, "WORD", "甲乙")
            .unwrap()
            .format();
        let second = store
            .get_or_insert(&creator, "WORD", "丙丁")
            .unwrap()
            .format();
        assert_eq!(
            masked["text"],
            format!("{first}{second}🙂{first}\n尾部{second}")
        );
        assert_eq!(restore_json(&masked, &store, &creator).unwrap(), input);
    }

    #[test]
    fn repeated_plaintext_across_types_preserves_reverse_mapping_order() {
        use crate::rules::{Allowlist, Rule};

        let store = MemoryStore::new();
        let creator = anonymous_creator();
        let rules = RuleSet::new(
            vec![
                Rule::regex("left", "LEFT", r"(?<=a)123", 2),
                Rule::regex("right", "RIGHT", r"(?<=b)123", 1),
            ],
            Allowlist::default(),
        );
        let input = json!({"text": "a123 b123"});
        let masked = desensitize_json(&input, &rules, &store, &creator).unwrap();
        // The rightmost occurrence must insert first, determining the type for both.
        let placeholder = store.get_or_insert(&creator, "RIGHT", "123").unwrap();
        assert_eq!(placeholder.type_prefix, "RIGHT");
        assert_eq!(
            masked["text"],
            format!("a{} b{}", placeholder.format(), placeholder.format())
        );
        assert_eq!(restore_json(&masked, &store, &creator).unwrap(), input);
    }

    #[test]
    fn restore_preserves_missing_foreign_and_nested_placeholders() {
        use crate::creator::Creator;

        let store = MemoryStore::new();
        let creator = anonymous_creator();
        let missing = "{{PHONE_01JQC3K7N8R2T4V6W8X0Y2Z4AA}}";
        let plaintext = format!("汉字🙂{missing}");
        let known = store
            .get_or_insert(&creator, "TOKEN", &plaintext)
            .unwrap()
            .format();
        let foreign = store
            .get_or_insert(&Creator::from_secret("other"), "PHONE", "13800138000")
            .unwrap()
            .format();
        let input = json!({"text": format!("{known}{missing}{foreign}{known}{{{{TODO_1}}}}尾部")});
        let restored = restore_json(&input, &store, &creator).unwrap();
        assert_eq!(
            restored["text"],
            format!("{plaintext}{missing}{foreign}{plaintext}{{{{TODO_1}}}}尾部")
        );
    }

    fn png_b64() -> String {
        // 1x1 PNG
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="
            .into()
    }

    #[test]
    fn anthropic_image_base64_survives_with_nearby_secrets() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let b64 = png_b64();
        // Contaminate a long blob with digit runs / sk- that rules would otherwise eat.
        let mut blob = b64.clone();
        while blob.len() < 220 {
            blob.push_str(&b64);
        }
        blob.push_str("13800138000");
        // Keep it pure base64 alphabet after stripping the phone for the "pure" case —
        // primary case: pure image data under source.data
        let input = json!({
            "model": "claude",
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "my phone 13800138000 ip 8.8.8.8"},
                    {"type": "image", "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": b64
                    }}
                ]
            }]
        });
        let out = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
        let text = out["messages"][0]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("{{PHONE_"), "{text}");
        assert!(text.contains("{{IP_"), "{text}");
        assert_eq!(
            out["messages"][0]["content"][1]["source"]["data"]
                .as_str()
                .unwrap(),
            b64
        );
    }

    #[test]
    fn openai_data_url_image_survives() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let url = format!("data:image/png;base64,{}", png_b64());
        let input = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "secret 13800138000"},
                    {"type": "image_url", "image_url": {"url": url}}
                ]
            }]
        });
        let out = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
        assert!(out["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("{{PHONE_"));
        let kept = out["messages"][0]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap();
        assert!(kept.starts_with("data:image/png;base64,"));
        assert!(kept.ends_with(&png_b64()));
        assert!(!kept.contains("{{"));
    }

    #[test]
    fn urlsafe_base64_blob_not_eaten_by_sk_rule() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        // URL-safe alphabet; embed sk- which APIKEY would match if scanned.
        let mut blob = "AAAA".to_string();
        blob.push_str("sk-ant-api03-secretvaluehereXXXXXXXX");
        while blob.len() < 220 {
            blob.push('A');
        }
        let input = json!({"data": blob});
        let out = desensitize_json(&input, &builtin_ruleset(), &store, &c).unwrap();
        assert_eq!(out["data"].as_str().unwrap(), blob);
    }
}

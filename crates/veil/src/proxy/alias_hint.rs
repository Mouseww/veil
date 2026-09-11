use serde_json::{json, Value};

pub const MARK: &str = "VEIL_ALIAS_HINT";

pub const HINT: &str = "VEIL_ALIAS_HINT: Tokens matching {{TYPE_26CHARS}} are aliases for real values the user can already see. Treat each alias as that real entity. When you repeat, quote, or use it, output the whole alias token unchanged. Never say you cannot see the original. Never invent digits or take a substring of the token.\n化名说明：形如 {{TYPE_26位}} 的记号是用户能看见的真值的化名。请把它当成真实内容来推理和作答。需要复述或使用时，原样输出整个化名，不要说看不见原文，不要从记号里拆字或编造后几位。";

pub fn inject_alias_hint(value: &mut Value) {
    if contains_mark(value) {
        return;
    }
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(sys) = obj.get_mut("system") {
        prepend_system(sys);
        return;
    }
    if let Some(ins) = obj.get_mut("instructions") {
        if let Value::String(s) = ins {
            *s = format!("{HINT}\n\n{s}");
            return;
        }
    }
    if let Some(Value::Array(msgs)) = obj.get_mut("messages") {
        if let Some(first) = msgs.first_mut() {
            if first.get("role").and_then(|r| r.as_str()) == Some("system") {
                prepend_message_content(first);
                return;
            }
        }
        msgs.insert(0, json!({"role": "system", "content": HINT}));
        return;
    }
    obj.insert("system".into(), Value::String(HINT.into()));
}

fn contains_mark(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains(MARK),
        Value::Array(a) => a.iter().any(contains_mark),
        Value::Object(o) => o.values().any(contains_mark),
        _ => false,
    }
}

fn prepend_system(sys: &mut Value) {
    match sys {
        Value::String(s) => *s = format!("{HINT}\n\n{s}"),
        Value::Array(blocks) => {
            blocks.insert(0, json!({"type": "text", "text": HINT}));
        }
        other => *other = Value::String(HINT.into()),
    }
}

fn prepend_message_content(msg: &mut Value) {
    let Some(obj) = msg.as_object_mut() else {
        return;
    };
    match obj.get_mut("content") {
        Some(Value::String(s)) => *s = format!("{HINT}\n\n{s}"),
        Some(Value::Array(blocks)) => {
            blocks.insert(0, json!({"type": "text", "text": HINT}));
        }
        _ => {
            obj.insert("content".into(), Value::String(HINT.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{inject_alias_hint, HINT, MARK};
    use serde_json::json;

    #[test]
    fn prepends_anthropic_system_string() {
        let mut v = json!({"model": "claude", "system": "be nice", "messages": []});
        inject_alias_hint(&mut v);
        assert!(v["system"].as_str().unwrap().contains(MARK));
        assert!(v["system"].as_str().unwrap().contains("be nice"));
        inject_alias_hint(&mut v);
        assert_eq!(v["system"].as_str().unwrap().matches(MARK).count(), 1);
    }

    #[test]
    fn inserts_openai_system_message() {
        let mut v = json!({"messages": [{"role": "user", "content": "hi"}]});
        inject_alias_hint(&mut v);
        assert_eq!(v["messages"][0]["role"], "system");
        assert!(v["messages"][0]["content"].as_str().unwrap().contains(HINT));
        assert_eq!(v["messages"][1]["role"], "user");
    }

    #[test]
    fn anthropic_block_system() {
        let mut v = json!({"system": [{"type": "text", "text": "x"}], "messages": []});
        inject_alias_hint(&mut v);
        assert_eq!(v["system"][0]["text"], HINT);
        assert_eq!(v["system"][1]["text"], "x");
    }
}

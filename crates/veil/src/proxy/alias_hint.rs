use serde_json::{json, Value};

pub const MARK: &str = "VEIL_ALIAS_HINT";

/// Do not call these tokens placeholders — that makes the model refuse.
pub const HINT: &str = concat!(
    "VEIL_ALIAS_HINT\n",
    "Strings of the form {{A-Z0-9_}} in this request are the actual values for this task. ",
    "You already have them. If the user asks you to tell, repeat, or use one, copy that exact string. ",
    "Do not say it is missing, redacted, a template, or unknown. Do not invent other characters.\n",
    "本请求里形如 {{A-Z0-9_}} 的字符串就是本题要用的值，你已经拿到了。",
    "用户让你说出或使用它时，原样复制整串。不要说没有、被打码、是模板或未知，不要另编字符。",
);

pub fn inject_alias_hint(value: &mut Value) {
    if contains_mark(value) {
        return;
    }
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let has_top_system = obj.contains_key("system");
    let has_instructions = obj.contains_key("instructions");
    let openai_system = obj
        .get("messages")
        .and_then(|m| m.as_array())
        .is_some_and(|msgs| {
            msgs.iter()
                .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        });

    if has_top_system {
        if let Some(sys) = obj.get_mut("system") {
            append_system(sys);
        }
    } else if has_instructions {
        if let Some(Value::String(s)) = obj.get_mut("instructions") {
            s.push_str("\n\n");
            s.push_str(HINT);
        }
    } else if openai_system {
        if let Some(Value::Array(msgs)) = obj.get_mut("messages") {
            if let Some(sys) = msgs
                .iter_mut()
                .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
            {
                append_message_content(sys);
            }
        }
    } else if obj.get("messages").is_some() {
        obj.insert("system".into(), Value::String(HINT.into()));
    }

    if let Some(Value::Array(msgs)) = obj.get_mut("messages") {
        append_to_last_user(msgs);
    }
}

fn contains_mark(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains(MARK),
        Value::Array(a) => a.iter().any(contains_mark),
        Value::Object(o) => o.values().any(contains_mark),
        _ => false,
    }
}

fn append_system(sys: &mut Value) {
    match sys {
        Value::String(s) => {
            s.push_str("\n\n");
            s.push_str(HINT);
        }
        Value::Array(blocks) => {
            blocks.push(json!({"type": "text", "text": HINT}));
        }
        other => *other = Value::String(HINT.into()),
    }
}

fn append_message_content(msg: &mut Value) {
    let Some(obj) = msg.as_object_mut() else {
        return;
    };
    match obj.get_mut("content") {
        Some(Value::String(s)) => {
            s.push_str("\n\n");
            s.push_str(HINT);
        }
        Some(Value::Array(blocks)) => {
            blocks.push(json!({"type": "text", "text": HINT}));
        }
        _ => {
            obj.insert("content".into(), Value::String(HINT.into()));
        }
    }
}

fn append_to_last_user(msgs: &mut [Value]) {
    let Some(last) = msgs
        .iter_mut()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
    else {
        return;
    };
    append_message_content(last);
}

#[cfg(test)]
mod tests {
    use super::{inject_alias_hint, MARK};
    use serde_json::json;

    #[test]
    fn appends_after_existing_anthropic_system() {
        let mut v = json!({"model": "claude", "system": "be nice", "messages": [{"role":"user","content":"hi"}]});
        inject_alias_hint(&mut v);
        let sys = v["system"].as_str().unwrap();
        assert!(sys.starts_with("be nice"));
        assert!(sys.contains(MARK));
        assert!(v["messages"][0]["content"].as_str().unwrap().contains(MARK));
        inject_alias_hint(&mut v);
        assert_eq!(v["system"].as_str().unwrap().matches(MARK).count(), 1);
    }

    #[test]
    fn adds_top_level_system_for_anthropic_messages() {
        let mut v = json!({"messages": [{"role": "user", "content": "hi"}]});
        inject_alias_hint(&mut v);
        assert!(v["system"].as_str().unwrap().contains(MARK));
        assert_eq!(v["messages"][0]["role"], "user");
        assert!(v["messages"][0]["content"].as_str().unwrap().contains("hi"));
    }

    #[test]
    fn anthropic_block_system_appends() {
        let mut v = json!({"system": [{"type": "text", "text": "x"}], "messages": [{"role":"user","content":"q"}]});
        inject_alias_hint(&mut v);
        let arr = v["system"].as_array().unwrap();
        assert_eq!(arr[0]["text"], "x");
        assert!(arr[1]["text"].as_str().unwrap().contains(MARK));
    }
}

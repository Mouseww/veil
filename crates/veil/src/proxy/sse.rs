use serde_json::Value;
use veil_engine::creator::Creator;
use veil_engine::mapping::MappingStore;
use veil_engine::sliding::RestoreWindow;
use veil_engine::walk::{restore_json, WalkError};

pub struct SseRestorer<'a, S: MappingStore + ?Sized> {
    store: &'a S,
    creator: Creator,
    window: RestoreWindow<'a, S>,
    buf: String,
}

impl<'a, S: MappingStore + ?Sized> SseRestorer<'a, S> {
    pub fn new(store: &'a S, creator: Creator) -> Self {
        Self {
            store,
            creator,
            window: RestoreWindow::new(store, creator),
            buf: String::new(),
        }
    }

    pub fn push(&mut self, chunk: &str) -> Result<String, WalkError> {
        self.buf.push_str(chunk);
        self.drain(false)
    }

    pub fn flush(&mut self) -> Result<String, WalkError> {
        self.drain(true)
    }

    fn drain(&mut self, flush: bool) -> Result<String, WalkError> {
        let mut out = String::new();
        loop {
            let Some(idx) = find_event_break(&self.buf) else {
                break;
            };
            let end = if self.buf[idx..].starts_with("\r\n\r\n") {
                idx + 4
            } else {
                idx + 2
            };
            let event = self.buf[..idx].trim_end_matches(['\r', '\n']).to_string();
            self.buf.drain(..end);
            if event.is_empty() {
                continue;
            }
            out.push_str(&restore_event(
                &mut self.window,
                self.store,
                &self.creator,
                &event,
            )?);
            out.push_str("\n\n");
        }
        if flush && !self.buf.trim().is_empty() {
            let rest = std::mem::take(&mut self.buf);
            out.push_str(&restore_event(
                &mut self.window,
                self.store,
                &self.creator,
                rest.trim_end(),
            )?);
        }
        Ok(out)
    }
}

fn find_event_break(buf: &str) -> Option<usize> {
    if let Some(i) = buf.find("\r\n\r\n") {
        return Some(i);
    }
    buf.find("\n\n")
}

fn restore_event<S: MappingStore + ?Sized>(
    window: &mut RestoreWindow<'_, S>,
    store: &S,
    creator: &Creator,
    event: &str,
) -> Result<String, WalkError> {
    let mut meta: Vec<String> = Vec::new();
    let mut data_parts: Vec<String> = Vec::new();
    for line in event.split('\n') {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("data:") {
            data_parts.push(rest.trim_start().to_string());
        } else {
            meta.push(line.to_string());
        }
    }
    if data_parts.is_empty() {
        return Ok(event.to_string());
    }
    let data = data_parts.join("\n");
    let restored = restore_data_payload(window, store, creator, &data)?;
    meta.push(format_data_line(&restored));
    Ok(meta.join("\n"))
}

fn format_data_line(data: &str) -> String {
    let mut s = String::from("data: ");
    s.push_str(data);
    s
}

fn restore_data_payload<S: MappingStore + ?Sized>(
    window: &mut RestoreWindow<'_, S>,
    store: &S,
    creator: &Creator,
    data: &str,
) -> Result<String, WalkError> {
    if data == "[DONE]" {
        return Ok(data.to_string());
    }
    let Ok(mut value) = serde_json::from_str::<Value>(data) else {
        return window.push(data);
    };
    if apply_delta_window(window, &mut value)? {
        return Ok(value.to_string());
    }
    Ok(restore_json(&value, store, creator)?.to_string())
}

fn apply_delta_window<S: MappingStore + ?Sized>(
    window: &mut RestoreWindow<'_, S>,
    value: &mut Value,
) -> Result<bool, WalkError> {
    for pointer in [
        "/delta/text",
        "/delta/partial_json",
        "/choices/0/delta/content",
        "/delta",
    ] {
        if pointer == "/delta"
            && (value.pointer("/delta/text").is_some()
                || value.pointer("/delta/partial_json").is_some())
        {
            continue;
        }
        if let Some(Value::String(text)) = value.pointer_mut(pointer) {
            let restored = window.push(text)?;
            *text = restored;
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn restore_json_body<S: MappingStore + ?Sized>(
    store: &S,
    creator: &Creator,
    body: &[u8],
) -> Result<Vec<u8>, WalkError> {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return Ok(body.to_vec()),
    };
    let restored = restore_json(&value, store, creator)?;
    serde_json::to_vec(&restored).map_err(|_| WalkError::StoreUnavailable)
}

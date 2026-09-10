use crate::creator::Creator;
use crate::mapping::{Lookup, MappingStore, StoreError};
use crate::placeholder::{parse_placeholder, PLACEHOLDER_REGEX};
use crate::walk::WalkError;

/// Max tail buffer in bytes. Placeholders are at most 63 bytes; 96 is a bound.
const MAX_TAIL: usize = 96;

pub struct RestoreWindow<'a, S: MappingStore> {
    store: &'a S,
    creator: Creator,
    tail: String,
}

impl<'a, S: MappingStore> RestoreWindow<'a, S> {
    pub fn new(store: &'a S, creator: Creator) -> Self {
        Self {
            store,
            creator,
            tail: String::new(),
        }
    }

    pub fn push(&mut self, s: &str) -> Result<String, WalkError> {
        self.tail.push_str(s);
        self.emit(false)
    }

    pub fn flush(&mut self) -> Result<String, WalkError> {
        self.emit(true)
    }

    fn emit(&mut self, flush: bool) -> Result<String, WalkError> {
        let buf = std::mem::take(&mut self.tail);
        let mut out = String::new();
        let mut last = 0;
        for m in PLACEHOLDER_REGEX.find_iter(&buf) {
            out.push_str(&buf[last..m.start()]);
            out.push_str(&self.restore_token(m.as_str())?);
            last = m.end();
        }
        let remaining = &buf[last..];
        if flush {
            out.push_str(remaining);
            return Ok(out);
        }
        let hold_at = hold_suffix_start(remaining);
        out.push_str(&remaining[..hold_at]);
        let keep = &remaining[hold_at..];
        if keep.len() > MAX_TAIL && !keep.starts_with("{{") {
            out.push_str(keep);
        } else {
            self.tail = keep.to_string();
        }
        Ok(out)
    }

    fn restore_token(&self, token: &str) -> Result<String, WalkError> {
        let Some(placeholder) = parse_placeholder(token) else {
            return Ok(token.to_string());
        };
        match self.store.lookup(&self.creator, &placeholder) {
            Ok(Lookup::Hit(plaintext)) => Ok(plaintext),
            Ok(Lookup::Miss) => Ok(token.to_string()),
            Err(StoreError::Unavailable) | Err(StoreError::Internal(_)) => {
                Err(WalkError::StoreUnavailable)
            }
        }
    }
}

/// Byte index where a suffix that may still be a prefix of `{{TYPE_ULID}}` starts.
fn hold_suffix_start(s: &str) -> usize {
    let Some(i) = s.rfind('{') else {
        return s.len();
    };
    let bytes = s.as_bytes();
    if i > 0 && bytes[i - 1] == b'{' && is_placeholder_prefix(&s[i - 1..]) {
        return i - 1;
    }
    if is_placeholder_prefix(&s[i..]) {
        return i;
    }
    s.len()
}

fn is_placeholder_prefix(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let b = s.as_bytes();
    if b[0] != b'{' {
        return false;
    }
    if b.len() == 1 {
        return true;
    }
    if b[1] != b'{' {
        return false;
    }
    if b.len() == 2 {
        return true;
    }
    let rest = &s[2..];
    let mut type_len = 0;
    let mut ulid_start = None;
    for (offset, c) in rest.char_indices() {
        if c == '_' {
            if type_len == 0 {
                return false;
            }
            ulid_start = Some(2 + offset + 1);
            break;
        }
        if type_len == 0 {
            if !c.is_ascii_uppercase() {
                return false;
            }
        } else if !c.is_ascii_uppercase() && !c.is_ascii_digit() {
            return false;
        }
        type_len += 1;
        if type_len > 32 {
            return false;
        }
    }
    let Some(ulid_at) = ulid_start else {
        return true;
    };
    is_ulid_close_prefix(&s[ulid_at..])
}

fn is_crockford(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z')
}

fn is_ulid_close_prefix(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 26 {
        if b[i] == b'}' {
            return false;
        }
        if !is_crockford(b[i]) {
            return false;
        }
        i += 1;
    }
    match &s[i..] {
        "" | "}" => true,
        "}}" => false,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creator::anonymous_creator;
    use crate::mapping::{DownStore, MappingStore, MemoryStore};
    use crate::placeholder::Placeholder;
    use crate::walk::WalkError;

    #[test]
    fn restores_placeholder_split_across_pushes() {
        let store = MemoryStore::new();
        let c = anonymous_creator();
        let p = store.get_or_insert(&c, "PHONE", "13800138000").unwrap();
        let token = p.format();
        let mid = token.len() / 2;
        let mut w = RestoreWindow::new(&store, c);
        let mut out = String::new();
        out.push_str(&w.push(&token[..mid]).unwrap());
        out.push_str(&w.push(&token[mid..]).unwrap());
        out.push_str(&w.flush().unwrap());
        assert_eq!(out, "13800138000");
    }

    #[test]
    fn emits_safe_prefix_immediately() {
        let store = MemoryStore::new();
        let mut w = RestoreWindow::new(&store, anonymous_creator());
        let out = w.push("hello ").unwrap();
        assert_eq!(out, "hello ");
    }

    #[test]
    fn unknown_valid_placeholder_emitted_unchanged() {
        let store = MemoryStore::new();
        let fake = Placeholder::fresh("PHONE").format();
        let mut w = RestoreWindow::new(&store, anonymous_creator());
        let mut out = w.push(&fake).unwrap();
        out.push_str(&w.flush().unwrap());
        assert_eq!(out, fake);
    }

    #[test]
    fn store_unavailable_with_valid_token_is_error() {
        let store = DownStore;
        let fake = Placeholder::fresh("PHONE").format();
        let mut w = RestoreWindow::new(&store, anonymous_creator());
        let err = w.push(&fake).unwrap_err();
        assert!(matches!(err, WalkError::StoreUnavailable));
    }
}

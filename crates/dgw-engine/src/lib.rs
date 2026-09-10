pub fn crate_name() -> &'static str {
    "dgw-engine"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "dgw-engine");
    }
}

pub mod builtin;
pub mod creator;
mod ip;
pub mod mapping;
pub mod placeholder;
pub mod rules;
pub mod sliding;
pub mod walk;

pub fn crate_name() -> &'static str {
    "veil-engine"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "veil-engine");
    }
}

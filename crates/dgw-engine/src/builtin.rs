use crate::rules::{Allowlist, Rule, RuleSet};

/// Built-in rule pack with the default allowlist.
pub fn builtin_ruleset() -> RuleSet {
    let mut email = Rule::regex(
        "email",
        "EMAIL",
        r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
        80,
    );
    email.enabled = false;

    RuleSet::new(
        vec![
            Rule::regex(
                "aksk",
                "AKSK",
                r"-----BEGIN [A-Z0-9 ]+PRIVATE KEY-----|AKIA[0-9A-Z]{16}|sk-ant-\S+|sk-[A-Za-z0-9_-]{20,}|Bearer eyJ\S+",
                200,
            ),
            Rule::regex(
                "connstr",
                "CONNSTR",
                r"(?i)(?:postgres|postgresql|mysql|mongodb|redis|mssql|sqlserver)://\S+|Pwd\s*=\s*[^;\s]+",
                180,
            ),
            Rule::regex("idcard", "IDCARD", r"(?<!\d)\d{17}[\dXx](?!\d)", 150),
            Rule::regex("phone", "PHONE", r"(?<!\d)1[3-9]\d{9}(?!\d)", 140),
            Rule::ip("ip", "IP", 120),
            email,
        ],
        default_allowlist(),
    )
}

fn default_allowlist() -> Allowlist {
    Allowlist::from(["127.0.0.1", "localhost", "0.0.0.0", "::1"])
}

#[cfg(test)]
mod tests {
    use super::builtin_ruleset;

    #[test]
    fn parses_public_and_private_ips() {
        let set = builtin_ruleset();
        let hits = set.find_hits("edge 8.8.8.8 and dc 10.0.0.5");
        let values: Vec<_> = hits
            .iter()
            .filter(|h| h.type_prefix == "IP")
            .map(|h| h.plaintext.as_str())
            .collect();
        assert!(values.contains(&"8.8.8.8"));
        assert!(values.contains(&"10.0.0.5"));
    }

    #[test]
    fn loopback_allowlisted() {
        let set = builtin_ruleset();
        assert!(set
            .find_hits("http://127.0.0.1:18787")
            .iter()
            .all(|h| h.type_prefix != "IP"));
        assert!(set
            .find_hits("listen on ::1")
            .iter()
            .all(|h| h.plaintext != "::1"));
    }

    #[test]
    fn china_mobile_and_id() {
        let set = builtin_ruleset();
        let t = "mobile 13800138000 id 110101199003078515";
        let hits = set.find_hits(t);
        let types: Vec<_> = hits.iter().map(|h| h.type_prefix.as_str()).collect();
        assert!(types.contains(&"PHONE"));
        assert!(types.contains(&"IDCARD"));
    }

    #[test]
    fn connection_string_beats_inner_ip() {
        let set = builtin_ruleset();
        let hits = set.find_hits("postgres://u:s3cret@10.1.2.3:5432/app");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_prefix, "CONNSTR");
    }

    #[test]
    fn akia_and_pem_header() {
        let set = builtin_ruleset();
        let hits = set.find_hits("AKIAIOSFODNN7EXAMPLE and -----BEGIN RSA PRIVATE KEY-----");
        let types: Vec<_> = hits.iter().map(|h| h.type_prefix.as_str()).collect();
        assert!(types.contains(&"AKSK"));
    }

    #[test]
    fn email_builtin_disabled() {
        let set = builtin_ruleset();
        assert!(set
            .find_hits("a@example.com")
            .iter()
            .all(|h| h.type_prefix != "EMAIL"));
    }

    #[test]
    fn garbage_is_not_an_ip() {
        assert!("not-an-ip".parse::<std::net::IpAddr>().is_err());
        let set = builtin_ruleset();
        assert!(set
            .find_hits("commit deadbeef")
            .iter()
            .all(|h| h.type_prefix != "IP"));
    }
}

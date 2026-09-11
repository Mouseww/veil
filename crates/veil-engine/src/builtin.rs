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
                "pem",
                "PEM",
                r"-----BEGIN [A-Z0-9 ]+PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]+PRIVATE KEY-----",
                220,
            ),
            Rule::regex(
                "apikey",
                "APIKEY",
                r"(?i)(?:api[_-]?key|access[_-]?key)\s*[:=]\s*\S{8,}|(?<![A-Za-z0-9])(?:sk-ant-|sk-|rk-|pk-live-|pk-test-|xai-|ghp_|gho_|github_pat_|AIza)[A-Za-z0-9_\-]{8,}|AKIA[0-9A-Z]{16}",
                210,
            ),
            Rule::regex(
                "token",
                "TOKEN",
                r"(?i)(?:api[_-]?token|access[_-]?token|auth[_-]?token|refresh[_-]?token)\s*[:=]\s*\S{8,}|(?i)Bearer\s+[A-Za-z0-9\-._~+/]+=*|(?<![A-Za-z0-9])(?:xox[baprs]-|glpat-|npm_[A-Za-z0-9]{20,}|eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}\.)",
                205,
            ),
            Rule::regex(
                "connstr",
                "CONNSTR",
                r#"(?i)(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|rediss|mssql|sqlserver|mariadb|amqp|amqps|jdbc:\w+)://\S+|(?:Server|Data Source|Initial Catalog)\s*=\s*[^;]+|(?:Pwd|Password|PWD)\s*=\s*[^;\s"]+"#,
                180,
            ),
            Rule::regex(
                "password",
                "PASSWORD",
                r#"(?i)(?:password|passwd|passphrase|login[_-]?password|登录密码|口令)\s*[:=]\s*(?:['"][^'"]{4,}['"]|\S{4,})"#,
                170,
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

    fn types(text: &str) -> Vec<String> {
        builtin_ruleset()
            .find_hits(text)
            .into_iter()
            .map(|h| h.type_prefix)
            .collect()
    }

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
        let t = types("mobile 13800138000 id 110101199003078515");
        assert!(t.contains(&"PHONE".into()));
        assert!(t.contains(&"IDCARD".into()));
    }

    #[test]
    fn connection_string_beats_inner_ip() {
        let hits = builtin_ruleset().find_hits("postgres://u:s3cret@10.1.2.3:5432/app");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].type_prefix, "CONNSTR");
    }

    #[test]
    fn jdbc_and_sql_password() {
        let t = types("jdbc:mysql://db:3306/app Password=hunter2;");
        assert!(t.contains(&"CONNSTR".into()), "{t:?}");
    }

    #[test]
    fn login_password_assignment() {
        let hits = builtin_ruleset().find_hits("ssh password: hunter2-ok");
        assert!(
            hits.iter()
                .any(|h| h.type_prefix == "PASSWORD" && h.plaintext.contains("hunter2")),
            "{hits:?}"
        );
    }

    #[test]
    fn api_key_and_sk() {
        let t = types("api_key=sk-abcdefghijklmnopqrstuvwxyz1234");
        assert!(t.contains(&"APIKEY".into()), "{t:?}");
        let t = types("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaaa.bb");
        assert!(t.contains(&"TOKEN".into()), "{t:?}");
        let t = types("xoxb-1234567890-abcdefghij");
        assert!(t.contains(&"TOKEN".into()), "{t:?}");
    }

    #[test]
    fn akia_and_pem_header() {
        let t = types("AKIAIOSFODNN7EXAMPLE and -----BEGIN RSA PRIVATE KEY-----\nMIIB\n-----END RSA PRIVATE KEY-----");
        assert!(
            t.contains(&"APIKEY".into()) || t.contains(&"PEM".into()),
            "{t:?}"
        );
    }

    #[test]
    fn email_builtin_disabled() {
        assert!(builtin_ruleset()
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
        assert!(set
            .find_hits("v1.2.3.4")
            .iter()
            .all(|h| h.type_prefix != "IP"));
    }

    fn ip_plaintexts(text: &str) -> Vec<String> {
        builtin_ruleset()
            .find_hits(text)
            .into_iter()
            .filter(|h| h.type_prefix == "IP")
            .map(|h| h.plaintext)
            .collect()
    }

    #[test]
    fn ip_hits_assignment_quotes_at_json_and_sentence_punct() {
        for (text, want) in [
            ("src_ip=10.0.0.5", "10.0.0.5"),
            ("Reach 10.0.0.5. Then stop.", "10.0.0.5"),
            ("host '8.8.8.8'", "8.8.8.8"),
            (r#""10.1.2.3""#, "10.1.2.3"),
            ("admin@10.0.0.5", "10.0.0.5"),
            (r#"{"ip":"8.8.8.8"}"#, "8.8.8.8"),
            ("见 10.0.0.5。", "10.0.0.5"),
        ] {
            let values = ip_plaintexts(text);
            assert!(
                values.iter().any(|v| v == want),
                "{text:?} expected IP {want:?}, got {values:?}"
            );
        }
        assert!(ip_plaintexts("http://127.0.0.1:18787").is_empty());
        assert!(ip_plaintexts("commit deadbeef").is_empty());
    }

    #[test]
    fn pem_match_includes_body_through_end() {
        let pem = "-----BEGIN RSA PRIVATE KEY-----
MIIBOgIBAAJBAK8=
-----END RSA PRIVATE KEY-----";
        let hits = builtin_ruleset().find_hits(pem);
        let pem_hits: Vec<_> = hits.iter().filter(|h| h.type_prefix == "PEM").collect();
        assert_eq!(pem_hits.len(), 1, "expected one PEM hit, got {hits:?}");
        assert!(
            pem_hits[0].plaintext.contains("MIIBOgIBAAJBAK8="),
            "PEM hit must include the base64 body, got {:?}",
            pem_hits[0].plaintext
        );
        assert!(pem_hits[0]
            .plaintext
            .contains("-----END RSA PRIVATE KEY-----"));
        assert!(pem_hits[0]
            .plaintext
            .starts_with("-----BEGIN RSA PRIVATE KEY-----"));
    }

    #[test]
    fn sk_requires_boundary_lookbehind() {
        let set = builtin_ruleset();
        assert!(
            set.find_hits("task-abcdefghijklmnopqrstuvwxyz1234")
                .iter()
                .all(|h| h.type_prefix != "APIKEY"),
            "embedded sk- in task-... must not be APIKEY"
        );
        let open_sk = set.find_hits("sk-abcdefghijklmnopqrstuvwxyz1234");
        assert!(
            open_sk.iter().any(|h| h.type_prefix == "APIKEY"),
            "boundary sk-<20+> must match, got {open_sk:?}"
        );
        let ant = set.find_hits("token sk-ant-api03-secret");
        assert!(
            ant.iter()
                .any(|h| h.type_prefix == "APIKEY" && h.plaintext.contains("sk-ant-")),
            "boundary sk-ant- must match, got {ant:?}"
        );
    }
}

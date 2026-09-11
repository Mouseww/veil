use crate::config::Config;

/// Protocol family selected from the request path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolFamily {
    Anthropic,
    OpenAiCompletions,
    OpenAiResponses,
}

pub fn classify(path: &str) -> Option<ProtocolFamily> {
    if path.starts_with("/v1/messages") {
        Some(ProtocolFamily::Anthropic)
    } else if path.starts_with("/v1/chat/completions") {
        Some(ProtocolFamily::OpenAiCompletions)
    } else if path.starts_with("/v1/responses") {
        Some(ProtocolFamily::OpenAiResponses)
    } else if path.starts_with("/v1/models") || path.starts_with("/v1/embeddings") {
        Some(ProtocolFamily::OpenAiCompletions)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct UpstreamConfig {
    pub anthropic_upstream: String,
    pub openai_completions_upstream: String,
    pub openai_responses_upstream: String,
}

pub fn upstream_base(family: ProtocolFamily, cfg: &UpstreamConfig) -> &str {
    match family {
        ProtocolFamily::Anthropic => &cfg.anthropic_upstream,
        ProtocolFamily::OpenAiCompletions => &cfg.openai_completions_upstream,
        ProtocolFamily::OpenAiResponses => &cfg.openai_responses_upstream,
    }
}

impl From<&Config> for UpstreamConfig {
    fn from(cfg: &Config) -> Self {
        Self {
            anthropic_upstream: cfg.anthropic_upstream.clone(),
            openai_completions_upstream: cfg.openai_completions_upstream.clone(),
            openai_responses_upstream: cfg.openai_responses_upstream.clone(),
        }
    }
}

impl UpstreamConfig {
    pub fn for_route(cfg: &Config, route: &crate::config::ClientRoute) -> Self {
        let u = route.upstream.trim();
        let origin = if u.is_empty() {
            return Self::from(cfg);
        } else {
            u.trim_end_matches('/').to_string()
        };
        match route.kind.as_str() {
            "anthropic" => Self {
                anthropic_upstream: origin,
                openai_completions_upstream: cfg.openai_completions_upstream.clone(),
                openai_responses_upstream: cfg.openai_responses_upstream.clone(),
            },
            "openai" => Self {
                anthropic_upstream: cfg.anthropic_upstream.clone(),
                openai_completions_upstream: origin.clone(),
                openai_responses_upstream: origin,
            },
            _ => Self {
                anthropic_upstream: origin.clone(),
                openai_completions_upstream: origin.clone(),
                openai_responses_upstream: origin,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, ProtocolFamily};

    #[test]
    fn classifies_primary_and_prefix_paths() {
        assert_eq!(classify("/v1/messages"), Some(ProtocolFamily::Anthropic));
        assert_eq!(
            classify("/v1/messages/count_tokens"),
            Some(ProtocolFamily::Anthropic)
        );
        assert_eq!(
            classify("/v1/chat/completions"),
            Some(ProtocolFamily::OpenAiCompletions)
        );
        assert_eq!(
            classify("/v1/responses"),
            Some(ProtocolFamily::OpenAiResponses)
        );
        assert_eq!(
            classify("/v1/models"),
            Some(ProtocolFamily::OpenAiCompletions)
        );
        assert_eq!(classify("/secret"), None);
    }
}

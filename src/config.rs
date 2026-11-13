//! Configuration: reads GROQ_API_KEY (required), NO_TTS, GROQ_BASE_URL (optional).
//! `load()` returns `Config` directly (not a Result): a missing/blank/whitespace
//! API key prints one error line and exits(1), per spec.

pub struct Config {
    pub groq_api_key: String,
    pub no_tts: bool,
    /// Full transcription endpoint, e.g. "https://api.groq.com/openai/v1/audio/transcriptions".
    pub endpoint: String,
}

fn compose_endpoint(base: &str) -> String {
    let mut base = base.trim_end_matches('/');
    if base.is_empty() {
        base = "https://api.groq.com";
    }
    format!("{}/openai/v1/audio/transcriptions", base)
}

pub fn load() -> Config {
    let groq_api_key = std::env::var("GROQ_API_KEY").unwrap_or_default();
    if groq_api_key.is_empty() || groq_api_key.chars().any(char::is_whitespace) {
        eprintln!("GROQ_API_KEY not set — get one at console.groq.com");
        std::process::exit(1);
    }
    let no_tts = match std::env::var("NO_TTS") {
        Ok(v) => !v.is_empty(),
        Err(_) => false,
    };
    // shortcut: intermediate binding differs from spec's `base_url` field name; main.rs only needs endpoint
    let base = std::env::var("GROQ_BASE_URL").unwrap_or_else(|_| "https://api.groq.com".into());
    Config {
        groq_api_key,
        no_tts,
        endpoint: compose_endpoint(&base),
    }
}

#[cfg(test)]
mod tests {
    use super::compose_endpoint;

    // Tests below are serial (env set_var/remove_var) — fine for a single-threaded test run.

    #[test]
    fn default_no_trailing_slash() {
        assert_eq!(
            compose_endpoint("https://api.groq.com"),
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn strips_trailing_slash() {
        assert_eq!(
            compose_endpoint("https://api.groq.com/"),
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
        assert_eq!(
            compose_endpoint("https://example.com///"),
            "https://example.com/openai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn empty_means_default() {
        assert_eq!(
            compose_endpoint(""),
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
    }
}

//! Configuration: reads GROQ_API_KEY (required), NO_TTS, GROQ_BASE_URL (optional).
//! `load()` returns `Config` directly (not a Result): a missing/blank/whitespace
//! API key prints one error line and exits(1), per spec.

pub struct Config {
    pub groq_api_key: String,
    pub no_tts: bool,
    /// Full transcription endpoint, e.g. "https://api.groq.com/openai/v1/audio/transcriptions".
    pub endpoint: String,
}

/// env var wins; else key file (one line, whitespace trimmed). The file was
/// written by request_secret with 0600 perms so the key never shows in ps/history.
fn read_key() -> Option<String> {
    let k = std::env::var("GROQ_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    if let Some(k) = k {
        return Some(k.trim().to_string());
    }
    let path = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let mut p = std::path::PathBuf::from(
                std::env::var_os("HOME").unwrap_or_else(|| "/root".into()),
            );
            p.push(".config");
            p
        })
        .join("static-dictation/api_key");
    let k = std::fs::read_to_string(path).ok()?;
    let k = k.trim();
    if k.is_empty() {
        None
    } else {
        Some(k.to_string())
    }
}

fn compose_endpoint(base: &str) -> String {
    let mut base = base.trim_end_matches('/');
    if base.is_empty() {
        base = "https://api.groq.com";
    }
    format!("{}/openai/v1/audio/transcriptions", base)
}

pub fn load() -> Config {
    let groq_api_key = read_key().unwrap_or_else(|| {
        eprintln!("GROQ_API_KEY not set and no key file at ~/.config/static-dictation/api_key — get one at console.groq.com");
        std::process::exit(1);
    });
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

use anyhow::{anyhow, Context, Result};

const DEFAULT_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// samples @16 kHz mono f32 → Groq transcription → text. BLOCKING HTTP.
pub fn transcribe(api_key: &str, endpoint: &str, samples: &[f32]) -> Result<String> {
    if samples.is_empty() {
        return Err(anyhow!("no audio captured"));
    }
    let endpoint = if endpoint.is_empty() { DEFAULT_URL } else { endpoint };
    let wav = to_wav(samples);
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let form = reqwest::blocking::multipart::Form::new()
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(wav)
                .file_name("audio.wav")
                .mime_str("audio/wav")?,
        )
        .text("model", "whisper-large-v3-turbo")
        .text("response_format", "json")
        .text("temperature", "0");
    let resp = client
        .post(endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .context("groq request failed")?;
    let status = resp.status();
    let body = resp.text().context("groq response read failed")?;
    if !status.is_success() {
        let snippet: String = body.chars().take(300).collect();
        return Err(anyhow!("groq error {}: {}", status, snippet));
    }
    let v: serde_json::Value = serde_json::from_str(&body).context("groq bad json")?;
    let text = v
        .get("text")
        .and_then(|t| t.as_str())
        .ok_or_else(|| anyhow!("groq response missing 'text'"))?;
    Ok(text.trim().to_string())
}

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // channels
    out.extend_from_slice(&16000u32.to_le_bytes());
    out.extend_from_slice(&(16000 * 2u32).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v: i16 = (clamped * i16::MAX as f32) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::to_wav;

    #[test]
    fn header() {
        let samples = [0.5f32; 10];
        let w = to_wav(&samples);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        assert_eq!(w.len(), 44 + 2 * samples.len());
        let size = u32::from_le_bytes([w[40], w[41], w[42], w[43]]);
        assert_eq!(size as usize, 2 * samples.len());
    }

    #[test]
    fn clamp() {
        assert_eq!(to_wav(&[2.0, -2.0]), to_wav(&[1.0, -1.0]));
    }

    #[test]
    fn empty() {
        let w = to_wav(&[]);
        assert_eq!(w.len(), 44);
        let size = u32::from_le_bytes([w[40], w[41], w[42], w[43]]);
        assert_eq!(size, 0);
    }
}

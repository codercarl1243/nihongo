/// Pipeline test — runs without the Tauri app.
///
/// Requires the Python sidecar running on localhost:8091.
/// Start it with:  cd sidecar && bash start.sh
/// Then run:       cargo run --bin test_pipeline --manifest-path src-tauri/Cargo.toml
///   or:           pnpm test:pipeline   (starts sidecar automatically)
///
/// Expected output:
///
///   [1/4] Sidecar health … PASS
///   [2/4] ASR round-trip  (TTS → resample 24k→16k → ASR) … PASS  (transcript = "こんにちは。")
///   [3/4] Kanji constraint — N5 profile → 0 kanji in response … PASS  (response = "こんにちは！")
///   [4/4] Kanji constraint — N2 profile → kanji present in response … PASS  (N kanji found — response = "…")
///
///   ✓ All tests passed.
///
/// Tests:
///   1. Health check   — sidecar responds on localhost:8091
///   2. ASR round-trip — speaks "こんにちは" via TTS, resamples 24kHz→16kHz,
///                       feeds to ASR, asserts transcript contains こんにちは
///   3. N5 constraint  — builds fake N5 SessionContext, asks LLM "こんにちは",
///                       asserts response contains 0 kanji (hiragana/katakana only)
///   4. N2 constraint  — builds fake N2 SessionContext, asks a substantive Japanese
///                       question, asserts response contains at least 1 kanji

use std::io::Write as _;
use tokio_stream::StreamExt;

use db::{LearnerProfile, SessionContext};
use llm::{SidecarClient, StreamItem};
use tutor::build_system_prompt_pub;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let llm = SidecarClient::new();
    let mut failures = 0;

    // ── 1. Health ────────────────────────────────────────────────────────────
    step(1, 4, "Sidecar health");
    llm.wait_until_ready(10).await?;
    pass(None);

    // ── 2. ASR round-trip ────────────────────────────────────────────────────
    step(2, 4, "ASR round-trip  (TTS → resample 24k→16k → ASR)");

    let phrase = "こんにちは";
    let wav = llm.speak(phrase).await?;
    let pcm_24k = wav_to_f32(&wav)?;
    let pcm_16k = resample(&pcm_24k, 24_000, 16_000);
    let transcript = llm.transcribe(&pcm_16k, None).await?;

    // Accept either common renderings of こんにちは
    let asr_ok = transcript.contains("こんにちは")
        || transcript.contains("今日は")
        || transcript.contains("コンニチハ");

    if asr_ok {
        pass(Some(format!("transcript = {:?}", transcript)));
    } else {
        fail(format!("expected こんにちは, got {:?}", transcript));
        failures += 1;
    }

    // ── 3. Kanji constraint — N5 (hiragana only) ─────────────────────────────
    step(3, 4, "Kanji constraint — N5 profile → 0 kanji in response");

    let response_n5 = ask_llm(&llm, 5, "こんにちは").await?;
    let kanji_count = count_kanji(&response_n5);

    if kanji_count == 0 {
        pass(Some(format!("response = {:?}", truncate(&response_n5, 80))));
    } else {
        fail(format!(
            "N5 response contained {kanji_count} kanji: {:?}",
            truncate(&response_n5, 120)
        ));
        failures += 1;
    }

    // ── 4. Kanji constraint — N2 (kanji expected) ────────────────────────────
    // Ask something that requires a content-heavy Japanese response so kanji
    // appear naturally rather than a one-word kana greeting.
    step(4, 4, "Kanji constraint — N2 profile → kanji present in response");

    let response_n2 = ask_llm(&llm, 2, "最近の勉強について教えてください。").await?;
    let kanji_count_n2 = count_kanji(&response_n2);

    if kanji_count_n2 > 0 {
        pass(Some(format!(
            "{kanji_count_n2} kanji found — response = {:?}",
            truncate(&response_n2, 80)
        )));
    } else {
        fail(format!(
            "N2 response had 0 kanji — expected Japanese with kanji: {:?}",
            truncate(&response_n2, 120)
        ));
        failures += 1;
    }

    // ── Summary ──────────────────────────────────────────────────────────────
    println!();
    if failures == 0 {
        println!("✓ All tests passed.");
    } else {
        println!("✗ {failures} test(s) failed.");
        std::process::exit(1);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a one-turn conversation with a fake learner profile at `level` and
/// return the full LLM response text.
async fn ask_llm(llm: &SidecarClient, level: u8, user_input: &str) -> anyhow::Result<String> {
    let ctx = SessionContext {
        profile: LearnerProfile { current_level: level, target_level: 1, total_words: 0 },
        current_topic: None,
        srs_due: vec![],
        last_notes: None,
    };

    let system_msg = build_system_prompt_pub(&ctx);
    let user_msg   = llm::ChatMessage::user(user_input);
    let messages   = vec![system_msg, user_msg];

    let mut full = String::new();
    let mut stream = llm.chat_stream(&messages).await?;
    while let Some(item) = stream.next().await {
        if let StreamItem::Token(tok) = item? {
            full.push_str(&tok);
        }
    }
    Ok(full)
}

/// Count CJK Unified Ideograph characters (kanji) in a string.
fn count_kanji(text: &str) -> usize {
    text.chars()
        .filter(|&c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
        .count()
}

/// Decode a WAV file (PCM 16-bit LE, standard 44-byte header) to f32 samples.
fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    anyhow::ensure!(wav.len() >= 44, "WAV payload too short ({} bytes)", wav.len());
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect())
}

/// Linear interpolation resample — adequate for test purposes.
fn resample(samples: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    let ratio = from_hz as f64 / to_hz as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let src = i as f64 * ratio;
            let idx = src as usize;
            let frac = (src - idx as f64) as f32;
            let a = samples.get(idx).copied().unwrap_or(0.0);
            let b = samples.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn step(n: u8, total: u8, label: &str) {
    print!("[{n}/{total}] {label} … ");
    std::io::stdout().flush().ok();
}

fn pass(detail: Option<String>) {
    match detail {
        Some(d) => println!("PASS  ({d})"),
        None    => println!("PASS"),
    }
}

fn fail(detail: String) {
    println!("FAIL  ({detail})");
}

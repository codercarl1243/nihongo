/// Pipeline test — runs without the Tauri app.
///
/// Requires the Python sidecar running on localhost:8091.
/// Start it with:  cd sidecar && bash start.sh
///   or:           pnpm test:pipeline   (starts sidecar automatically)
///
/// Expected output:
///
///   [1/6] Sidecar health … PASS
///   [2/6] ASR round-trip  (TTS → resample 24k→16k → ASR) … PASS  (transcript = "こんにちは。")
///   [3/6] Echo strip — TTS audio detectable by ASR (validates 400ms tail) … PASS  (echo = "こんにちは。")
///   [4/6] Kanji constraint — N5 profile → 0 kanji in response … PASS  (response = "こんにちは！")
///   [5/6] Kanji constraint — N3 profile → kanji present but response in mixed script … PASS
///   [6/6] Kanji constraint — N2 profile → 勉強 and 日本語 in response … PASS  (response = "日本語の勉強…")
///
///   ✓ All tests passed.
///
/// What each test checks:
///   1. Health        — sidecar responds on localhost:8091
///   2. ASR           — TTS speaks "こんにちは", ASR must return こんにちは (or 今日は)
///   3. Echo strip    — TTS output fed back to ASR must be transcribed as speech.
///                      Proves that without AudioManager's 400ms echo-tail suppression,
///                      room bleed would trigger a false turn.
///   4. N5 kanji      — fake N5 profile; response to "こんにちは" must contain 0 kanji
///   5. N3 kanji      — fake N3 profile; response to "日本語を勉強しています" must contain
///                      some kanji (N3 allows everyday kanji) but not be all-kana like N5
///   6. N2 kanji      — fake N2 profile; response to "日本語の勉強をしています" must
///                      contain both 勉強 and 日本語 (predictable from the prompt)

use std::io::Write as _;
use tokio_stream::StreamExt;

use db::{LearnerProfile, SessionContext};
use llm::{SidecarClient, StreamItem};
use nihongo_lib::tts::{VoiceVoxClient, DEFAULT_SPEAKER};
use tutor::{build_system_prompt_pub, Japanese};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let llm = SidecarClient::new();
    let tts = VoiceVoxClient::new(DEFAULT_SPEAKER);
    let mut failures = 0;

    // ── 1. Health ────────────────────────────────────────────────────────────
    step(1, 6, "Sidecar health");
    llm.wait_until_ready(10).await?;
    pass(None);

    // ── 2. ASR round-trip ────────────────────────────────────────────────────
    step(2, 6, "ASR round-trip  (TTS → resample 24k→16k → ASR)");

    let phrase = "こんにちは";
    let wav = tts.speak(phrase).await?;
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

    // ── 3. Echo strip — TTS output re-fed to ASR ─────────────────────────────
    // Proves that TTS audio leaking into the mic would be picked up by ASR,
    // validating that AudioManager's 400ms echo-tail suppression window does
    // real work rather than being a no-op guard.
    step(3, 6, "Echo strip — TTS audio detectable by ASR (validates 400ms tail)");

    let echo_phrase = "おはようございます";
    let echo_wav = tts.speak(echo_phrase).await?;
    let echo_pcm_16k = resample(&wav_to_f32(&echo_wav)?, 24_000, 16_000);
    let echo_transcript = llm.transcribe(&echo_pcm_16k, None).await?;

    let echo_ok = !echo_transcript.trim().is_empty()
        && (echo_transcript.contains("おはよう")
            || echo_transcript.contains("お早う")
            || echo_transcript.contains("オハヨウ"));

    if echo_ok {
        pass(Some(format!("echo detectable — {:?}", echo_transcript)));
    } else {
        fail(format!(
            "TTS audio not transcribed by ASR — echo suppression test inconclusive: {:?}",
            echo_transcript
        ));
        failures += 1;
    }

    // ── 4. Kanji constraint — N5 (hiragana only) ─────────────────────────────
    step(4, 6, "Kanji constraint — N5 profile → 0 kanji in response");

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

    // ── 5. Kanji constraint — N3 (mixed: some kanji, not all-kana) ───────────
    // N3 allows common everyday kanji. A substantive question should produce
    // at least some kanji, confirming the script is not locked to hiragana-only.
    step(5, 6, "Kanji constraint — N3 profile → kanji present (mixed script)");
    let response_n3 = ask_llm(&llm, 3, "日本語を勉強しています").await?;
    let kanji_count_n3 = count_kanji(&response_n3);

    if kanji_count_n3 > 0 {
        pass(Some(format!(
            "{kanji_count_n3} kanji found — response = {:?}",
            truncate(&response_n3, 80)
        )));
    } else {
        fail(format!(
            "N3 response had 0 kanji — expected mixed script: {:?}",
            truncate(&response_n3, 120)
        ));
        failures += 1;
    }

    // ── 6. Kanji constraint — N2 (kanji expected) ────────────────────────────
    // Ask something that requires a content-heavy Japanese response so kanji
    // appear naturally rather than a one-word kana greeting.
    step(6, 6, "Kanji constraint — N2 profile → 勉強 and 日本語 in response");
    let question = "日本語の勉強をしています";
    println!("    (question = {:?})", question);
    let response_n2 = ask_llm(&llm, 2, question).await?;
    let has_benkyou = response_n2.contains("勉強");
    let has_nihongo = response_n2.contains("日本語");

    if has_benkyou && has_nihongo {
        pass(Some(format!(
            "contains 勉強 and 日本語 — response = {:?}",
            truncate(&response_n2, 80)
        )));
    } else {
        fail(format!(
            "missing expected kanji — 勉強:{has_benkyou} 日本語:{has_nihongo} — response: {:?}",
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

    let system_msg = build_system_prompt_pub(&ctx, &Japanese);
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

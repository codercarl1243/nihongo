use std::sync::Arc;
use std::time::Duration;

use llm::{ChatMessage, ChatUsage, SidecarClient, StreamItem};
use tauri::{AppHandle, Emitter, Manager};
use tokio_stream::StreamExt;
use tutor::{Japanese, LangConfig, parse_response_pub, parse_summary_pub, build_system_prompt_pub};

use crate::events::{
    MicStatusEvent, PipelineStatusEvent, ResponseDoneEvent, TranscriptEvent,
};
use crate::tts::TtsEngine;
use crate::AppState;

// ---------------------------------------------------------------------------
// Per-turn pipeline
// All locks are acquired, used, and DROPPED before any .await.
// ---------------------------------------------------------------------------

pub async fn handle_turn(
    audio: Vec<f32>,
    app: &AppHandle,
    llm: &SidecarClient,
    tts: &TtsEngine,
) -> anyhow::Result<()> {
    let state = app.state::<AppState>();

    // ── 1. ASR ──────────────────────────────────────────────────────────────
    // No hard language constraint — auto-detect per utterance so both Japanese
    // and English are transcribed in their native script.
    app.emit("pipeline_status", PipelineStatusEvent { stage: "ASR: Transcribing".into() }).ok(); // [PIPELINE_DEBUG]
    let transcript = llm.transcribe(&normalize_peak(&audio), None).await?;
    eprintln!("[asr] raw={transcript:?}");
    if transcript.trim().is_empty() {
        return Ok(());
    }
    app.emit("transcript", TranscriptEvent { text: transcript.clone() })?;
    // User's turn is received — show thinking state immediately rather than
    // leaving the mic indicator on "Listening" through the whole pipeline.
    app.emit("mic_status", MicStatusEvent { active: false }).ok();

    app.emit("pipeline_status", PipelineStatusEvent { stage: "LLM: Generating".into() }).ok(); // [PIPELINE_DEBUG]

    // ── 2. Append user turn, snapshot history ────────────────────────────────
    // Lock acquired and dropped before any await.
    let messages: Vec<ChatMessage> = {
        let mut sg = state.session.lock().unwrap();
        let s = sg.as_mut().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        s.push_user(&transcript);
        s.current_messages().to_vec()
    };

    // ── 3+6. Pipeline: LLM stream → sentence channel → TTS → play ─────────
    // Sentences flow through two pipelined channels:
    //   1. sentence_tx/rx: LLM task → dispatcher (capacity 8)
    //   2. wav_handle_tx/rx: dispatcher → playback loop (capacity 8)
    // The dispatcher spawns a TTS task the moment each sentence arrives, so
    // sentence N+1's HTTP request is already queued before sentence N's WAV is
    // fully consumed by the playback loop.
    let (sentence_tx, mut sentence_rx) = tokio::sync::mpsc::channel::<String>(8);
    let (wav_handle_tx, mut wav_handle_rx) =
        tokio::sync::mpsc::channel::<tokio::task::JoinHandle<anyhow::Result<Vec<u8>>>>(8);

    let llm_clone  = llm.clone();
    let msgs_clone = messages.clone();
    let llm_task   = tokio::spawn(async move {
        let mut buf   = String::new();
        let mut full  = String::new();
        let mut usage = ChatUsage::default();
        let mut stream = llm_clone.chat_stream(&msgs_clone).await?;
        while let Some(item) = stream.next().await {
            match item? {
                StreamItem::Token(tok) => {
                    full.push_str(&tok);
                    buf.push_str(&tok);
                    while let Some(sent) = flush_sentence(&mut buf) {
                        sentence_tx.send(sent).await.ok();
                    }
                }
                StreamItem::Usage(u) => { usage = u; }
            }
        }
        // flush remainder (no terminal punctuation)
        let rem = buf.trim().to_string();
        if !rem.is_empty() { sentence_tx.send(rem).await.ok(); }
        Ok::<(String, ChatUsage), anyhow::Error>((full, usage))
    });

    // Dispatcher: receives sentences, immediately fires a TTS task per sentence,
    // and sends the JoinHandle (in order) to the playback loop.
    let tts_for_dispatcher  = tts.clone();
    let app_for_dispatcher  = app.clone();
    let dispatcher = tokio::spawn(async move {
        let state = app_for_dispatcher.state::<AppState>();
        while let Some(sentence) = sentence_rx.recv().await {
            if state.audio.lock().unwrap().barge_in_pending() { break; }
            // Strip parenthetical asides (translation notes, pronunciation guides)
            // before sending to TTS. Malformed fragments like "(How are you today?"
            // or "= お元気ですか?" cause Qwen3-TTS to loop, generating seconds of
            // repeated phonemes. Skip the sentence entirely if nothing speakable remains.
            let Some(clean) = clean_for_tts(&sentence) else { continue; };
            let tts = tts_for_dispatcher.clone();
            let handle = tokio::spawn(async move { tts.speak(&clean).await });
            if wav_handle_tx.send(handle).await.is_err() { break; }
        }
    });

    let mut total_samples = 0usize;
    let mut tts_started   = false;

    // Playback loop: awaits handles in arrival order (ordering preserved by the
    // mpsc channel), then plays each WAV. Runs inside an async block so
    // unmute() is guaranteed to fire even if a speak() future returns an error.
    let tts_result = async {
        while let Some(handle) = wav_handle_rx.recv().await {
            if !tts_started {
                app.emit("pipeline_status", PipelineStatusEvent { stage: "TTS: Synthesizing".into() }).ok(); // [PIPELINE_DEBUG]
                state.audio.lock().unwrap().mute();
                state.player.lock().unwrap().resume();
                tts_started = true;
            }
            // User spoke during TTS — stop queuing sentences and let the
            // barge-in flush the player buffer. Dropping wav_handle_rx (via
            // break) closes the channel, causing the dispatcher's send to fail
            // so it also stops firing new requests.
            if state.audio.lock().unwrap().barge_in_pending() {
                state.player.lock().unwrap().stop();
                break;
            }
            let wav = handle.await.map_err(|e| anyhow::anyhow!("TTS task panicked: {e}"))??;
            let pcm = wav_to_f32(&wav)?;
            total_samples += pcm.len();
            state.player.lock().unwrap().play_chunk(&pcm, 24_000)?;
            if let Some(ref sink) = *state.aec_sink.lock().unwrap() {
                sink.push(&pcm, 24_000);
            }
            // signal_audio_started is idempotent — only the first call sets the timestamp.
            state.audio.lock().unwrap().signal_audio_started();
        }
        Ok::<(), anyhow::Error>(())
    }.await;

    dispatcher.abort(); // no-op if the dispatcher already exited naturally

    // On TTS error: unmute immediately so the session is never left permanently muted.
    // On success: unmute is deferred until after wait_for_playback (see below) so the
    // audio doesn't get picked up by the mic and re-transcribed as user speech.
    if tts_started && tts_result.is_err() {
        state.audio.lock().unwrap().unmute();
        app.emit("mic_status", MicStatusEvent { active: true }).ok();
    }

    tts_result?;

    // Guard: if the LLM task fails after TTS has already started, the normal
    // unmute path (below) is never reached — leaving the mic permanently muted.
     let llm_join = llm_task.await;
    // Handle both task panic (Err) and LLM error (Ok(Err)) as error conditions
     if llm_join.as_ref().map(|r| r.is_err()).unwrap_or(true) && tts_started {
         state.audio.lock().unwrap().unmute();
         app.emit("mic_status", MicStatusEvent { active: true }).ok();
     }
    let (full_text, usage) = llm_join??;

    // Emit response immediately so text appears while audio is still playing.
    // milestone is false here — classification is not yet complete (runs in parallel).
    app.emit("response_done", ResponseDoneEvent {
        full_response: full_text.trim().to_string(),
        milestone: false,
        prompt_tokens: usage.prompt_tokens,
    })?;
    app.emit("pipeline_status", PipelineStatusEvent { stage: "Audio: Playing".into() }).ok(); // [PIPELINE_DEBUG]

    // Spawn classification in parallel with audio playback so it adds no latency.
    let llm_classify  = llm.clone();
    let tx_classify   = transcript.clone();
    let ft_classify   = full_text.clone();
    let classify_task = tokio::spawn(async move {
        llm_classify
            .classify_turn(&tx_classify, &ft_classify)
            .await
            .unwrap_or((false, false))
    });

    // ── Wait for all queued audio, then unmute ───────────────────────────────
    // Unmuting before wait_for_playback lets the 400ms echo suppression window
    // expire while audio is still playing, causing the TTS output to be captured
    // by the mic and re-transcribed as user speech. Unmuting after playback means
    // the suppression window only needs to cover actual room reverb (~50–100ms).
    if total_samples > 0 {
        let duration = Duration::from_secs_f64(total_samples as f64 / 24_000.0);
        wait_for_playback(&state, duration).await;
    }
    if tts_started {
        state.audio.lock().unwrap().unmute();
        app.emit("mic_status", MicStatusEvent { active: true }).ok();
    }

    app.emit("pipeline_status", PipelineStatusEvent { stage: "LLM: Classifying".into() }).ok(); // [PIPELINE_DEBUG]
    let (milestone, correction) = classify_task.await.unwrap_or((false, false));
    eprintln!("[classify] milestone={milestone} correction={correction}");

    app.emit("pipeline_status", PipelineStatusEvent { stage: "DB: Writing".into() }).ok(); // [PIPELINE_DEBUG]

    let tutor_resp = parse_response_pub(&full_text, milestone, correction);

    // Adjust silence threshold for the next turn based on what the tutor just said.
    // Drill prompts expect a short specific phrase → tighten to 500ms.
    // Open questions / conversation → reset to the level-appropriate default.
    let is_drill = {
        let sg = state.session.lock().unwrap();
        sg.as_ref().map(|s| s.lang().is_drill_prompt(&full_text)).unwrap_or(false)
    };
    if is_drill {
        state.audio.lock().unwrap().set_silence_threshold(500);
    } else {
        state.audio.lock().unwrap().reset_silence_threshold();
    }

    // ── 4. Persist turn (sync, lock dropped before await) ───────────────────
    let needs_compact: bool = {
        let mut sg = state.session.lock().unwrap();
        let s = sg.as_mut().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        let db = state.db.lock().unwrap();
        s.record_turn_sync(&transcript, &tutor_resp, &db)?
    };

    // ── 4b. Vocabulary introduction + fluency update ─────────────────────────
    // best-effort — never breaks the pipeline on DB errors.
    {
        let db = state.db.lock().unwrap();
        if let Ok(ctx) = db.session_context() {
            if let Some(ref active) = ctx.current_topic {
                if let Ok(ids) = db.find_vocab_in_text(&transcript, active.topic.id) {
                    for vid in &ids {
                        let _ = db.introduce_word(*vid);
                    }

                    // Update SRS fluency for words the student actively used when
                    // the tutor clearly affirmed or corrected them.
                    if tutor_resp.milestone || tutor_resp.correction {
                        let due_ids: std::collections::HashSet<i64> =
                            ctx.srs_due.iter().map(|v| v.vocabulary_id).collect();
                        for vid in ids {
                            if due_ids.contains(&vid) {
                                let _ = db.update_word_fluency(vid, tutor_resp.milestone);
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 5. Context compaction (if milestone + threshold reached) ────────────
    if needs_compact {
        compact_context(app, llm).await?;
    }

    app.emit("pipeline_status", PipelineStatusEvent { stage: "VAD 1: Listening".into() }).ok(); // [PIPELINE_DEBUG]

    Ok(())
}

/// Sleeps for `duration`, polling every 50ms for a barge-in (player stopped).
pub async fn wait_for_playback(state: &AppState, duration: Duration) {
    let step        = Duration::from_millis(50);
    let mut elapsed = Duration::ZERO;

    while elapsed < duration {
        tokio::time::sleep(step).await;
        elapsed += step;
        if state.player.lock().unwrap().is_stopped() {
            break;
        }
        if state.audio.lock().unwrap().barge_in_pending() {
            state.player.lock().unwrap().stop();
            break;
        }
    }
}

/// Build a summary, save it, then reset the session context.
/// Called after a milestone when the context threshold is crossed.
pub async fn compact_context(app: &AppHandle, llm: &SidecarClient) -> anyhow::Result<()> {
    app.emit("pipeline_status", PipelineStatusEvent { stage: "LLM: Compacting".into() }).ok(); // [PIPELINE_DEBUG]
    let state = app.state::<AppState>();

    // Snapshot the summary-request messages (lock dropped before await)
    let summary_msgs: Vec<ChatMessage> = {
        let sg = state.session.lock().unwrap();
        let s = sg.as_ref().ok_or_else(|| anyhow::anyhow!("no active session"))?;
        s.summary_request_messages()
    };

    let mut raw_summary = String::new();
    {
        let mut stream = llm.chat_stream(&summary_msgs).await?;
        while let Some(item) = stream.next().await {
            if let StreamItem::Token(tok) = item? {
                raw_summary.push_str(&tok);
            }
        }
    }

    let summary = parse_summary_pub(&raw_summary);

    let lang: LangConfig = {
        let sg = state.session.lock().unwrap();
        sg.as_ref().map(|s| s.lang()).unwrap_or_else(|| Arc::new(Japanese))
    };
    let new_system: ChatMessage = {
        let db = state.db.lock().unwrap();
        db.save_lesson_summary(&summary)?;
        let ctx = db.session_context()?;
        build_system_prompt_pub(&ctx, lang.as_ref())
    };

    {
        let mut sg = state.session.lock().unwrap();
        if let Some(s) = sg.as_mut() {
            s.reset_context(new_system);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// WAV → f32 (PCM_16 LE, standard 44-byte header)
// ---------------------------------------------------------------------------

pub fn wav_to_f32(wav: &[u8]) -> anyhow::Result<Vec<f32>> {
    let offset = find_wav_data_offset(wav)?;
    let samples = wav[offset..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
        .collect();
    Ok(samples)
}

/// Scan RIFF chunks to find the byte offset of the first `data` chunk payload.
/// This handles WAV files with extra chunks (e.g. VoiceVox's `fact` chunk)
/// between `fmt` and `data` that would otherwise be mis-parsed as audio.
fn find_wav_data_offset(wav: &[u8]) -> anyhow::Result<usize> {
    if wav.len() < 12 {
        anyhow::bail!("WAV payload too short");
    }
    let mut pos = 12; // skip the 12-byte RIFF/WAVE header
    while pos + 8 <= wav.len() {
        let id   = &wav[pos..pos + 4];
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if id == b"data" {
            return Ok(pos + 8);
        }
        // Chunks are word-aligned; skip id (4) + size field (4) + payload (size, rounded up)
        let next_pos = pos + 8 + size + (size & 1);
        if next_pos > wav.len() {
            anyhow::bail!("chunk size {} would exceed WAV length", size);
        }
        pos = next_pos;
    }
    anyhow::bail!("no data chunk found in WAV");
}

// ---------------------------------------------------------------------------
// Audio helpers
// ---------------------------------------------------------------------------

/// Extract the next complete sentence from `buf` (up to 。！？!?.), consuming it.
fn flush_sentence(buf: &mut String) -> Option<String> {
    // Include '.' so English sentences flush independently rather than accumulating
    // the entire response into one TTS request. Japanese sentences use 。 instead.
    const ENDS: &[char] = &['。', '！', '？', '!', '?', '.'];
    let pos = buf.find(|c: char| ENDS.contains(&c))?;
    let end = pos + buf[pos..].chars().next().unwrap().len_utf8();
    let sentence = buf[..end].trim().to_string();
    *buf = buf[end..].trim_start().to_string();
    if sentence.is_empty() { None } else { Some(sentence) }
}

/// Prepare a sentence for TTS by removing content that confuses LLM-based TTS models.
///
/// Strips:
/// - Parenthetical blocks `(…)` and `（…）` — LLMs emit these as translation notes
///   or pronunciation guides; they're meant to be read, not spoken.
/// - Leading non-word characters left over after stripping (e.g. `= `, `- `).
///
/// Returns `None` if nothing speakable remains after cleaning.
fn clean_for_tts(text: &str) -> Option<String> {
    let mut out   = String::with_capacity(text.len());
    let mut depth = 0u32;
    for ch in text.chars() {
        match ch {
            '(' | '（' => depth += 1,
            ')' | '）' => { depth = depth.saturating_sub(1); }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    // Strip any leading punctuation / symbols left after paren removal.
    let trimmed = out.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

/// Scale audio so its peak is 0.9, leaving silence untouched.
/// Compensates for low-gain microphones (e.g. earbuds).
fn normalize_peak(audio: &[f32]) -> Vec<f32> {
    let peak = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    if peak < 0.001 { return audio.to_vec(); }
    let scale = 0.9 / peak;
    audio.iter().map(|s| (s * scale).clamp(-1.0, 1.0)).collect()
}

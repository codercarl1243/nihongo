use std::time::{Duration, Instant};

pub struct BargeInDetector {
    streak: u32,
    confirmed: bool,
    buffer: Vec<f32>,
    streak_required: u32,
    rms_threshold: f32,
    start_delay_ms: u64,
    audio_started_ms: u64,
}

pub enum BargeInState {
    Suppressed,
    BelowGate,
    Streak(u32),
    Confirmed,
    Buffering,
}

impl BargeInDetector {
    pub fn new(streak_required: u32, rms_threshold: f32, start_delay_ms: u64) -> Self {
        Self {
            streak: 0,
            confirmed: false,
            buffer: Vec::new(),
            streak_required,
            rms_threshold,
            start_delay_ms,
            audio_started_ms: 0,
        }
    }

    /// Record the ms timestamp when TTS audio first reached the speaker.
    /// Idempotent: only the first call per turn takes effect.
    pub fn signal_audio_started(&mut self, now_ms: u64) {
        if self.audio_started_ms == 0 {
            self.audio_started_ms = now_ms;
        }
    }

    /// Reset all state. Call when `mute()` is triggered at the start of a TTS turn.
    pub fn reset(&mut self) {
        self.streak = 0;
        self.confirmed = false;
        self.buffer.clear();
        self.audio_started_ms = 0;
    }

    /// Returns true once audio has started AND `start_delay_ms` has elapsed.
    pub fn is_active(&self, now_ms: u64) -> bool {
        self.audio_started_ms != 0
            && now_ms.saturating_sub(self.audio_started_ms) >= self.start_delay_ms
    }

    /// Evaluate one VAD window chunk for barge-in.
    ///
    /// `is_speech` comes from a dedicated VAD instance (separate from the main turn VAD).
    /// `now_ms` is the current Unix-epoch millisecond timestamp.
    pub fn process_chunk(
        &mut self,
        chunk: &[f32],
        is_speech: bool,
        now_ms: u64,
    ) -> BargeInState {
        if !self.is_active(now_ms) {
            return BargeInState::Suppressed;
        }

        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        let passes_gate = rms >= self.rms_threshold && is_speech;

        if !passes_gate {
            if self.confirmed {
                // Keep buffering trailing silence — turn boundary decided on unmute.
                self.buffer.extend_from_slice(chunk);
                return BargeInState::Buffering;
            }
            self.streak = 0;
            return BargeInState::BelowGate;
        }

        if self.confirmed {
            self.buffer.extend_from_slice(chunk);
            return BargeInState::Buffering;
        }

        self.streak += 1;
        if self.streak >= self.streak_required {
            self.confirmed = true;
            self.buffer.clear();
            self.buffer.extend_from_slice(chunk);
            return BargeInState::Confirmed;
        }

        BargeInState::Streak(self.streak)
    }

    /// Drain the accumulated barge-in audio. Call on unmute to get buffered speech.
    pub fn take_buffer(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.buffer)
    }
}

pub struct EchoTailTracker {
    suppress_until: Option<Instant>,
    duration: Duration,
}

impl EchoTailTracker {
    pub fn new(duration: Duration) -> Self {
        Self { suppress_until: None, duration }
    }

    /// Start the suppression window from now.
    pub fn arm(&mut self) {
        self.suppress_until = Some(Instant::now() + self.duration);
    }

    /// Returns true while the suppression window is active.
    pub fn is_active(&self) -> bool {
        self.suppress_until.map(|t| Instant::now() < t).unwrap_or(false)
    }

    /// Cancel the suppression window immediately.
    pub fn disarm(&mut self) {
        self.suppress_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud_chunk(size: usize) -> Vec<f32> {
        // RMS = 0.1, well above the 0.025 threshold
        vec![0.1f32; size]
    }

    fn quiet_chunk(size: usize) -> Vec<f32> {
        vec![0.005f32; size]
    }

    const THRESHOLD: f32 = 0.025;
    const STREAK: u32 = 6;
    const DELAY_MS: u64 = 700;
    const CHUNK: usize = 512;

    fn detector() -> BargeInDetector {
        BargeInDetector::new(STREAK, THRESHOLD, DELAY_MS)
    }

    fn now_after_delay() -> u64 {
        1000 + DELAY_MS + 1
    }

    // --- BargeInDetector tests ---

    #[test]
    fn suppressed_before_delay_elapsed() {
        let mut d = detector();
        d.signal_audio_started(1000);
        // now_ms is only 1ms past start — delay not elapsed
        let state = d.process_chunk(&loud_chunk(CHUNK), true, 1001);
        assert!(matches!(state, BargeInState::Suppressed));
    }

    #[test]
    fn active_after_delay_elapsed() {
        let mut d = detector();
        d.signal_audio_started(1000);
        assert!(d.is_active(now_after_delay()));
    }

    #[test]
    fn rms_below_gate_does_not_build_streak() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let state = d.process_chunk(&quiet_chunk(CHUNK), true, now_after_delay());
        assert!(matches!(state, BargeInState::BelowGate));
        // Streak stays zero
        let state2 = d.process_chunk(&quiet_chunk(CHUNK), true, now_after_delay());
        assert!(matches!(state2, BargeInState::BelowGate));
    }

    #[test]
    fn streak_builds_on_successive_speech_chunks() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let now = now_after_delay();
        for i in 1..STREAK {
            let state = d.process_chunk(&loud_chunk(CHUNK), true, now);
            assert!(matches!(state, BargeInState::Streak(s) if s == i));
        }
    }

    #[test]
    fn confirmed_at_streak_required() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let now = now_after_delay();
        for _ in 0..STREAK - 1 {
            d.process_chunk(&loud_chunk(CHUNK), true, now);
        }
        let state = d.process_chunk(&loud_chunk(CHUNK), true, now);
        assert!(matches!(state, BargeInState::Confirmed));
        assert_eq!(d.take_buffer().len(), CHUNK);
    }

    #[test]
    fn streak_resets_on_one_quiet_chunk() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let now = now_after_delay();
        for _ in 0..STREAK - 1 {
            d.process_chunk(&loud_chunk(CHUNK), true, now);
        }
        // One quiet chunk resets the streak
        d.process_chunk(&quiet_chunk(CHUNK), true, now);
        // Next loud chunk should start at Streak(1) again
        let state = d.process_chunk(&loud_chunk(CHUNK), true, now);
        assert!(matches!(state, BargeInState::Streak(1)));
    }

    #[test]
    fn confirmed_buffers_subsequent_chunks() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let now = now_after_delay();
        for _ in 0..STREAK {
            d.process_chunk(&loud_chunk(CHUNK), true, now);
        }
        // One more chunk — already confirmed
        let state = d.process_chunk(&loud_chunk(CHUNK), true, now);
        assert!(matches!(state, BargeInState::Buffering));
        assert_eq!(d.take_buffer().len(), CHUNK * 2);
    }

    #[test]
    fn buffer_cleared_on_reset() {
        let mut d = detector();
        d.signal_audio_started(1000);
        let now = now_after_delay();
        for _ in 0..STREAK {
            d.process_chunk(&loud_chunk(CHUNK), true, now);
        }
        d.reset();
        assert!(d.take_buffer().is_empty());
        assert!(!d.is_active(now));
    }

    #[test]
    fn signal_audio_started_is_idempotent() {
        let mut d = detector();
        d.signal_audio_started(1000);
        d.signal_audio_started(9999); // second call should be ignored
        assert!(d.is_active(1000 + DELAY_MS + 1));
        // If second call had won, is_active at 9999+delay would differ
        assert!(!d.is_active(1001)); // still respects original start
    }

    // --- EchoTailTracker tests ---

    #[test]
    fn echo_tail_arms_and_expires() {
        let mut t = EchoTailTracker::new(Duration::from_millis(50));
        t.arm();
        assert!(t.is_active());
        std::thread::sleep(Duration::from_millis(60));
        assert!(!t.is_active());
    }

    #[test]
    fn echo_tail_disarm_cancels() {
        let mut t = EchoTailTracker::new(Duration::from_millis(5000));
        t.arm();
        assert!(t.is_active());
        t.disarm();
        assert!(!t.is_active());
    }
}

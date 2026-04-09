use anyhow::Result;
use std::collections::VecDeque;
use std::path::Path;

pub enum VadFrame {
    Speech(Vec<f32>),
    Noise,
}

pub trait VoiceActivityDetector: Send + Sync {
    fn push_frame(&mut self, frame: &[f32]) -> Result<VadFrame>;
    fn is_voice(&mut self, frame: &[f32]) -> Result<bool>;
    fn reset(&mut self) {}
}

pub struct SileroVad {
    engine: vad_rs::Vad,
    threshold: f32,
}

impl SileroVad {
    pub fn new(model_path: &Path, threshold: f32) -> Result<Self> {
        let path_str = model_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?;
        let engine = vad_rs::Vad::new(path_str, 16000)
            .map_err(|e| anyhow::anyhow!("Failed to create Silero VAD: {}", e))?;
        Ok(Self { engine, threshold })
    }
}

impl VoiceActivityDetector for SileroVad {
    fn push_frame(&mut self, frame: &[f32]) -> Result<VadFrame> {
        let is_voice = self.is_voice(frame)?;
        if is_voice {
            Ok(VadFrame::Speech(frame.to_vec()))
        } else {
            Ok(VadFrame::Noise)
        }
    }

    fn is_voice(&mut self, frame: &[f32]) -> Result<bool> {
        let result = self
            .engine
            .compute(frame)
            .map_err(|e| anyhow::anyhow!("VAD compute error: {}", e))?;
        Ok(result.prob > self.threshold)
    }

    fn reset(&mut self) {
        self.engine.reset();
    }
}

pub struct SmoothedVad {
    inner: Box<dyn VoiceActivityDetector>,
    prefill_frames: usize,
    hangover_frames: usize,
    onset_frames: usize,
    frame_buffer: VecDeque<Vec<f32>>,
    in_speech: bool,
    onset_counter: usize,
    hangover_counter: usize,
    frames_since_reset: usize,
}

impl SmoothedVad {
    pub fn new(
        inner: Box<dyn VoiceActivityDetector>,
        prefill_frames: usize,
        hangover_frames: usize,
        onset_frames: usize,
    ) -> Self {
        Self {
            inner,
            prefill_frames,
            hangover_frames,
            onset_frames,
            frame_buffer: VecDeque::new(),
            in_speech: false,
            onset_counter: 0,
            hangover_counter: 0,
            frames_since_reset: 0,
        }
    }
}

impl VoiceActivityDetector for SmoothedVad {
    fn push_frame(&mut self, frame: &[f32]) -> Result<VadFrame> {
        self.frames_since_reset += 1;
        let is_voice = self.inner.is_voice(frame)?;

        let frame_owned = frame.to_vec();

        if self.frames_since_reset <= self.prefill_frames {
            self.frame_buffer.push_back(frame_owned);
            return Ok(VadFrame::Noise);
        }

        if !self.in_speech {
            if is_voice {
                self.onset_counter += 1;
                self.frame_buffer.push_back(frame_owned);
                if self.onset_counter >= self.onset_frames {
                    self.in_speech = true;
                    self.hangover_counter = 0;
                    let mut speech = Vec::new();
                    while let Some(buf) = self.frame_buffer.pop_front() {
                        speech.extend_from_slice(&buf);
                    }
                    return Ok(VadFrame::Speech(speech));
                }
                return Ok(VadFrame::Noise);
            } else {
                self.onset_counter = 0;
                while self.frame_buffer.len() >= self.prefill_frames {
                    self.frame_buffer.pop_front();
                }
                self.frame_buffer.push_back(frame_owned);
                return Ok(VadFrame::Noise);
            }
        } else {
            if is_voice {
                self.hangover_counter = 0;
                return Ok(VadFrame::Speech(frame_owned));
            } else {
                self.hangover_counter += 1;
                if self.hangover_counter >= self.hangover_frames {
                    self.in_speech = false;
                    self.onset_counter = 0;
                    self.frame_buffer.clear();
                    return Ok(VadFrame::Noise);
                }
                return Ok(VadFrame::Speech(frame_owned));
            }
        }
    }

    fn is_voice(&mut self, frame: &[f32]) -> Result<bool> {
        self.inner.is_voice(frame)
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.frame_buffer.clear();
        self.in_speech = false;
        self.onset_counter = 0;
        self.hangover_counter = 0;
        self.frames_since_reset = 0;
    }
}

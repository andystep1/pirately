use rubato::{FftFixedIn, Resampler};

const OUTPUT_RATE: usize = 16000;
const FRAME_SAMPLES: usize = 480;
const CHUNK_SIZE: usize = 1024;
const SUB_CHUNKS: usize = 1;
const CHANNELS: usize = 1;

pub struct FrameResampler {
    resampler: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
}

impl FrameResampler {
    pub fn new(input_rate: usize) -> Self {
        let resampler = if input_rate == OUTPUT_RATE {
            None
        } else {
            Some(
                FftFixedIn::<f32>::new(
                    input_rate,
                    OUTPUT_RATE,
                    CHUNK_SIZE,
                    SUB_CHUNKS,
                    CHANNELS,
                )
                .expect("Failed to create resampler"),
            )
        };
        Self {
            resampler,
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, src: &[f32], mut emit: impl FnMut(&[f32])) {
        let resampled: Vec<f32> = match &mut self.resampler {
            None => src.to_vec(),
            Some(resampler) => {
                let mut all_output = Vec::new();
                let mut pos = 0;
                while pos < src.len() {
                    let needed = resampler.input_frames_next();
                    let chunk = if src.len() - pos >= needed {
                        let end = pos + needed;
                        let c = &src[pos..end];
                        pos = end;
                        c
                    } else {
                        let c = &src[pos..];
                        pos = src.len();
                        c
                    };

                    if chunk.len() < needed {
                        let mut padded = chunk.to_vec();
                        padded.resize(needed, 0.0f32);
                        let input = [&padded[..]];
                        if let Ok(out) = resampler.process(&input, None) {
                            for ch in &out {
                                all_output.extend_from_slice(ch);
                            }
                        }
                    } else {
                        let input = [chunk];
                        if let Ok(out) = resampler.process(&input, None) {
                            for ch in &out {
                                all_output.extend_from_slice(ch);
                            }
                        }
                    }
                }
                all_output
            }
        };

        self.pending.extend_from_slice(&resampled);

        while self.pending.len() >= FRAME_SAMPLES {
            let frame: Vec<f32> = self.pending.drain(..FRAME_SAMPLES).collect();
            emit(&frame);
        }
    }

    pub fn finish(&mut self, mut emit: impl FnMut(&[f32])) {
        if let Some(resampler) = &mut self.resampler {
            if let Ok(tail) = resampler.process_partial(None::<&[Vec<f32>]>, None) {
                for ch in &tail {
                    self.pending.extend_from_slice(ch);
                }
            }
        }

        while self.pending.len() >= FRAME_SAMPLES {
            let frame: Vec<f32> = self.pending.drain(..FRAME_SAMPLES).collect();
            emit(&frame);
        }

        if !self.pending.is_empty() {
            let mut padded = self.pending.clone();
            padded.resize(FRAME_SAMPLES, 0.0f32);
            emit(&padded);
            self.pending.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_when_rates_match() {
        let mut resampler = FrameResampler::new(16000);
        assert!(resampler.resampler.is_none());

        let mut frames = Vec::new();
        let input: Vec<f32> = (0..480).map(|i| i as f32 / 480.0).collect();
        resampler.push(&input, |f| frames.push(f.to_vec()));

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 480);
    }
}

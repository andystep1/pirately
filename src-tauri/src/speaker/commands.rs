// Pluely AI Speech Detection, and capture system audio (speaker output) as a stream of f32 samples.
use crate::speaker::{AudioDevice, SpeakerInput};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::StreamExt;
use hound::{WavSpec, WavWriter};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_shell::ShellExt;
use tracing::{error, warn};

use crate::audio::resampler::FrameResampler;
use crate::audio::vad::{SileroVad, VoiceActivityDetector};
use crate::transcription::engine::TranscriptionManager;
use crossbeam_channel as cb_channel;

struct LiveAudioChunk {
    samples: Vec<f32>,
    sample_rate: u32,
    is_break: bool,
    speech_duration_secs: f64,
    use_local_stt: bool,
}

const LIVE_CHUNK_SAMPLES: usize = 48000;
const MIN_CHUNK_SAMPLES: usize = 16000;
const SILENCE_FRAMES_FOR_BREAK: usize = 90;
const MIN_SPEECH_SECS_FOR_LLM: f64 = 5.0;

// VAD Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    pub hop_size: usize,
    pub sensitivity_rms: f32,
    pub peak_threshold: f32,
    pub silence_chunks: usize,
    pub min_speech_chunks: usize,
    pub pre_speech_chunks: usize,
    pub noise_gate_threshold: f32,
    pub max_recording_duration_secs: u64,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hop_size: 1024,
            sensitivity_rms: 0.025,
            peak_threshold: 0.06,
            silence_chunks: 60,
            min_speech_chunks: 10,
            pre_speech_chunks: 15,
            noise_gate_threshold: 0.008,
            max_recording_duration_secs: 180,
        }
    }
}

#[tauri::command]
pub async fn start_system_audio_capture(
    app: AppHandle,
    vad_config: Option<VadConfig>,
    device_id: Option<String>,
) -> Result<(), String> {
    tracing::info!("start_system_audio_capture called, vad_config: {:?}, device_id: {:?}", vad_config, device_id);
    let state = app.state::<crate::AudioState>();

    // Check if already capturing (atomic check)
    {
        let guard = state
            .stream_task
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;

        if guard.is_some() {
            warn!("Capture already running");
            return Err("Capture already running".to_string());
        }
    }

    // Update VAD config if provided
    if let Some(config) = vad_config {
        let mut vad_cfg = state
            .vad_config
            .lock()
            .map_err(|e| format!("Failed to acquire VAD config lock: {}", e))?;
        *vad_cfg = config;
    }

    let input = SpeakerInput::new_with_device(device_id).map_err(|e| {
        error!("Failed to create speaker input: {}", e);
        format!("Failed to access system audio: {}", e)
    })?;

    let stream = input.stream();
    let sr = stream.sample_rate();

    // Validate sample rate
    if !(8000..=96000).contains(&sr) {
        error!("Invalid sample rate: {}", sr);
        return Err(format!(
            "Invalid sample rate: {}. Expected 8000-96000 Hz",
            sr
        ));
    }

    let app_clone = app.clone();
    let vad_config = state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to read VAD config: {}", e))?
        .clone();

    let use_local_stt = state.use_local_stt.load(Ordering::Relaxed);
    let transcription_manager = state.transcription_manager.clone();

    if use_local_stt {
        let model_id = state
            .selected_local_model
            .lock()
            .map_err(|e| format!("Failed to read model selection: {}", e))?
            .clone()
            .unwrap_or_else(|| "parakeet-tdt-0.6b-v3".to_string());

        if let Err(e) = transcription_manager.ensure_model_loaded(&model_id) {
            tracing::info!("Model '{}' not loaded ({}), attempting auto-download...", model_id, e);
            let mm = state.model_manager.clone();
            let mid = model_id.clone();
            let app_dl = app.clone();
            
            let _ = app_clone.emit("live-transcription", serde_json::json!({
                "text": "Downloading transcription model (first time only)...",
                "is_final": false,
                "speech_duration_secs": 0.0,
            }));
            
            match mm.download_model(&mid, &app_dl).await {
                Ok(()) => {
                    tracing::info!("Auto-download of '{}' completed", mid);
                    if let Err(e2) = transcription_manager.ensure_model_loaded(&mid) {
                        tracing::warn!("Failed to load model after download: {}", e2);
                    }
                }
                Err(dl_err) => {
                    tracing::warn!("Auto-download failed: {}", dl_err);
                    let _ = app_clone.emit("live-transcription", serde_json::json!({
                        "text": format!("Failed to download model: {}. Go to Dev Space > Model Manager to download manually.", dl_err),
                        "is_final": true,
                        "speech_duration_secs": 0.0,
                    }));
                }
            }
        }
    }

    // Mark as capturing BEFORE spawning task
    *state
        .is_capturing
        .lock()
        .map_err(|e| format!("Failed to set capturing state: {}", e))? = true;

    // Emit capture started event
    let _ = app_clone.emit("capture-started", sr);

    let state_clone = app.state::<crate::AudioState>();
    let task = tokio::spawn(async move {
        tracing::info!("Audio capture task started, vad_enabled={}, sr={}, use_local_stt={}", vad_config.enabled, sr, use_local_stt);
        if vad_config.enabled {
            run_live_capture(
                app_clone.clone(),
                stream,
                sr,
                use_local_stt,
                transcription_manager,
            )
            .await;
        } else {
            run_continuous_capture(
                app_clone.clone(),
                stream,
                sr,
                vad_config,
                use_local_stt,
                transcription_manager,
            )
            .await;
        }

        let state = app_clone.state::<crate::AudioState>();
        {
            if let Ok(mut guard) = state.stream_task.lock() {
                *guard = None;
            };
        }
    });

    *state_clone
        .stream_task
        .lock()
        .map_err(|e| format!("Failed to store task: {}", e))? = Some(task);

    Ok(())
}

// VAD-enabled capture - OPTIMIZED for real-time speech detection
async fn run_live_capture(
    app: AppHandle,
    stream: impl StreamExt<Item = f32> + Unpin,
    sr: u32,
    use_local_stt: bool,
    transcription_manager: Arc<TranscriptionManager>,
) {
    let mut stream = stream;
    let mut resampler = FrameResampler::new(sr as usize);

    let vad_model_path = get_silero_model_path(&app);
    tracing::info!("Loading Silero VAD from: {:?}", vad_model_path);
    let mut silero = match SileroVad::new(&vad_model_path, 0.3) {
        Ok(s) => {
            tracing::info!("Silero VAD loaded successfully");
            s
        }
        Err(e) => {
            error!("Failed to load Silero VAD model: {}", e);
            return;
        }
    };

    let (chunk_tx, chunk_rx) = cb_channel::bounded::<LiveAudioChunk>(8);

    let worker_app = app.clone();
    let worker_tm = transcription_manager.clone();
    let worker_handle = std::thread::spawn(move || {
        transcription_worker(chunk_rx, &worker_app, &worker_tm);
    });

    let mut chunk_buffer: Vec<f32> = Vec::with_capacity(LIVE_CHUNK_SAMPLES + 4800);
    let mut speech_frame_count: usize = 0;
    let mut silence_frame_count: usize = 0;
    let mut sample_buf = Vec::with_capacity(1024);

    while let Some(sample) = stream.next().await {
        sample_buf.push(sample);
        if sample_buf.len() >= 1024 {
            let chunk = std::mem::replace(&mut sample_buf, Vec::with_capacity(1024));
            resampler.push(&chunk, |frame| {
                match silero.is_voice(frame) {
                    Ok(true) => {
                        silence_frame_count = 0;
                        speech_frame_count += 1;
                    }
                    Ok(false) => {
                        silence_frame_count += 1;
                    }
                    Err(_) => {}
                }
                chunk_buffer.extend_from_slice(frame);

                if silence_frame_count >= SILENCE_FRAMES_FOR_BREAK && !chunk_buffer.is_empty() {
                    let speech_duration_secs = (speech_frame_count as f64 * 0.03);
                    if chunk_buffer.len() >= MIN_CHUNK_SAMPLES {
                        let _ = chunk_tx.send(LiveAudioChunk {
                            samples: std::mem::replace(&mut chunk_buffer, Vec::with_capacity(LIVE_CHUNK_SAMPLES + 4800)),
                            sample_rate: 16000,
                            is_break: true,
                            speech_duration_secs,
                            use_local_stt,
                        });
                    } else {
                        chunk_buffer.clear();
                    }
                    speech_frame_count = 0;
                    silence_frame_count = 0;
                    return;
                }

                if chunk_buffer.len() >= LIVE_CHUNK_SAMPLES {
                    let mut split_pos = LIVE_CHUNK_SAMPLES;
                    let search_start = LIVE_CHUNK_SAMPLES.saturating_sub(1600);
                    let search_end = (LIVE_CHUNK_SAMPLES + 1600).min(chunk_buffer.len());
                    if search_start < search_end {
                        let mut best_silence = search_start;
                        let mut best_energy = f32::MAX;
                        for i in (search_start..search_end).step_by(480) {
                            let end = (i + 480).min(chunk_buffer.len());
                            if end <= i { continue; }
                            let energy: f32 = chunk_buffer[i..end].iter().map(|s| s * s).sum::<f32>() / (end - i) as f32;
                            if energy < best_energy {
                                best_energy = energy;
                                best_silence = i;
                            }
                        }
                        split_pos = best_silence;
                    }

                    let speech_duration_secs = (speech_frame_count as f64 * 0.03);
                    let chunk_samples: Vec<f32> = chunk_buffer.drain(..split_pos).collect();
                    if chunk_samples.len() >= MIN_CHUNK_SAMPLES {
                        let _ = chunk_tx.send(LiveAudioChunk {
                            samples: chunk_samples,
                            sample_rate: 16000,
                            is_break: false,
                            speech_duration_secs,
                            use_local_stt,
                        });
                    }
                    speech_frame_count = 0;
                }
            });
        }
    }

    resampler.finish(|frame| {
        match silero.is_voice(frame) {
            Ok(true) => {
                speech_frame_count += 1;
            }
            Ok(false) => {}
            Err(_) => {}
        }
        chunk_buffer.extend_from_slice(frame);
    });

    if !chunk_buffer.is_empty() && chunk_buffer.len() >= MIN_CHUNK_SAMPLES {
        let speech_duration_secs = (speech_frame_count as f64 * 0.03);
        let _ = chunk_tx.send(LiveAudioChunk {
            samples: chunk_buffer,
            sample_rate: 16000,
            is_break: true,
            speech_duration_secs,
            use_local_stt,
        });
    }

    drop(chunk_tx);
    let _ = worker_handle.join();
}

fn transcription_worker(
    rx: cb_channel::Receiver<LiveAudioChunk>,
    app: &AppHandle,
    transcription_manager: &TranscriptionManager,
) {
    while let Ok(chunk) = rx.recv() {
        let normalized = normalize_audio_level(&chunk.samples, 0.1);

        if chunk.use_local_stt {
            let text = match transcription_manager.transcribe(normalized.clone()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("Local transcription failed, falling back to remote: {}", e);
                    match samples_to_wav_b64(chunk.sample_rate, &normalized) {
                        Ok(b64) => {
                            let _ = app.emit("speech-detected", b64);
                        }
                        Err(enc_err) => {
                            tracing::warn!("Failed to encode audio: {}", enc_err);
                        }
                    }
                    continue;
                }
            };

            if text.trim().is_empty() {
                continue;
            }

            tracing::info!("Live transcription: {:?} (is_break={}, speech={:.1}s)", text, chunk.is_break, chunk.speech_duration_secs);

            let _ = app.emit("live-transcription", serde_json::json!({
                "text": text,
                "is_final": chunk.is_break,
                "speech_duration_secs": chunk.speech_duration_secs,
            }));
        } else {
            match samples_to_wav_b64(chunk.sample_rate, &normalized) {
                Ok(b64) => {
                    let _ = app.emit("speech-detected", b64);
                }
                Err(e) => {
                    tracing::warn!("Failed to encode audio: {}", e);
                }
            }
        }
    }
}

// Continuous capture (VAD disabled)
async fn run_continuous_capture(
    app: AppHandle,
    stream: impl StreamExt<Item = f32> + Unpin,
    sr: u32,
    config: VadConfig,
    use_local_stt: bool,
    transcription_manager: Arc<TranscriptionManager>,
) {
    let mut stream = stream;
    let max_samples = (sr as u64 * config.max_recording_duration_secs) as usize;

    // Pre-allocate buffer to prevent reallocations
    let mut audio_buffer = Vec::with_capacity(max_samples);
    let start_time = Instant::now();
    let max_duration = Duration::from_secs(config.max_recording_duration_secs);

    // Atomic flag for manual stop
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_for_listener = stop_flag.clone();

    // Listen for manual stop event
    let stop_listener = app.listen("manual-stop-continuous", move |_| {
        stop_flag_for_listener.store(true, Ordering::Release);
    });

    // Emit recording started
    let _ = app.emit(
        "continuous-recording-start",
        config.max_recording_duration_secs,
    );

    // Accumulate audio - check stop flag on EVERY sample for immediate response
    loop {
        // Check stop flag FIRST on every iteration for immediate stopping
        if stop_flag.load(Ordering::Acquire) {
            break;
        }

        tokio::select! {
            sample_opt = stream.next() => {
                match sample_opt {
                    Some(sample) => {
                        if stop_flag.load(Ordering::Acquire) {
                            break;
                        }

                        audio_buffer.push(sample);

                        let elapsed = start_time.elapsed();

                        // Emit progress every second
                        if audio_buffer.len() % (sr as usize) == 0 {
                            let _ = app.emit("recording-progress", elapsed.as_secs());
                        }

                        // Check size limit (safety)
                        if audio_buffer.len() >= max_samples {
                            break;
                        }

                        // Check time limit
                        if elapsed >= max_duration {
                            break;
                        }
                    },
                    None => {
                        warn!("Audio stream ended unexpectedly");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
            }
        }
    }

    // Clean up event listener (CRITICAL)
    app.unlisten(stop_listener);

    // Process and emit audio
    if !audio_buffer.is_empty() {
        let cleaned_audio = apply_noise_gate(&audio_buffer, config.noise_gate_threshold);
        let cleaned_audio = normalize_audio_level(&cleaned_audio, 0.1);

        if use_local_stt {
            match transcription_manager.transcribe(cleaned_audio.clone()) {
                Ok(text) => {
                    let _ = app.emit("speech-transcribed", serde_json::json!({ "text": text }));
                }
                Err(e) => {
                    error!("Local transcription failed: {}", e);
                    match samples_to_wav_b64(sr, &cleaned_audio) {
                        Ok(b64) => {
                            let _ = app.emit("speech-detected", b64);
                        }
                        Err(enc_err) => {
                            error!("Failed to encode continuous audio: {}", enc_err);
                            let _ = app.emit("audio-encoding-error", enc_err);
                        }
                    }
                }
            }
        } else {
            match samples_to_wav_b64(sr, &cleaned_audio) {
                Ok(b64) => {
                    let _ = app.emit("speech-detected", b64);
                }
                Err(e) => {
                    error!("Failed to encode continuous audio: {}", e);
                    let _ = app.emit("audio-encoding-error", e);
                }
            }
        }
    } else {
        warn!("No audio captured in continuous mode");
        let _ = app.emit("audio-encoding-error", "No audio recorded");
    }

    let _ = app.emit("continuous-recording-stopped", ());
}

// Apply noise gate
fn apply_noise_gate(samples: &[f32], threshold: f32) -> Vec<f32> {
    const KNEE_RATIO: f32 = 3.0; // Compression ratio for soft knee

    samples
        .iter()
        .map(|&s| {
            let abs = s.abs();
            if abs < threshold {
                s * (abs / threshold).powf(1.0 / KNEE_RATIO)
            } else {
                s
            }
        })
        .collect()
}

// Calculate RMS and peak (optimized)
fn calculate_audio_metrics(chunk: &[f32]) -> (f32, f32) {
    let mut sumsq = 0.0f32;
    let mut peak = 0.0f32;

    for &v in chunk {
        let a = v.abs();
        peak = peak.max(a);
        sumsq += v * v;
    }

    let rms = (sumsq / chunk.len() as f32).sqrt();
    (rms, peak)
}

fn normalize_audio_level(samples: &[f32], target_rms: f32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
    let current_rms = (sum_squares / samples.len() as f32).sqrt();

    if current_rms < 0.001 {
        return samples.to_vec();
    }

    let gain = (target_rms / current_rms).min(10.0);

    samples
        .iter()
        .map(|&s| {
            let amplified = s * gain;
            if amplified.abs() > 1.0 {
                amplified.signum() * (1.0 - (-amplified.abs()).exp())
            } else {
                amplified
            }
        })
        .collect()
}

// Convert samples to WAV base64 (with proper error handling)
fn samples_to_wav_b64(sample_rate: u32, mono_f32: &[f32]) -> Result<String, String> {
    // Validate sample rate
    if !(8000..=96000).contains(&sample_rate) {
        error!("Invalid sample rate: {}", sample_rate);
        return Err(format!(
            "Invalid sample rate: {}. Expected 8000-96000 Hz",
            sample_rate
        ));
    }

    // Validate buffer
    if mono_f32.is_empty() {
        return Err("Empty audio buffer".to_string());
    }

    let mut cursor = Cursor::new(Vec::new());
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::new(&mut cursor, spec).map_err(|e| {
        error!("Failed to create WAV writer: {}", e);
        e.to_string()
    })?;

    for &s in mono_f32 {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16).map_err(|e| e.to_string())?;
    }

    writer.finalize().map_err(|e| e.to_string())?;

    Ok(B64.encode(cursor.into_inner()))
}

fn get_silero_model_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .resource_dir()
        .map(|d| {
            d.join("resources")
                .join("models")
                .join("silero_vad_v4.onnx")
        })
        .unwrap_or_else(|_| {
            std::path::PathBuf::from("resources/models/silero_vad_v4.onnx")
        })
}

#[tauri::command]
pub async fn stop_system_audio_capture(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();

    // Abort task in separate scope (Send trait fix)
    {
        let mut guard = state
            .stream_task
            .lock()
            .map_err(|e| format!("Failed to acquire task lock: {}", e))?;

        if let Some(task) = guard.take() {
            task.abort();
        }
    }

    // LONGER delay for proper cleanup (300ms instead of 150ms)
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Mark as not capturing
    *state
        .is_capturing
        .lock()
        .map_err(|e| format!("Failed to update capturing state: {}", e))? = false;

    // Additional cleanup delay (CRITICAL for mic indicator)
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Emit stopped event
    let _ = app.emit("capture-stopped", ());
    Ok(())
}

/// Manual stop for continuous recording
#[tauri::command]
pub async fn manual_stop_continuous(app: AppHandle) -> Result<(), String> {
    let _ = app.emit("manual-stop-continuous", ());

    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

    Ok(())
}

#[tauri::command]
pub fn check_system_audio_access(_app: AppHandle) -> Result<bool, String> {
    match SpeakerInput::new() {
        Ok(_) => Ok(true),
        Err(e) => {
            error!("System audio access check failed: {}", e);
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn request_system_audio_access(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        app.shell()
            .command("open")
            .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_AudioCapture"])
            .spawn()
            .map_err(|e| {
                error!("Failed to open system preferences: {}", e);
                e.to_string()
            })?;
    }
    #[cfg(target_os = "windows")]
    {
        app.shell()
            .command("ms-settings:sound")
            .spawn()
            .map_err(|e| {
                error!("Failed to open sound settings: {}", e);
                e.to_string()
            })?;
    }
    #[cfg(target_os = "linux")]
    {
        let commands = ["pavucontrol", "gnome-control-center sound"];
        let mut opened = false;

        for cmd in &commands {
            if app.shell().command(cmd).spawn().is_ok() {
                opened = true;
                break;
            }
        }

        if !opened {
            warn!("Failed to open audio settings on Linux");
        }
    }

    Ok(())
}

// VAD Configuration Management
#[tauri::command]
pub async fn get_vad_config(app: AppHandle) -> Result<VadConfig, String> {
    let state = app.state::<crate::AudioState>();
    let config = state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to get VAD config: {}", e))?
        .clone();
    Ok(config)
}

#[tauri::command]
pub async fn update_vad_config(app: AppHandle, config: VadConfig) -> Result<(), String> {
    // Validate config
    if config.sensitivity_rms < 0.0 || config.sensitivity_rms > 1.0 {
        return Err("Invalid sensitivity_rms: must be 0.0-1.0".to_string());
    }
    if config.max_recording_duration_secs > 3600 {
        return Err("Invalid max_recording_duration_secs: must be <= 3600 (1 hour)".to_string());
    }

    let state = app.state::<crate::AudioState>();
    *state
        .vad_config
        .lock()
        .map_err(|e| format!("Failed to update VAD config: {}", e))? = config;

    Ok(())
}

#[tauri::command]
pub async fn get_capture_status(app: AppHandle) -> Result<bool, String> {
    let state = app.state::<crate::AudioState>();
    let is_capturing = *state
        .is_capturing
        .lock()
        .map_err(|e| format!("Failed to get capture status: {}", e))?;
    Ok(is_capturing)
}

#[tauri::command]
pub fn get_audio_sample_rate(_app: AppHandle) -> Result<u32, String> {
    let input = SpeakerInput::new().map_err(|e| {
        error!("Failed to create speaker input: {}", e);
        format!("Failed to access system audio: {}", e)
    })?;

    let stream = input.stream();
    let sr = stream.sample_rate();

    Ok(sr)
}

#[tauri::command]
pub fn get_input_devices() -> Result<Vec<AudioDevice>, String> {
    crate::speaker::list_input_devices().map_err(|e| {
        error!("Failed to get input devices: {}", e);
        format!("Failed to get input devices: {}", e)
    })
}

#[tauri::command]
pub fn get_output_devices() -> Result<Vec<AudioDevice>, String> {
    crate::speaker::list_output_devices().map_err(|e| {
        error!("Failed to get output devices: {}", e);
        format!("Failed to get output devices: {}", e)
    })
}

#[tauri::command]
pub async fn set_stt_mode(app: AppHandle, use_local: bool) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();
    state
        .use_local_stt
        .store(use_local, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn set_local_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let state = app.state::<crate::AudioState>();
    let mut guard = state
        .selected_local_model
        .lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?;
    *guard = Some(model_id);
    Ok(())
}

#[tauri::command]
pub async fn transcribe_local(
    app: AppHandle,
    audio_base64: String,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

    let state = app.state::<crate::AudioState>();
    let model_id = state
        .selected_local_model
        .lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?
        .clone()
        .unwrap_or_else(|| "parakeet-tdt-0.6b-v3".to_string());

    state
        .transcription_manager
        .ensure_model_loaded(&model_id)
        .map_err(|e| format!("Failed to load model '{}': {}", model_id, e))?;

    let audio_bytes = B64
        .decode(&audio_base64)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    let cursor = std::io::Cursor::new(audio_bytes);
    let mut reader = hound::WavReader::new(cursor)
        .map_err(|e| format!("Failed to parse WAV: {}", e))?;
    let spec = reader.spec();

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / i16::MAX as f32)
        .collect();

    let resampled = if spec.sample_rate != 16000 {
        let mut resampler = crate::audio::resampler::FrameResampler::new(
            spec.sample_rate as usize,
        );
        let mut output = Vec::new();
        resampler.push(&samples, |frame| {
            output.extend_from_slice(frame);
        });
        resampler.finish(|frame| {
            output.extend_from_slice(frame);
        });
        output
    } else {
        samples
    };

    state
        .transcription_manager
        .transcribe(resampled)
        .map_err(|e| format!("Transcription failed: {}", e))
}

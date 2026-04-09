# In-Process Speech-to-Text Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace HTTP-based STT with in-process transcription (like Handy), while keeping system audio capture and HTTP STT as fallback.

**Architecture:** Add `transcribe-rs` crate to the Rust backend for local Whisper/Parakeet inference. Replace the RMS/peak VAD with Silero VAD v4 (neural network). Audio pipeline: system audio capture → rubato resampler (native→16kHz) → Silero VAD → local model → text event. Dual-path: local STT (text event) and remote STT (WAV base64 → frontend → HTTP).

**Tech Stack:** Rust (transcribe-rs, rubato, vad-rs, ONNX Runtime, whisper.cpp), Tauri v2, React/TypeScript frontend.

---

## Architecture Overview

```
System Audio (CoreAudio/WASAPI/PulseAudio) → f32 @ native rate (44.1/48kHz)
    │
    ▼
FrameResampler (rubato: native → 16kHz, 30ms frames of 480 samples)
    │
    ▼
Silero VAD v4 (neural, replaces RMS/peak threshold detection)
    │  speech segments: Vec<f32> @ 16kHz
    ├─────────────────────────────────────┐
    ▼ [local STT selected]                ▼ [remote STT selected]
TranscriptionManager (transcribe-rs)      WAV → base64 → "speech-detected" event
    │  text                                   │
    ▼                                         ▼
"speech-transcribed" event               Frontend → fetchSTT() → HTTP
    │                                         │
    └─────────────────────────────────────────┘
                    ▼
          processWithAI() → AI provider
```

## Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| STT crate | `transcribe-rs` 0.3.x | Same as Handy. Unified API for Whisper + Parakeet + others. Platform GPU accel built-in. |
| Models | All 14+ Handy models | Parakeet V3 (default), Whisper Small/Medium/Turbo/Large, Moonshine, SenseVoice, GigaAM, Canary, Cohere |
| VAD | Silero v4 (ONNX) bundled | Neural VAD, far superior to RMS/peak. ~2MB, bundled in app. |
| Resampler | rubato (FFT-based) | System audio is 44.1/48kHz, models need 16kHz. |
| Model storage | `{app_data}/models/` | Same pattern as Handy. Download with SHA256 verify. Resume support. |
| VAD scope | Both local and remote STT | Silero replaces RMS/peak for all audio capture paths. |
| Remote STT | Keep as fallback | Users who prefer cloud STT keep it working unchanged. |

## File Structure

**New files:**
```
src-tauri/src/
├── audio/                              # NEW module
│   ├── mod.rs                          # Module declarations
│   ├── resampler.rs                    # FrameResampler (rubato wrapper)
│   └── vad.rs                          # SileroVad + SmoothedVad
├── transcription/                      # NEW module
│   ├── mod.rs                          # Module declarations
│   ├── model_manager.rs                # Model download/cache/verify
│   └── engine.rs                       # TranscriptionManager (load/infer/unload)
└── resources/
    └── models/
        └── silero_vad_v4.onnx          # Bundled Silero model (~2MB)
```

**Modified files:**
```
src-tauri/Cargo.toml                    # Add dependencies
src-tauri/tauri.conf.json               # Add silero model as resource
src-tauri/src/lib.rs                    # Register new modules + commands + state
src-tauri/src/speaker/commands.rs       # Upgrade VAD, add local transcription path
src/config/stt.constants.ts             # Add "in-process" built-in provider
src/hooks/useSystemAudio.ts             # Handle "speech-transcribed" event
src/contexts/app.context.tsx            # Add local model selection state
src/pages/dev/components/model-manager/ # Model management UI (new components)
```

---

## Reference: Handy Implementation Details

### transcribe-rs usage (from Handy)

Platform-specific feature flags:
- macOS: `["whisper-metal"]` (Metal GPU via whisper.cpp)
- Windows: `["whisper-vulkan", "ort-directml"]` (Vulkan + DirectML)
- Linux: `["whisper-vulkan"]` (Vulkan)
- Default build: `["whisper-cpp", "onnx"]`

Model loading:
```rust
// Whisper (GGML binary file)
let engine = WhisperEngine::load(&model_path)?;
// Parakeet (ONNX directory, Int8 quantization)
let engine = ParakeetModel::load(&model_path, &Quantization::Int8)?;
// Moonshine
let engine = MoonshineModel::load(&model_path, MoonshineVariant::Base, &Quantization::default())?;
// Moonshine Streaming
let engine = StreamingModel::load(&model_path, 0, &Quantization::default())?;
// SenseVoice
let engine = SenseVoiceModel::load(&model_path, &Quantization::Int8)?;
// GigaAM
let engine = GigaAMModel::load(&model_path, &Quantization::Int8)?;
// Canary
let engine = CanaryModel::load(&model_path, &Quantization::Int8)?;
// Cohere
let engine = CohereModel::load(&model_path, &Quantization::Int8)?;
```

Transcription call:
```rust
pub fn transcribe(&self, audio: Vec<f32>) -> Result<String>  // audio is f32 @ 16kHz
```

Panic safety pattern:
```rust
let mut engine = engine_guard.take();  // Take ownership
drop(engine_guard);                     // Release mutex before engine call
let result = catch_unwind(AssertUnwindSafe(|| engine.transcribe(&audio)));
// Put engine back on success, drop on panic
```

### Handy's full model catalog (14+ models)

| ID | Engine | Size | Format | URL |
|---|---|---|---|---|
| `small` | Whisper | 465 MB | file (ggml-small.bin) | `https://blob.handy.computer/ggml-small.bin` |
| `medium` | Whisper | 469 MB | file (whisper-medium-q4_1.bin) | `https://blob.handy.computer/whisper-medium-q4_1.bin` |
| `turbo` | Whisper | 1549 MB | file (ggml-large-v3-turbo.bin) | `https://blob.handy.computer/ggml-large-v3-turbo.bin` |
| `large` | Whisper | 1031 MB | file (ggml-large-v3-q5_0.bin) | `https://blob.handy.computer/ggml-large-v3-q5_0.bin` |
| `breeze-asr` | Whisper | 1030 MB | file | `https://blob.handy.computer/breeze-asr.bin` |
| `parakeet-tdt-0.6b-v2` | Parakeet | 451 MB | dir | `https://blob.handy.computer/parakeet-v2-int8.tar.gz` |
| `parakeet-tdt-0.6b-v3` | Parakeet | 456 MB | dir | `https://blob.handy.computer/parakeet-v3-int8.tar.gz` |
| `moonshine-base` | Moonshine | 55 MB | dir | Handy's blob |
| `moonshine-tiny-streaming-en` | MoonshineStreaming | 31 MB | dir | Handy's blob |
| `moonshine-small-streaming-en` | MoonshineStreaming | 99 MB | dir | Handy's blob |
| `moonshine-medium-streaming-en` | MoonshineStreaming | 192 MB | dir | Handy's blob |
| `sense-voice-int8` | SenseVoice | 152 MB | dir | Handy's blob |
| `gigaam-v3-e2e-ctc` | GigaAM | 151 MB | dir | Handy's blob |
| `canary-180m-flash` | Canary | 146 MB | dir | Handy's blob |
| `canary-1b-v2` | Canary | 691 MB | dir | Handy's blob |
| `cohere-int8` | Cohere | 1708 MB | dir | Handy's blob |

### Silero VAD implementation (from Handy)

```rust
// SileroVad: thin wrapper around vad-rs
const SILERO_FRAME_MS: u32 = 30;
const SILERO_FRAME_SAMPLES: usize = (16000 * 30 / 1000); // = 480

pub struct SileroVad {
    engine: vad_rs::Vad,
    threshold: f32,  // 0.3
}

// SmoothedVad: temporal smoothing wrapper
pub struct SmoothedVad {
    inner: Box<dyn VoiceActivityDetector>,
    prefill_frames: usize,    // 15 (450ms lookback)
    hangover_frames: usize,   // 15 (450ms tail)
    onset_frames: usize,      // 2  (60ms to trigger)
    frame_buffer: VecDeque<Vec<f32>>,
    // ...
}
```

State machine:
- `(silence -> speech)`: increment onset_counter. When >= onset_frames, emit prefill + current frame
- `(speech -> speech)`: reset hangover, emit current frame
- `(speech -> silence)`: decrement hangover. When 0, mark silence
- `(silence -> silence)`: reset onset counter

### FrameResampler (from Handy)

```rust
pub struct FrameResampler {
    resampler: Option<FftFixedIn<f32>>,  // None if native rate == 16kHz
    chunk_in: usize,                      // 1024
    in_buf: Vec<f32>,
    frame_samples: usize,                 // 480 (= 16000 * 0.030)
    pending: Vec<f32>,
}
```

### Pirately's current pipeline (to be replaced)

```
[System Audio Capture] f32 @ native rate
    -> buffer into 1024-sample chunks
    -> RMS/peak energy VAD
    -> collect speech
    -> normalize -> WAV -> base64
    -> emit "speech-detected" event
    -> frontend: base64 -> Blob -> fetchSTT() -> HTTP
    -> frontend: text -> processWithAI()
```

Key files to modify:
- `src-tauri/src/speaker/commands.rs` — `run_vad_capture()` (lines 135-257) and `run_continuous_capture()` (lines 260-360)
- `src-tauri/src/lib.rs` — `AudioState` struct (lines 20-25), `invoke_handler` (lines 74-118), managed state (line 42)
- `src/hooks/useSystemAudio.ts` — `speech-detected` listener (lines 220-305)

---

## Tasks

### Task 1: Add Rust Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add to `[dependencies]`:
```toml
transcribe-rs = { version = "0.3.8", features = ["whisper-cpp", "onnx"] }
rubato = "0.16.2"
vad-rs = { git = "https://github.com/cjpais/vad-rs", default-features = false }
sha2 = "0.10"
tar = "0.4"
flate2 = "1.0"
```

Add platform-specific overrides (these replace the default `transcribe-rs`):
```toml
[target.'cfg(target_os = "macos")'.dependencies]
transcribe-rs = { version = "0.3.8", features = ["whisper-metal"] }

[target.'cfg(target_os = "windows")'.dependencies]
transcribe-rs = { version = "0.3.8", features = ["whisper-vulkan", "ort-directml"] }

[target.'cfg(target_os = "linux")'.dependencies]
transcribe-rs = { version = "0.3.8", features = ["whisper-vulkan"] }
```

Note: We already have `hound` (3.5.1), `reqwest` (0.12), `futures-util` (0.3), `tokio` (full), `anyhow` (1.0), `serde`/`serde_json`, `base64` (0.22), `ringbuf` (0.4.8).

- [ ] **Step 2: Run `cargo check` to verify compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles successfully (may take a while for first build)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add transcribe-rs, rubato, vad-rs dependencies for in-process STT"
```

---

### Task 2: Create Audio Resampler Module

**Files:**
- Create: `src-tauri/src/audio/mod.rs`
- Create: `src-tauri/src/audio/resampler.rs`

This task is independent of Tasks 3, 4, 5 and can be done in parallel.

- [ ] **Step 1: Create `src-tauri/src/audio/mod.rs`**

```rust
pub mod resampler;
pub mod vad;
```

- [ ] **Step 2: Create `src-tauri/src/audio/resampler.rs`**

Port Handy's `FrameResampler` from their `audio_toolkit/audio/resampler.rs`. Key implementation:

```rust
use rubato::{FftFixedIn, Resampler};
use std::time::Duration;

const RESAMPLER_CHUNK_SIZE: usize = 1024;

pub struct FrameResampler {
    resampler: Option<FftFixedIn<f32>>,
    chunk_in: usize,
    in_buf: Vec<f32>,
    frame_samples: usize,
    pending: Vec<f32>,
}

impl FrameResampler {
    pub fn new(in_hz: usize, out_hz: usize, frame_dur: Duration) -> Self {
        let frame_samples = ((out_hz as f64 * frame_dur.as_secs_f64()).round()) as usize;
        let resampler = (in_hz != out_hz).then(|| {
            FftFixedIn::<f32>::new(in_hz, out_hz, RESAMPLER_CHUNK_SIZE, 1, 1)
                .expect("Failed to create resampler")
        });

        Self {
            resampler,
            chunk_in: RESAMPLER_CHUNK_SIZE,
            in_buf: Vec::new(),
            frame_samples,
            pending: Vec::new(),
        }
    }

    pub fn push(&mut self, src: &[f32], mut emit: impl FnMut(&[f32])) {
        self.in_buf.extend_from_slice(src);

        if let Some(resampler) = &mut self.resampler {
            while self.in_buf.len() >= self.chunk_in {
                let chunk: Vec<f32> = self.in_buf.drain(..self.chunk_in).collect();
                if let Ok(waves) = resampler.process(&[&chunk], None) {
                    for wave in waves {
                        self.pending.extend_from_slice(&wave);
                    }
                }
            }
        } else {
            self.pending.extend(self.in_buf.drain(..));
        }

        while self.pending.len() >= self.frame_samples {
            let frame: Vec<f32> = self.pending.drain(..self.frame_samples).collect();
            emit(&frame);
        }
    }

    pub fn finish(&mut self, mut emit: impl FnMut(&[f32])) {
        if let Some(resampler) = &mut self.resampler {
            if !self.in_buf.is_empty() {
                let needed = resampler.input_frames_next();
                let mut input = self.in_buf.clone();
                if input.len() < needed {
                    input.resize(needed, 0.0);
                }
                if let Ok(waves) = resampler.process(&[&input], None) {
                    for wave in waves {
                        self.pending.extend_from_slice(&wave);
                    }
                }
            }
        }

        while self.pending.len() >= self.frame_samples {
            let frame: Vec<f32> = self.pending.drain(..self.frame_samples).collect();
            emit(&frame);
        }

        if !self.pending.is_empty() {
            emit(&self.pending);
            self.pending.clear();
        }
    }
}
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS (may need to create vad.rs stub or comment out `pub mod vad;` temporarily)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/
git commit -m "feat: add audio resampler module with rubato FFT resampling"
```

---

### Task 3: Create Silero VAD Module

**Files:**
- Create: `src-tauri/src/audio/vad.rs`
- Bundle: `src-tauri/resources/models/silero_vad_v4.onnx`
- Modify: `src-tauri/tauri.conf.json` (add resource)

This task is independent of Tasks 2, 4, 5 and can be done in parallel.

- [ ] **Step 1: Download and bundle Silero VAD model**

Download the Silero VAD v4 ONNX model (~2MB) and place at `src-tauri/resources/models/silero_vad_v4.onnx`.

Add to `src-tauri/tauri.conf.json` under `bundle.resources`:
```json
"resources": [
    "resources/models/*"
]
```

- [ ] **Step 2: Create `src-tauri/src/audio/vad.rs`**

Port Handy's VAD system from `audio_toolkit/vad/`. Key structs:

```rust
use anyhow::{Result, Context};
use std::collections::VecDeque;
use std::path::Path;

pub enum VadFrame<'a> {
    Speech(&'a [f32]),
    Noise,
}

pub trait VoiceActivityDetector: Send + Sync {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>>;
    fn is_voice(&mut self, frame: &[f32]) -> Result<bool> {
        Ok(self.push_frame(frame)?.is_speech())
    }
    fn reset(&mut self) {}
}

pub struct SileroVad {
    engine: vad_rs::Vad,
    threshold: f32,
}

impl SileroVad {
    pub fn new<P: AsRef<Path>>(model_path: P, threshold: f32) -> Result<Self> {
        let engine = vad_rs::Vad::new(
            model_path.as_ref().to_str().context("invalid path")?,
            16000
        ).context("Failed to load Silero VAD model")?;
        Ok(Self { engine, threshold })
    }
}

impl VoiceActivityDetector for SileroVad {
    fn push_frame<'a>(&'a mut self, frame: &'a [f32]) -> Result<VadFrame<'a>> {
        let result = self.engine.compute(frame)?;
        if result.prob > self.threshold {
            Ok(VadFrame::Speech(frame))
        } else {
            Ok(VadFrame::Noise)
        }
    }
}

pub struct SmoothedVad {
    inner: Box<dyn VoiceActivityDetector>,
    prefill_frames: usize,
    hangover_frames: usize,
    onset_frames: usize,
    frame_buffer: VecDeque<Vec<f32>>,
    hangover_counter: usize,
    onset_counter: usize,
    in_speech: bool,
    output_buffer: Vec<f32>,
}
```

The SmoothedVad state machine handles onset detection (requires 2 consecutive voice frames), prefill buffering (15 frames = 450ms context before speech), and hangover (15 frames = 450ms trailing after speech ends).

- [ ] **Step 3: Run `cargo check`**

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/audio/vad.rs src-tauri/resources/
git commit -m "feat: add Silero VAD v4 module with smoothed detection"
```

---

### Task 4: Create Model Manager

**Files:**
- Create: `src-tauri/src/transcription/mod.rs`
- Create: `src-tauri/src/transcription/model_manager.rs`

This task is independent of Tasks 2, 3 and can be done in parallel. Depends on Task 1 (dependencies).

- [ ] **Step 1: Create `src-tauri/src/transcription/mod.rs`**

```rust
pub mod model_manager;
pub mod engine;
```

- [ ] **Step 2: Create `src-tauri/src/transcription/model_manager.rs`**

Port Handy's ModelManager with all 14+ models. Key responsibilities:
- Define all built-in models with download URLs, sizes, SHA256 hashes, engine types
- Download models with progress reporting (via Tauri events), HTTP Range header resume
- SHA256 verification after download
- Extract tar.gz for directory-based models (Parakeet, Moonshine, etc.)
- Cache in `{app_data}/models/`
- Custom model discovery (scan for .bin files)

Tauri commands to expose:
- `list_available_models` -> `Vec<ModelInfo>`
- `download_model(model_id: String)` -> progress via `"model-download-progress"` events
- `delete_model(model_id: String)`
- `get_selected_model()` / `set_selected_model(model_id: String)`

Full model catalog with URLs from Handy's blob storage (see Reference section above for all 14+ models).

- [ ] **Step 3: Run `cargo check`**

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/transcription/
git commit -m "feat: add model manager with download/cache/verify for STT models"
```

---

### Task 5: Create Transcription Engine

**Files:**
- Create: `src-tauri/src/transcription/engine.rs`

Depends on Task 4 (model manager).

- [ ] **Step 1: Create `src-tauri/src/transcription/engine.rs`**

Port Handy's TranscriptionManager. Key features:
- Lazy model loading (load on first transcription request)
- Loading guard with Condvar (prevent concurrent loads)
- Panic recovery (take engine out of mutex, catch_unwind, put back on success)
- Idle unloading (background thread checks activity timestamp)
- Support all engine types via LoadedEngine enum

```rust
enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
    Moonshine(MoonshineModel),
    MoonshineStreaming(StreamingModel),
    SenseVoice(SenseVoiceModel),
    GigaAM(GigaAMModel),
    Canary(CanaryModel),
    Cohere(CohereModel),
}

pub struct TranscriptionManager {
    engine: Arc<Mutex<Option<LoadedEngine>>>,
    model_manager: Arc<ModelManager>,
    current_model_id: Arc<Mutex<Option<String>>>,
    last_activity: Arc<AtomicU64>,
    is_loading: Arc<Mutex<bool>>,
    loading_condvar: Arc<Condvar>,
}
```

- [ ] **Step 2: Run `cargo check`**

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/transcription/engine.rs
git commit -m "feat: add transcription engine with lazy loading and panic recovery"
```

---

### Task 6: Wire Up In Audio Pipeline

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/speaker/commands.rs`

This is the core integration task. Depends on Tasks 2, 3, 4, 5.

- [ ] **Step 1: Add modules and state to `src-tauri/src/lib.rs`**

- Add `mod audio;` and `mod transcription;`
- Update `AudioState` to include `use_local_stt: Arc<AtomicBool>` and `transcription_manager: Arc<TranscriptionManager>`
- Change `AudioState::default()` to `AudioState::new(app_handle)` (needs AppHandle for model manager init)
- Register new Tauri commands in `invoke_handler`
- Add managed state for ModelManager and TranscriptionManager

- [ ] **Step 2: Rewrite `run_vad_capture()` in `commands.rs`**

Replace RMS/peak VAD with:
1. `FrameResampler` (native rate -> 16kHz, 30ms frames)
2. `SmoothedVad` (Silero neural VAD with onset/prefill/hangover)
3. Keep same speech state machine structure (collect speech, detect silence, emit)
4. Add dual-path emission:
   - Local STT: `TranscriptionManager::transcribe(speech_buffer)` -> `"speech-transcribed"` event
   - Remote STT: `samples_to_wav_b64()` -> `"speech-detected"` event (as now)

Also update `run_continuous_capture()` to use the resampler for the local STT path.

- [ ] **Step 3: Add `set_stt_mode` Tauri command**

```rust
#[tauri::command]
pub async fn set_stt_mode(app: AppHandle, use_local: bool) -> Result<(), String>
```

- [ ] **Step 4: Update `start_system_audio_capture` to pass transcription state**

- [ ] **Step 5: Run `cargo check`**

- [ ] **Step 6: Test full pipeline on macOS**

Run: `npm run tauri dev`
Test: Start system audio capture -> play audio -> verify "speech-transcribed" events

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/
git commit -m "feat: wire up in-process transcription with Silero VAD in audio pipeline"
```

---

### Task 7: Frontend — In-Process STT Provider

**Files:**
- Modify: `src/config/stt.constants.ts`
- Modify: `src/hooks/useSystemAudio.ts`
- Modify: `src/contexts/app.context.tsx`

Depends on Task 6 (Rust backend changes).

- [ ] **Step 1: Add "in-process" provider to `src/config/stt.constants.ts`**

```typescript
{
    id: "in-process",
    name: "In-Process (Local Model)",
    curl: "",
    responseContentPath: "text",
    streaming: false,
}
```

- [ ] **Step 2: Update `src/hooks/useSystemAudio.ts`**

Add listener for `"speech-transcribed"` when in-process provider is selected. When selected, receive text directly and call `processWithAI()` — skip `fetchSTT()` entirely.

- [ ] **Step 3: Update `src/contexts/app.context.tsx`**

Add local model selection state and model listing functions.

- [ ] **Step 4: Test end-to-end**

Run: `npm run tauri dev`
Test: Select "In-Process" provider -> start capture -> play audio -> see transcription -> see AI response

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat: add in-process STT provider to frontend with local model support"
```

---

### Task 8: Frontend — Model Management UI

**Files:**
- Create: `src/pages/dev/components/model-manager/index.tsx`
- Create: `src/pages/dev/components/model-manager/ModelList.tsx`
- Create: `src/pages/dev/components/model-manager/ModelCard.tsx`
- Modify: `src/pages/dev/` layout to include model manager section

Depends on Task 7.

- [ ] **Step 1: Create model card component**

Each model shows: name, engine type, size, download/progress bar, select/delete buttons.

- [ ] **Step 2: Create model list component**

Grid of all available models from `list_available_models` Tauri command.

- [ ] **Step 3: Integrate into dev-space layout**

Add as new section/tab in the dev-space page alongside AI/STT provider configs.

- [ ] **Step 4: Test: browse models -> download -> select -> verify in capture**

- [ ] **Step 5: Commit**

```bash
git add src/pages/dev/components/model-manager/
git commit -m "feat: add model management UI with download progress and model selection"
```

---

### Task 9: Cleanup & Polish

**Files:**
- Modify: `src/config/stt.constants.ts` (remove broken providers)
- Modify: `AGENTS.md` (update documentation)

- [ ] **Step 1: Remove broken STT providers**

Remove `speechmatics-stt` and `rev-ai-stt` from `stt.constants.ts`.

- [ ] **Step 2: Fix local-whisper provider endpoint**

Update to standard OpenAI-compatible endpoint `/v1/audio/transcriptions`.

- [ ] **Step 3: Update AGENTS.md**

Document new in-process STT system, Silero VAD, model management, updated file structure.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete in-process STT with model management, cleanup broken providers"
```

---

## Risk Considerations

| Risk | Mitigation |
|---|---|
| `transcribe-rs` compilation issues on some platforms | Platform-specific Cargo.toml features; test macOS first |
| Large model downloads fail | Resume support via HTTP Range headers |
| ONNX Runtime conflicts with existing deps | transcribe-rs bundles its own ONNX Runtime |
| Silero model not available at runtime | Bundled as Tauri resource |
| Memory pressure from loaded models | Idle unloading with configurable timeout |
| System audio at 44.1/48kHz vs 16kHz expectation | rubato resampler handles transparently |
| `vad-rs` API differences from Handy's fork | Use same cjpais fork |
| `transcribe-rs` API may differ between 0.3.3 and 0.3.8 | Verify exact API before implementation |

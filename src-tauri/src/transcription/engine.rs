use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{anyhow, Result};

use super::model_manager::{EngineType, ModelManager};
use transcribe_rs::onnx::canary::CanaryModel;
use transcribe_rs::onnx::cohere::CohereModel;
use transcribe_rs::onnx::gigaam::GigaAMModel;
use transcribe_rs::onnx::moonshine::{MoonshineModel, MoonshineVariant};
use transcribe_rs::onnx::parakeet::ParakeetModel;
use transcribe_rs::onnx::sense_voice::SenseVoiceModel;
use transcribe_rs::onnx::Quantization;
use transcribe_rs::whisper_cpp::WhisperEngine;
use transcribe_rs::SpeechModel;

enum LoadedEngine {
    Whisper(WhisperEngine),
    Parakeet(ParakeetModel),
    Moonshine(MoonshineModel),
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

impl TranscriptionManager {
    pub fn new(model_manager: Arc<ModelManager>) -> Self {
        Self {
            engine: Arc::new(Mutex::new(None)),
            model_manager,
            current_model_id: Arc::new(Mutex::new(None)),
            last_activity: Arc::new(AtomicU64::new(0)),
            is_loading: Arc::new(Mutex::new(false)),
            loading_condvar: Arc::new(Condvar::new()),
        }
    }

    pub fn ensure_model_loaded(&self, model_id: &str) -> Result<()> {
        {
            let current = self.current_model_id.lock().unwrap();
            if let Some(ref id) = *current {
                if id == model_id {
                    return Ok(());
                }
            }
        }

        let mut loading = self.is_loading.lock().unwrap();
        while *loading {
            loading = self.loading_condvar.wait(loading).unwrap();
        }
        *loading = true;
        drop(loading);

        let result = self.load_model_internal(model_id);

        {
            let mut loading = self.is_loading.lock().unwrap();
            *loading = false;
        }
        self.loading_condvar.notify_all();

        result
    }

    fn load_model_internal(&self, model_id: &str) -> Result<()> {
        {
            let current = self.current_model_id.lock().unwrap();
            if let Some(ref id) = *current {
                if id == model_id {
                    return Ok(());
                }
            }
        }

        self.unload();

        let model_info = self.model_manager.get_model_info(model_id)?;
        if !model_info.is_downloaded {
            return Err(anyhow!(
                "Model '{}' is not downloaded. Please download it first.",
                model_id
            ));
        }

        let path = self.model_manager.get_model_path(model_id)?;

        let engine = match model_info.engine_type {
            EngineType::Whisper => LoadedEngine::Whisper(
                WhisperEngine::load(&path)
                    .map_err(|e| anyhow!("Failed to load Whisper model: {}", e))?,
            ),
            EngineType::Parakeet => LoadedEngine::Parakeet(
                ParakeetModel::load(&path, &Quantization::Int8)
                    .map_err(|e| anyhow!("Failed to load Parakeet model: {}", e))?,
            ),
            EngineType::Moonshine | EngineType::MoonshineStreaming => LoadedEngine::Moonshine(
                MoonshineModel::load(&path, MoonshineVariant::Base, &Quantization::default())
                    .map_err(|e| anyhow!("Failed to load Moonshine model: {}", e))?,
            ),
            EngineType::SenseVoice => LoadedEngine::SenseVoice(
                SenseVoiceModel::load(&path, &Quantization::Int8)
                    .map_err(|e| anyhow!("Failed to load SenseVoice model: {}", e))?,
            ),
            EngineType::GigaAM => LoadedEngine::GigaAM(
                GigaAMModel::load(&path, &Quantization::Int8)
                    .map_err(|e| anyhow!("Failed to load GigaAM model: {}", e))?,
            ),
            EngineType::Canary => LoadedEngine::Canary(
                CanaryModel::load(&path, &Quantization::Int8)
                    .map_err(|e| anyhow!("Failed to load Canary model: {}", e))?,
            ),
            EngineType::Cohere => LoadedEngine::Cohere(
                CohereModel::load(&path, &Quantization::Int8)
                    .map_err(|e| anyhow!("Failed to load Cohere model: {}", e))?,
            ),
        };

        {
            let mut guard = self.engine.lock().unwrap();
            *guard = Some(engine);
        }
        {
            let mut current = self.current_model_id.lock().unwrap();
            *current = Some(model_id.to_string());
        }

        Ok(())
    }

    pub fn transcribe(&self, audio: Vec<f32>) -> Result<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_activity.store(now, Ordering::Relaxed);

        let mut guard = self.engine.lock().unwrap();
        let engine_opt = guard.take();

        if engine_opt.is_none() {
            *guard = engine_opt;
            return Err(anyhow!("No model loaded"));
        }

        let mut engine = engine_opt.unwrap();

        let result = match &mut engine {
            LoadedEngine::Whisper(e) => {
                let opts = transcribe_rs::TranscribeOptions::default();
                e.transcribe(&audio, &opts)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("Whisper transcription failed: {}", e))
            }
            LoadedEngine::Parakeet(e) => {
                let params = transcribe_rs::onnx::parakeet::ParakeetParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("Parakeet transcription failed: {}", e))
            }
            LoadedEngine::Moonshine(e) => {
                let params = transcribe_rs::onnx::moonshine::MoonshineParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("Moonshine transcription failed: {}", e))
            }
            LoadedEngine::SenseVoice(e) => {
                let params = transcribe_rs::onnx::sense_voice::SenseVoiceParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("SenseVoice transcription failed: {}", e))
            }
            LoadedEngine::GigaAM(e) => {
                let params = transcribe_rs::onnx::gigaam::GigaAMParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("GigaAM transcription failed: {}", e))
            }
            LoadedEngine::Canary(e) => {
                let params = transcribe_rs::onnx::canary::CanaryParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("Canary transcription failed: {}", e))
            }
            LoadedEngine::Cohere(e) => {
                let params = transcribe_rs::onnx::cohere::CohereParams::default();
                e.transcribe_with(&audio, &params)
                    .map(|r| r.text)
                    .map_err(|e| anyhow!("Cohere transcription failed: {}", e))
            }
        };

        match result {
            Ok(text) => {
                *guard = Some(engine);
                Ok(text)
            }
            Err(e) => {
                *guard = Some(engine);
                Err(e)
            }
        }
    }

    pub fn unload(&self) {
        {
            let mut guard = self.engine.lock().unwrap();
            *guard = None;
        }
        {
            let mut current = self.current_model_id.lock().unwrap();
            *current = None;
        }
    }

    pub fn get_current_model_id(&self) -> Option<String> {
        self.current_model_id.lock().unwrap().clone()
    }
}

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EngineType {
    Whisper,
    Parakeet,
    Moonshine,
    MoonshineStreaming,
    SenseVoice,
    GigaAM,
    Canary,
    Cohere,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub engine_type: EngineType,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size_mb: u64,
    pub is_directory: bool,
    #[serde(default)]
    pub is_downloaded: bool,
}

fn builtin_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "parakeet-tdt-0.6b-v3".into(),
            name: "Parakeet V3".into(),
            engine_type: EngineType::Parakeet,
            filename: "parakeet-tdt-0.6b-v3-int8".into(),
            url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz".into(),
            sha256: String::new(),
            size_mb: 456,
            is_directory: true,
            is_downloaded: false,
        },
        ModelInfo {
            id: "parakeet-tdt-0.6b-v2".into(),
            name: "Parakeet V2".into(),
            engine_type: EngineType::Parakeet,
            filename: "parakeet-tdt-0.6b-v2-int8".into(),
            url: "https://blob.handy.computer/parakeet-v2-int8.tar.gz".into(),
            sha256: String::new(),
            size_mb: 451,
            is_directory: true,
            is_downloaded: false,
        },
        ModelInfo {
            id: "small".into(),
            name: "Whisper Small".into(),
            engine_type: EngineType::Whisper,
            filename: "ggml-small.bin".into(),
            url: "https://blob.handy.computer/ggml-small.bin".into(),
            sha256: String::new(),
            size_mb: 465,
            is_directory: false,
            is_downloaded: false,
        },
        ModelInfo {
            id: "medium".into(),
            name: "Whisper Medium".into(),
            engine_type: EngineType::Whisper,
            filename: "whisper-medium-q4_1.bin".into(),
            url: "https://blob.handy.computer/whisper-medium-q4_1.bin".into(),
            sha256: String::new(),
            size_mb: 469,
            is_directory: false,
            is_downloaded: false,
        },
        ModelInfo {
            id: "turbo".into(),
            name: "Whisper Turbo".into(),
            engine_type: EngineType::Whisper,
            filename: "ggml-large-v3-turbo.bin".into(),
            url: "https://blob.handy.computer/ggml-large-v3-turbo.bin".into(),
            sha256: String::new(),
            size_mb: 1549,
            is_directory: false,
            is_downloaded: false,
        },
        ModelInfo {
            id: "large".into(),
            name: "Whisper Large".into(),
            engine_type: EngineType::Whisper,
            filename: "ggml-large-v3-q5_0.bin".into(),
            url: "https://blob.handy.computer/ggml-large-v3-q5_0.bin".into(),
            sha256: String::new(),
            size_mb: 1031,
            is_directory: false,
            is_downloaded: false,
        },
        ModelInfo {
            id: "moonshine-base".into(),
            name: "Moonshine Base".into(),
            engine_type: EngineType::Moonshine,
            filename: "moonshine-base".into(),
            url: "https://blob.handy.computer/moonshine-base.tar.gz".into(),
            sha256: String::new(),
            size_mb: 55,
            is_directory: true,
            is_downloaded: false,
        },
        ModelInfo {
            id: "sense-voice-int8".into(),
            name: "SenseVoice".into(),
            engine_type: EngineType::SenseVoice,
            filename: "sense-voice-int8".into(),
            url: "https://blob.handy.computer/sense-voice-int8.tar.gz".into(),
            sha256: String::new(),
            size_mb: 152,
            is_directory: true,
            is_downloaded: false,
        },
        ModelInfo {
            id: "gigaam-v3-e2e-ctc".into(),
            name: "GigaAM v3".into(),
            engine_type: EngineType::GigaAM,
            filename: "gigaam-v3-e2e-ctc".into(),
            url: "https://blob.handy.computer/gigaam-v3-e2e-ctc.tar.gz".into(),
            sha256: String::new(),
            size_mb: 151,
            is_directory: true,
            is_downloaded: false,
        },
        ModelInfo {
            id: "canary-180m-flash".into(),
            name: "Canary Flash".into(),
            engine_type: EngineType::Canary,
            filename: "canary-180m-flash".into(),
            url: "https://blob.handy.computer/canary-180m-flash.tar.gz".into(),
            sha256: String::new(),
            size_mb: 146,
            is_directory: true,
            is_downloaded: false,
        },
    ]
}

pub struct ModelManager {
    models_dir: PathBuf,
    available_models: Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&models_dir)?;

        let mut models: HashMap<String, ModelInfo> = builtin_models()
            .into_iter()
            .map(|m| {
                let downloaded = Self::check_downloaded_static(&models_dir, &m);
                let mut m = m;
                m.is_downloaded = downloaded;
                (m.id.clone(), m)
            })
            .collect();

        if let Ok(extra) = fs::read_to_string(models_dir.join("custom_models.json")) {
            if let Ok(custom) = serde_json::from_str::<Vec<ModelInfo>>(&extra) {
                for m in custom {
                    let downloaded = Self::check_downloaded_static(&models_dir, &m);
                    let mut m = m;
                    m.is_downloaded = downloaded;
                    models.insert(m.id.clone(), m);
                }
            }
        }

        Ok(Self {
            models_dir,
            available_models: Mutex::new(models),
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn check_downloaded_static(models_dir: &PathBuf, model: &ModelInfo) -> bool {
        let path = models_dir.join(&model.filename);
        if model.is_directory {
            path.is_dir()
        } else {
            path.is_file()
        }
    }

    pub fn list_models(&self) -> Vec<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        let mut list: Vec<ModelInfo> = models.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub fn get_model_path(&self, model_id: &str) -> Result<PathBuf> {
        let models = self.available_models.lock().unwrap();
        let model = models
            .get(model_id)
            .ok_or_else(|| anyhow!("Model '{}' not found", model_id))?;
        Ok(self.models_dir.join(&model.filename))
    }

    pub fn get_model_info(&self, model_id: &str) -> Result<ModelInfo> {
        let models = self.available_models.lock().unwrap();
        models
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("Model '{}' not found", model_id))
    }

    pub fn check_downloaded(&self, model: &ModelInfo) -> bool {
        Self::check_downloaded_static(&self.models_dir, model)
    }

    pub async fn download_model(&self, model_id: &str, app_handle: &AppHandle) -> Result<()> {
        let model = {
            let models = self.available_models.lock().unwrap();
            models
                .get(model_id)
                .cloned()
                .ok_or_else(|| anyhow!("Model '{}' not found", model_id))?
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        let result = if model.is_directory {
            self.download_and_extract(&model, app_handle, &cancel_flag)
                .await
        } else {
            self.download_file(&model, app_handle, &cancel_flag).await
        };

        {
            let mut flags = self.cancel_flags.lock().unwrap();
            flags.remove(model_id);
        }

        result?;

        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(m) = models.get_mut(model_id) {
                m.is_downloaded = true;
            }
        }

        Ok(())
    }

    async fn download_file(
        &self,
        model: &ModelInfo,
        app_handle: &AppHandle,
        cancel_flag: &Arc<AtomicBool>,
    ) -> Result<()> {
        let dest_path = self.models_dir.join(&model.filename);

        let response = reqwest::get(&model.url).await?;
        let total_bytes = response.content_length().unwrap_or(0);

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&dest_path).await?;

        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                let _ = tokio::fs::remove_file(&dest_path).await;
                return Err(anyhow!("Download cancelled"));
            }

            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if total_bytes > 0 {
                let _ = app_handle.emit(
                    "model-download-progress",
                    serde_json::json!({
                        "model_id": model.id,
                        "downloaded_bytes": downloaded,
                        "total_bytes": total_bytes,
                        "progress_pct": (downloaded as f64 / total_bytes as f64 * 100.0).round() as u64,
                    }),
                );
            }
        }

        file.flush().await?;
        Ok(())
    }

    async fn download_and_extract(
        &self,
        model: &ModelInfo,
        app_handle: &AppHandle,
        cancel_flag: &Arc<AtomicBool>,
    ) -> Result<()> {
        let tmp_dir = self.models_dir.join(format!("{}.tmp", model.filename));
        let archive_path = self
            .models_dir
            .join(format!("{}.tar.gz", model.filename));
        let final_dir = self.models_dir.join(&model.filename);

        let result = async {
            let response = reqwest::get(&model.url).await?;
            let total_bytes = response.content_length().unwrap_or(0);

            let mut downloaded: u64 = 0;
            let mut stream = response.bytes_stream();
            let mut file = tokio::fs::File::create(&archive_path).await?;

            use futures_util::StreamExt;

            while let Some(chunk) = stream.next().await {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(anyhow!("Download cancelled"));
                }

                let chunk = chunk?;
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;

                if total_bytes > 0 {
                    let _ = app_handle.emit(
                        "model-download-progress",
                        serde_json::json!({
                            "model_id": model.id,
                            "downloaded_bytes": downloaded,
                            "total_bytes": total_bytes,
                            "progress_pct": (downloaded as f64 / total_bytes as f64 * 100.0).round() as u64,
                        }),
                    );
                }
            }

            file.flush().await?;
            drop(file);

            if cancel_flag.load(Ordering::Relaxed) {
                return Err(anyhow!("Download cancelled"));
            }

            let _ = fs::remove_dir_all(&tmp_dir);
            fs::create_dir_all(&tmp_dir)?;

            let archive_file = fs::File::open(&archive_path)?;
            let gz = flate2::read::GzDecoder::new(archive_file);
            let mut archive = tar::Archive::new(gz);
            archive.unpack(&tmp_dir)?;

            if final_dir.exists() {
                let _ = fs::remove_dir_all(&final_dir);
            }

            let tmp_entries: Vec<_> = fs::read_dir(&tmp_dir)?
                .filter_map(|e| e.ok())
                .collect();

            if tmp_entries.len() == 1 && tmp_entries[0].path().is_dir() {
                fs::rename(&tmp_entries[0].path(), &final_dir)?;
                let _ = fs::remove_dir_all(&tmp_dir);
            } else {
                fs::rename(&tmp_dir, &final_dir)?;
            }

            Ok(())
        }
        .await;

        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&tmp_dir);

        result
    }

    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        let model = {
            let models = self.available_models.lock().unwrap();
            models
                .get(model_id)
                .cloned()
                .ok_or_else(|| anyhow!("Model '{}' not found", model_id))?
        };

        let path = self.models_dir.join(&model.filename);
        if model.is_directory {
            if path.is_dir() {
                fs::remove_dir_all(&path)?;
            }
        } else if path.is_file() {
            fs::remove_file(&path)?;
        }

        {
            let mut models = self.available_models.lock().unwrap();
            if let Some(m) = models.get_mut(model_id) {
                m.is_downloaded = false;
            }
        }

        Ok(())
    }

    pub fn cancel_download(&self, model_id: &str) {
        let flags = self.cancel_flags.lock().unwrap();
        if let Some(flag) = flags.get(model_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
pub async fn list_available_models(
    models_dir_state: tauri::State<'_, Arc<ModelManager>>,
) -> Result<Vec<ModelInfo>, String> {
    Ok(models_dir_state.list_models())
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    let state = app.state::<Arc<ModelManager>>();
    state
        .download_model(&model_id, &app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    let state = app.state::<Arc<ModelManager>>();
    state.delete_model(&model_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_model_download(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    let state = app.state::<Arc<ModelManager>>();
    state.cancel_download(&model_id);
    Ok(())
}

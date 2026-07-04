// voice.rs - TTS/STT framework (OpenAI-compatible)
#![allow(dead_code)]

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::db;
use crate::lib_state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfigDto {
    pub tts_enabled:     bool,
    pub stt_enabled:     bool,
    pub tts_voice:       String,
    pub tts_model:       String,
    pub tts_base_url:    String,
    pub has_tts_api_key: bool,
}

#[derive(Debug, Clone)]
struct VoiceRuntime {
    tts_enabled:  bool,
    stt_enabled:  bool,
    tts_voice:    String,
    tts_model:    String,
    tts_base_url: String,
    tts_api_key:  String,
}

async fn cfg_bool(pool: &SqlitePool, key: &str) -> bool {
    matches!(
        db::get_config(pool, key).await.ok().flatten().as_deref(),
        Some("1" | "true")
    )
}

async fn cfg_str(pool: &SqlitePool, key: &str, default: &str) -> String {
    db::get_config(pool, key)
        .await
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

async fn load_runtime(pool: &SqlitePool, llm_api_key: &str) -> VoiceRuntime {
    VoiceRuntime {
        tts_enabled:  cfg_bool(pool, "voice_tts_enabled").await,
        stt_enabled:  cfg_bool(pool, "voice_stt_enabled").await,
        tts_voice:    cfg_str(pool, "voice_tts_voice", "nova").await,
        tts_model:    cfg_str(pool, "voice_tts_model", "tts-1").await,
        tts_base_url: cfg_str(pool, "voice_tts_base_url", "https://api.openai.com/v1").await,
        tts_api_key:  db::get_config(pool, "voice_tts_api_key")
            .await
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| llm_api_key.to_string()),
    }
}

#[derive(Deserialize)]
pub struct VoiceUpdatePayload {
    pub tts_enabled:  Option<bool>,
    pub stt_enabled:  Option<bool>,
    pub tts_voice:    Option<String>,
    pub tts_model:    Option<String>,
    pub tts_base_url: Option<String>,
    pub tts_api_key:  Option<String>,
}

#[tauri::command]
pub async fn voice_get_config(state: State<'_, AppState>) -> Result<VoiceConfigDto, String> {
    let llm_key = state.llm_cfg_hot.lock().map_err(|e| e.to_string())?.api_key.clone();
    let rt = load_runtime(&state.pool, &llm_key).await;
    Ok(VoiceConfigDto {
        tts_enabled:     rt.tts_enabled,
        stt_enabled:     rt.stt_enabled,
        tts_voice:       rt.tts_voice,
        tts_model:       rt.tts_model,
        tts_base_url:    rt.tts_base_url,
        has_tts_api_key: !rt.tts_api_key.is_empty(),
    })
}

#[tauri::command]
pub async fn voice_update_config(
    state: State<'_, AppState>,
    payload: VoiceUpdatePayload,
) -> Result<(), String> {
    let pool = &state.pool;
    if let Some(v) = payload.tts_enabled {
        db::set_config(pool, "voice_tts_enabled", if v { "1" } else { "0" })
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = payload.stt_enabled {
        db::set_config(pool, "voice_stt_enabled", if v { "1" } else { "0" })
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = &payload.tts_voice {
        db::set_config(pool, "voice_tts_voice", v).await.map_err(|e| e.to_string())?;
    }
    if let Some(v) = &payload.tts_model {
        db::set_config(pool, "voice_tts_model", v).await.map_err(|e| e.to_string())?;
    }
    if let Some(v) = &payload.tts_base_url {
        db::set_config(pool, "voice_tts_base_url", v).await.map_err(|e| e.to_string())?;
    }
    if let Some(v) = &payload.tts_api_key {
        if !v.is_empty() {
            db::set_config(pool, "voice_tts_api_key", v).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct TtsRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
}

pub async fn synthesize_mp3(rt: &VoiceRuntime, text: &str) -> Result<Vec<u8>, String> {
    if !rt.tts_enabled {
        return Err("TTS not enabled".into());
    }
    if rt.tts_api_key.is_empty() {
        return Err("TTS API key missing".into());
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty text".into());
    }
    let url = format!("{}/audio/speech", rt.tts_base_url.trim_end_matches('/'));
    let client = Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(&rt.tts_api_key)
        .json(&TtsRequest {
            model: &rt.tts_model,
            input: trimmed,
            voice: &rt.tts_voice,
        })
        .send()
        .await
        .map_err(|e| format!("TTS request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("TTS API {status}: {body}"));
    }
    resp.bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn voice_synthesize(state: State<'_, AppState>, text: String) -> Result<String, String> {
    let llm_key = state.llm_cfg_hot.lock().map_err(|e| e.to_string())?.api_key.clone();
    let rt = load_runtime(&state.pool, &llm_key).await;
    let bytes = synthesize_mp3(&rt, &text).await?;
    Ok(STANDARD.encode(bytes))
}

#[tauri::command]
pub async fn voice_transcribe(
    state: State<'_, AppState>,
    audio_base64: String,
    mime_type: Option<String>,
) -> Result<String, String> {
    let llm_key = state.llm_cfg_hot.lock().map_err(|e| e.to_string())?.api_key.clone();
    let rt = load_runtime(&state.pool, &llm_key).await;
    if !rt.stt_enabled {
        return Err("STT not enabled".into());
    }
    if rt.tts_api_key.is_empty() {
        return Err("STT API key missing".into());
    }
    let audio = STANDARD
        .decode(audio_base64.trim())
        .map_err(|e| e.to_string())?;
    if audio.is_empty() {
        return Err("empty audio".into());
    }
    let mime = mime_type.unwrap_or_else(|| "audio/webm".into());
    let ext = if mime.contains("wav") {
        "wav"
    } else if mime.contains("mp4") || mime.contains("m4a") {
        "m4a"
    } else {
        "webm"
    };
    let url = format!(
        "{}/audio/transcriptions",
        rt.tts_base_url.trim_end_matches('/')
    );
    let part = reqwest::multipart::Part::bytes(audio)
        .file_name(format!("audio.{ext}"))
        .mime_str(&mime)
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1");
    let client = Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(&rt.tts_api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("STT request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("STT API {status}: {body}"));
    }
    #[derive(Deserialize)]
    struct WhisperResp {
        text: String,
    }
    let parsed: WhisperResp = resp.json().await.map_err(|e| e.to_string())?;
    Ok(parsed.text.trim().to_string())
}

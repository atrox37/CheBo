// memory_vector.rs - vector memory (DeepSeek local fallback)
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::db;
use crate::llm::LlmConfig;
use crate::local_embed::{embed_local, LOCAL_MODEL_ID};

const INDEX_BATCH_SIZE: i64 = 8;
const MIN_SIMILARITY: f32 = 0.32;
const OLLAMA_DEFAULT: &str = "http://127.0.0.1:11434";
const OLLAMA_EMBED_MODEL: &str = "nomic-embed-text";

pub fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_vec(blob: &[u8], dims: i32) -> Vec<f32> {
    let expected = (dims as usize) * 4;
    if blob.len() < expected { return Vec::new(); }
    blob[..expected].chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1e-8 { 0.0 } else { dot / denom }
}

fn is_ollama_base(base_url: &str) -> bool {
    let u = base_url.to_lowercase();
    u.contains("11434") || u.contains("ollama")
}

fn is_deepseek_base(base_url: &str) -> bool {
    base_url.to_lowercase().contains("deepseek")
}

fn is_local_embed_config(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "local" | "builtin" | "chebo-local" | "chebo-local-v1")
}

fn normalize_ollama_root(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") { trimmed.trim_end_matches("/v1").to_string() } else { trimmed.to_string() }
}

async fn ollama_reachable(base: &str) -> bool {
    let root = normalize_ollama_root(base);
    let url = format!("{root}/api/tags");
    match Client::builder().timeout(Duration::from_secs(2)).build() {
        Ok(c) => c.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false),
        Err(_) => false,
    }
}

async fn resolve_embedding_settings(pool: &SqlitePool, cfg: &LlmConfig) -> (String, String, String) {
    let cfg_model = db::get_config(pool, "embedding_model").await.ok().flatten();
    let cfg_base = db::get_config(pool, "embedding_base_url").await.ok().flatten();
    let api_key = db::get_config(pool, "embedding_api_key").await.ok().flatten()
        .filter(|s| !s.is_empty()).unwrap_or_else(|| cfg.api_key.clone());

    if let Some(base) = cfg_base.filter(|s| !s.is_empty()) {
        if is_local_embed_config(&base) {
            return (LOCAL_MODEL_ID.into(), String::new(), api_key);
        }
        let model = cfg_model.filter(|s| !s.is_empty()).unwrap_or_else(|| {
            if is_ollama_base(&base) { OLLAMA_EMBED_MODEL.into() } else { "text-embedding-3-small".into() }
        });
        return (model, base, api_key);
    }

    if is_ollama_base(&cfg.base_url) {
        let model = cfg_model.filter(|s| !s.is_empty()).unwrap_or_else(|| OLLAMA_EMBED_MODEL.into());
        return (model, cfg.base_url.clone(), api_key);
    }

    if is_deepseek_base(&cfg.base_url) {
        if ollama_reachable(OLLAMA_DEFAULT).await {
            log::info!("DeepSeek chat + Ollama embedding ({OLLAMA_EMBED_MODEL})");
            let model = cfg_model.filter(|s| !s.is_empty()).unwrap_or_else(|| OLLAMA_EMBED_MODEL.into());
            return (model, OLLAMA_DEFAULT.into(), api_key);
        }
        log::info!("DeepSeek chat + built-in local embedding ({LOCAL_MODEL_ID})");
        return (LOCAL_MODEL_ID.into(), String::new(), api_key);
    }

    let model = cfg_model.filter(|s| !s.is_empty()).unwrap_or_else(|| "text-embedding-3-small".into());
    (model, cfg.base_url.clone(), api_key)
}

async fn embed_ollama(base_url: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    let root = normalize_ollama_root(base_url);
    let url = format!("{root}/api/embeddings");
    let client = Client::builder().timeout(Duration::from_secs(60)).build()?;
    let resp = client.post(&url).json(&json!({ "model": model, "prompt": text })).send().await?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Ollama embedding failed: {body}"));
    }
    let val: Value = resp.json().await?;
    let arr = val.get("embedding").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("Ollama embedding response invalid"))?;
    Ok(arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
}

async fn embed_openai_compatible(base_url: &str, api_key: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    let root = base_url.trim_end_matches('/');
    let url = if root.ends_with("/v1") { format!("{root}/embeddings") } else { format!("{root}/v1/embeddings") };
    let client = Client::builder().timeout(Duration::from_secs(60)).build()?;
    let mut req = client.post(&url).json(&json!({ "model": model, "input": text }));
    if !api_key.is_empty() { req = req.bearer_auth(api_key); }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Embedding API failed: {body}"));
    }
    let val: Value = resp.json().await?;
    let arr = val.pointer("/data/0/embedding").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("Embedding API response invalid"))?;
    Ok(arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
}

pub async fn embed_text(pool: &SqlitePool, cfg: &LlmConfig, text: &str) -> Result<(Vec<f32>, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return Err(anyhow!("empty text")); }
    let (model, base, api_key) = resolve_embedding_settings(pool, cfg).await;

    if model == LOCAL_MODEL_ID {
        return Ok((embed_local(trimmed), LOCAL_MODEL_ID.into()));
    }

    let vec = if is_ollama_base(&base) {
        match embed_ollama(&base, &model, trimmed).await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Ollama embed failed, fallback local: {e}");
                return Ok((embed_local(trimmed), LOCAL_MODEL_ID.into()));
            }
        }
    } else {
        match embed_openai_compatible(&base, &api_key, &model, trimmed).await {
            Ok(v) => v,
            Err(e) => {
                if is_deepseek_base(&cfg.base_url) || is_deepseek_base(&base) {
                    log::warn!("no embedding API, fallback local: {e}");
                    return Ok((embed_local(trimmed), LOCAL_MODEL_ID.into()));
                }
                return Err(e);
            }
        }
    };

    if vec.is_empty() { return Err(anyhow!("empty embedding")); }
    Ok((vec, model))
}

async fn index_one(pool: &SqlitePool, cfg: &LlmConfig, source_type: &str, source_id: &str, content: &str) -> Result<()> {
    let hash = content_hash(content);
    let (embedding, embed_model) = embed_text(pool, cfg, content).await?;
    db::upsert_memory_vector(
        pool, source_type, source_id, content, &hash,
        &vec_to_blob(&embedding), embedding.len() as i32, &embed_model,
    ).await?;
    Ok(())
}

pub async fn sync_index_incremental(pool: &SqlitePool, cfg: &LlmConfig, max_items: i64) -> usize {
    let candidates = match db::fetch_unindexed_memory_items(pool, max_items).await {
        Ok(c) => c,
        Err(e) => { log::warn!("fetch_unindexed_memory_items: {e}"); return 0; }
    };
    let mut indexed = 0usize;
    for item in candidates {
        if index_one(pool, cfg, &item.source_type, &item.source_id, &item.content).await.is_ok() {
            indexed += 1;
        }
    }
    indexed
}

pub async fn start_index_loop(pool: SqlitePool, cfg: Arc<LlmConfig>) {
    tokio::time::sleep(Duration::from_secs(8)).await;
    loop {
        let n = sync_index_incremental(&pool, &cfg, INDEX_BATCH_SIZE * 4).await;
        if n > 0 { log::info!("memory vector index: added {n}"); }
        tokio::time::sleep(Duration::from_secs(if n > 0 { 30 } else { 300 })).await;
    }
}

#[derive(Debug, Clone)]
pub struct RecallHit {
    pub content: String,
    pub score: f32,
}

fn format_hit(source_type: &str, content: &str) -> String {
    match source_type {
        "summary" => format!("[摘要] {content}"),
        "ltm" => format!("[长期记忆] {content}"),
        "profile" => format!("[用户画像] {content}"),
        "persona" => format!("[人格记忆] {content}"),
        other => format!("[{other}] {content}"),
    }
}

pub async fn recall_semantic(pool: &SqlitePool, cfg: &LlmConfig, query: &str, top_k: usize) -> Result<Vec<RecallHit>> {
    let _ = sync_index_incremental(pool, cfg, INDEX_BATCH_SIZE).await;
    let (query_vec, embed_model) = embed_text(pool, cfg, query).await?;
    let rows = db::get_memory_vectors_by_model(pool, &embed_model).await?;
    if rows.is_empty() { return Ok(Vec::new()); }

    let mut scored: Vec<RecallHit> = rows.into_iter().filter_map(|row| {
        let vec = blob_to_vec(&row.embedding, row.dims);
        if vec.len() != query_vec.len() { return None; }
        let score = cosine_similarity(&query_vec, &vec);
        if score < MIN_SIMILARITY { return None; }
        Some(RecallHit { content: format_hit(&row.source_type, &row.content), score })
    }).collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    Ok(scored)
}

pub async fn recall_keyword(pool: &SqlitePool, query: &str) -> Vec<String> {
    use sqlx::Row;
    let kw = format!("%{}%", query.to_lowercase());
    let mut results: Vec<String> = Vec::new();

    if let Ok(rows) = sqlx::query(
        "SELECT summary AS content FROM memory_summaries WHERE LOWER(summary) LIKE ? ORDER BY created_at DESC LIMIT 5",
    ).bind(&kw).fetch_all(pool).await {
        for row in rows { results.push(format!("[摘要] {}", row.get::<String, _>("content"))); }
    }
    if let Ok(rows) = sqlx::query(
        "SELECT key, value FROM user_profile WHERE LOWER(key || ' ' || value) LIKE ? LIMIT 5",
    ).bind(&kw).fetch_all(pool).await {
        for row in rows {
            results.push(format!("[用户画像] {}: {}", row.get::<String, _>("key"), row.get::<String, _>("value")));
        }
    }
    if let Ok(rows) = sqlx::query(
        "SELECT content FROM long_term_memories WHERE LOWER(content) LIKE ? ORDER BY created_at DESC LIMIT 3",
    ).bind(&kw).fetch_all(pool).await {
        for row in rows { results.push(format!("[长期记忆] {}", row.get::<String, _>("content"))); }
    }
    results
}
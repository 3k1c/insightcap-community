use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Mutex;

// ─── VisionConfig ──────────────────────────────────────────────────────────

/// Vision model 組態，由呼叫端從 settings.ai_models.vision_model 建構
#[derive(Debug, Clone)]
pub struct VisionConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
}

impl VisionConfig {
    /// 從 ModelSettings 建構；若模型未設定或無 API key（非 Ollama）則回傳 None
    pub fn from_settings(s: &crate::settings::store::ModelSettings) -> Option<Self> {
        if s.model.trim().is_empty() {
            return None;
        }
        let api_key = s.api_key.clone().unwrap_or_default();
        if api_key.is_empty() && s.provider != "ollama" {
            return None;
        }
        Some(Self {
            provider: s.provider.clone(),
            model: s.model.clone(),
            api_key,
            base_url: s.base_url.clone(),
        })
    }
}

// ─── Vision 能力探測與快取 ──────────────────────────────────────────────────

static VISION_CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

fn vision_cache() -> &'static Mutex<HashMap<String, bool>> {
    VISION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 測試指定模型是否支援 vision（結果快取整個 App 生命週期）
/// 原理：送一張 4×4 紅色測試圖並詢問顏色，若能回覆即為 vision model
pub async fn probe_vision_support(config: &VisionConfig) -> bool {
    let cache_key = format!("{}::{}", config.provider, config.model);

    // 快取命中
    {
        let cache = vision_cache().lock().await;
        if let Some(&result) = cache.get(&cache_key) {
            return result;
        }
    }

    println!(
        "[Vision] 正在探測 {} ({}) 是否支援 vision...",
        config.model, config.provider
    );

    let test_img = generate_probe_image();
    let probe_prompt = "What color is this image? Reply with just the color name.";

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        describe_image(
            &test_img,
            &config.provider,
            &config.model,
            &config.api_key,
            config.base_url.as_deref(),
            probe_prompt,
        ),
    )
    .await;

    let is_vision = match result {
        Ok(Ok(text)) => !text.trim().is_empty(),
        Ok(Err(e)) => {
            println!("[Vision] 探測失敗: {}", e);
            false
        }
        Err(_) => {
            println!("[Vision] 探測超時");
            false
        }
    };

    // 寫入快取
    {
        let mut cache = vision_cache().lock().await;
        cache.insert(cache_key, is_vision);
    }

    if is_vision {
        println!("[Vision] ✅ {} 支援 vision", config.model);
    } else {
        println!("[Vision] ❌ {} 不支援 vision，略過", config.model);
    }

    is_vision
}

/// 嘗試用 vision model 增強圖片理解
/// 回傳 Some(text) 表示成功，None 表示不可用或失敗
pub async fn try_vision_enhance(
    config: &VisionConfig,
    image_bytes: &[u8],
    prompt: &str,
) -> Option<String> {
    if !probe_vision_support(config).await {
        return None;
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(120),
        describe_image(
            image_bytes,
            &config.provider,
            &config.model,
            &config.api_key,
            config.base_url.as_deref(),
            prompt,
        ),
    )
    .await
    {
        Ok(Ok(raw)) => {
            let text = extract_text_from_vision_output(&raw);
            if text.is_empty() || text == "(no text detected)" {
                None
            } else {
                Some(text)
            }
        }
        Ok(Err(e)) => {
            eprintln!("[Vision] enhance failed: {}", e);
            None
        }
        Err(_) => {
            eprintln!("[Vision] enhance timed out");
            None
        }
    }
}

/// 生成 4×4 紅色 PNG 測試圖片（用於 probe）
fn generate_probe_image() -> Vec<u8> {
    let img = image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
        4,
        4,
        image::Rgb([255u8, 0, 0]),
    ));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("Failed to encode probe image");
    buf
}

// ─── 統一 Vision/OCR 呼叫入口 ──────────────────────────────────────────────

/// 統一的 Vision/OCR 呼叫入口
/// 支援：
///   - GLM-OCR 模式（OpenAI-compatible, /v1/chat/completions）
///   - LLaVA / Ollama 舊格式（/api/chat + images array）
///   - OpenAI GPT-4o Vision
///   - Anthropic Claude Vision
pub async fn describe_image(
    image_data: &[u8],
    provider: &str,
    model: &str,
    api_key: &str,
    base_url: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let b64_img = STANDARD.encode(image_data);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        "openai" => call_openai_vision(&client, &b64_img, model, api_key, prompt).await,
        "anthropic" => call_anthropic_vision(&client, &b64_img, model, api_key, prompt).await,
        _ => {
            // For Ollama, detect whether to use OpenAI-compatible or legacy format
            // GLM-OCR and newer multimodal models use /v1/chat/completions
            // LLaVA uses /api/chat with images array
            let base = base_url.unwrap_or("http://localhost:11434");
            let normalized_base = base.trim_end_matches('/');

            // 判斷是否使用 OpenAI-compatible 格式
            // GLM-OCR、minicpm-v 等新式模型使用 /v1/ 端點
            let is_openai_compat = is_openai_compatible_model(model);

            if is_openai_compat {
                call_openai_compat_vision(
                    &client,
                    &b64_img,
                    model,
                    normalized_base,
                    api_key,
                    prompt,
                )
                .await
            } else {
                call_ollama_legacy_vision(&client, &b64_img, model, normalized_base, prompt).await
            }
        }
    }
}

/// 判斷模型是否需要 OpenAI-compatible 格式
fn is_openai_compatible_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    // GLM-OCR、MiniCPM-V、InternVL、DeepSeek 等需要 /v1/ 端點
    // Qwen-VL 雖然支援 v1，但在本機載入 8B 時使用原生格式通常更穩定
    lower.contains("glm")
        || lower.contains("minicpm")
        || lower.contains("internvl")
        || lower.contains("deepseek")
        || lower.contains("qvq")
        || lower.contains("internvl2")
}

// ─── Prompts ────────────────────────────────────────────────────────────────

/// OCR Prompt — 一般場景（截圖、UI、對話內圖片）
pub fn general_vision_prompt() -> &'static str {
    "You are a precise visual content analyzer. Analyze the provided image and respond in the same language as the text visible in the image (use Traditional Chinese if Chinese text is present, otherwise use English).\n\nPlease provide:\n1. SUMMARY: A concise 1-3 sentence description of what the image shows\n2. TEXT: Extract ALL readable text (UI labels, headings, body text, code, etc.) preserving original formatting where possible\n3. TYPE: Classify as one of: [code, document, screenshot, diagram, photo, other]\n\nOutput Format:\n---\nSUMMARY: <description>\nTEXT: <all visible text>\nTYPE: <classification>\n---\n\nIf no text is visible, write: TEXT: (no text detected)"
}

/// OCR Prompt — 純文字辨識（文件掃描）
pub fn ocr_only_prompt() -> &'static str {
    "You are an OCR engine. Extract all text from this image exactly as written, preserving line breaks, paragraph spacing, bullet points, list structure, headers, and numbers. Output ONLY the extracted text, no commentary. If no text is visible, output: (no text detected)"
}

// ─── 輸出萃取 ───────────────────────────────────────────────────────────────

/// 從 Vision Model 輸出中萃取可用文字
pub fn extract_text_from_vision_output(raw: &str) -> String {
    // 嘗試解析結構化輸出（SUMMARY: ... TEXT: ... TYPE: ...）
    if let Some(text_start) = raw.find("TEXT:") {
        let after = &raw[text_start + 5..]; // skip "TEXT:"
        let text_part = if let Some(type_pos) = after.find("\nTYPE:") {
            &after[..type_pos]
        } else {
            after
        };
        let cleaned = text_part.trim().to_string();
        if !cleaned.is_empty() && cleaned != "(no text detected)" {
            return cleaned;
        }
    }

    // Fallback: 取整個輸出，但移除已知的前綴行
    let fallback: String = raw
        .lines()
        .filter(|line| {
            let l = line.trim();
            !l.starts_with("SUMMARY:")
                && !l.starts_with("TYPE:")
                && !l.starts_with("---")
                && !l.is_empty()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 截斷過長的輸出
    let max_len = 4000;
    if fallback.chars().count() > max_len {
        fallback.chars().take(max_len).collect()
    } else {
        fallback
    }
}

// ─── 各 Provider 實作 ──────────────────────────────────────────────────────

/// 呼叫 OpenAI GPT-4o Vision
async fn call_openai_vision(
    client: &Client,
    b64_img: &str,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    let res = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{}", b64_img)}},
                    {"type": "text", "text": prompt}
                ]
            }],
            "max_tokens": 1024
        }))
        .send()
        .await
        .map_err(|e| format!("OpenAI Vision request failed: {}", e))?;

    let json_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(json_res["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// 呼叫 Anthropic Claude Vision
async fn call_anthropic_vision(
    client: &Client,
    b64_img: &str,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 1024,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": b64_img}},
                    {"type": "text", "text": prompt}
                ]
            }]
        }))
        .send()
        .await
        .map_err(|e| format!("Anthropic Vision request failed: {}", e))?;

    let json_res: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(json_res["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// 呼叫 Ollama OpenAI-compatible 端點（GLM-OCR、MiniCPM-V 等）
async fn call_openai_compat_vision(
    client: &Client,
    b64_img: &str,
    model: &str,
    base: &str,
    api_key: &str,
    prompt: &str,
) -> Result<String, String> {
    let url = if base.ends_with("/v1") {
        format!("{}/chat/completions", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };

    let mut req = client.post(&url);

    // 若有 API key 則加入 Authorization header（雲端服務需要）
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let res = req
        .json(&json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": {"url": format!("data:image/png;base64,{}", b64_img)}
                    },
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }],
            "max_tokens": 1024,
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| format!("OpenAI-compat Vision request failed ({}): {}", url, e))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_body = res.text().await.unwrap_or_default();
        return Err(format!("Vision API error {}: {}", status, err_body));
    }

    let json_res: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Vision response: {}", e))?;

    Ok(json_res["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

/// 呼叫 Ollama 舊版格式（LLaVA 等 /api/chat + images array）
async fn call_ollama_legacy_vision(
    client: &Client,
    b64_img: &str,
    model: &str,
    base: &str,
    prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/api/chat", base);

    let res = client
        .post(&url)
        .json(&json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": prompt,
                "images": [b64_img]
            }],
            "stream": false
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama Legacy Vision request failed: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_body = res.text().await.unwrap_or_default();
        return Err(format!("Ollama Vision error {}: {}", status, err_body));
    }

    let json_res: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama Vision response: {}", e))?;

    Ok(json_res["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

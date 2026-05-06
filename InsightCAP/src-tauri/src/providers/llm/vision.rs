use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct VisionConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
}

impl VisionConfig {
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

static VISION_CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

fn vision_cache() -> &'static Mutex<HashMap<String, bool>> {
    VISION_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn probe_vision_support(config: &VisionConfig) -> bool {
    let cache_key = format!("{}::{}", config.provider, config.model);

    {
        let cache = vision_cache().lock().await;
        if let Some(&result) = cache.get(&cache_key) {
            return result;
        }
    }

    println!(
        "[Vision] Probing vision support for {} ({})...",
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
            println!("[Vision] Probe failed: {}", e);
            false
        }
        Err(_) => {
            println!("[Vision] Probe timed out");
            false
        }
    };

    {
        let mut cache = vision_cache().lock().await;
        cache.insert(cache_key, is_vision);
    }

    if is_vision {
        println!("[Vision] {} supports vision", config.model);
    } else {
        println!("[Vision] {} does not support vision", config.model);
    }

    is_vision
}

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
            let base = base_url.unwrap_or("http://localhost:11434");
            let normalized_base = base.trim_end_matches('/');

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

fn is_openai_compatible_model(model: &str) -> bool {
    let lower = model.to_lowercase();
    lower.contains("glm")
        || lower.contains("minicpm")
        || lower.contains("internvl")
        || lower.contains("deepseek")
        || lower.contains("qvq")
        || lower.contains("internvl2")
}

pub fn general_vision_prompt() -> &'static str {
    "You are a precise visual content analyzer. Analyze the provided image and respond in the same language as the text visible in the image (use Traditional Chinese if Chinese text is present, otherwise use English).\n\nPlease provide:\n1. SUMMARY: A concise 1-3 sentence description of what the image shows\n2. TEXT: Extract ALL readable text (UI labels, headings, body text, code, etc.) preserving original formatting where possible\n3. TYPE: Classify as one of: [code, document, screenshot, diagram, photo, other]\n\nOutput Format:\n---\nSUMMARY: <description>\nTEXT: <all visible text>\nTYPE: <classification>\n---\n\nIf no text is visible, write: TEXT: (no text detected)"
}

pub fn ocr_only_prompt() -> &'static str {
    "You are an OCR engine. Extract all text from this image exactly as written, preserving line breaks, paragraph spacing, bullet points, list structure, headers, and numbers. Output ONLY the extracted text, no commentary. If no text is visible, output: (no text detected)"
}

pub fn extract_text_from_vision_output(raw: &str) -> String {
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

    let max_len = 4000;
    if fallback.chars().count() > max_len {
        fallback.chars().take(max_len).collect()
    } else {
        fallback
    }
}

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

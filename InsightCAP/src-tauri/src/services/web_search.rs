use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct TavilyRequest {
    api_key: String,
    query: String,
    search_depth: String,
    max_results: u32,
    include_answer: bool,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
    #[allow(dead_code)]
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TavilyResponse {
    answer: Option<String>,
    results: Vec<TavilyResult>,
}

/// 呼叫 Tavily 搜尋 API，回傳格式化的 web context 字串（可直接注入 system prompt）
/// 回傳 (formatted_context, sources_list)
pub async fn tavily_search(api_key: &str, query: &str) -> Result<(String, Vec<String>), String> {
    let client = Client::new();
    let body = TavilyRequest {
        api_key: api_key.to_string(),
        query: query.to_string(),
        search_depth: "basic".to_string(),
        max_results: 5,
        include_answer: true,
    };

    let resp = client
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Tavily 請求失敗: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Tavily API 錯誤 {}: {}", status, text));
    }

    let data: TavilyResponse = resp
        .json()
        .await
        .map_err(|e| format!("Tavily 回應解析失敗: {}", e))?;

    let mut parts: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();

    if let Some(answer) = &data.answer {
        if !answer.trim().is_empty() {
            parts.push(format!("**搜尋摘要**\n{}", answer.trim()));
        }
    }

    for (i, result) in data.results.iter().enumerate() {
        let content = result.content.trim();
        if !content.is_empty() {
            parts.push(format!(
                "[{}] **{}**\n來源：{}\n{}",
                i + 1,
                result.title,
                result.url,
                content
            ));
            sources.push(result.url.clone());
        }
    }

    if parts.is_empty() {
        return Err("Tavily 搜尋無結果".to_string());
    }

    Ok((parts.join("\n\n"), sources))
}

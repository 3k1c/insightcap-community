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
        .map_err(|e| format!("Tavily request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Tavily API error {}: {}", status, text));
    }

    let data: TavilyResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Tavily response: {}", e))?;

    let mut parts: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();

    if let Some(answer) = &data.answer {
        if !answer.trim().is_empty() {
            parts.push(format!("**Web Answer**\n{}", answer.trim()));
        }
    }

    for (i, result) in data.results.iter().enumerate() {
        let content = result.content.trim();
        if !content.is_empty() {
            parts.push(format!(
                "[{}] **{}**\nSource: {}\n{}",
                i + 1,
                result.title,
                result.url,
                content
            ));
            sources.push(result.url.clone());
        }
    }

    if parts.is_empty() {
        return Err("Tavily returned empty search results".to_string());
    }

    Ok((parts.join("\n\n"), sources))
}

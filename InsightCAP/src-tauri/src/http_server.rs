/// Phase 6 — 本地 HTTP API Server（Axum）
///
/// 端點：
///   GET  /api/health   — 連線確認
///   POST /api/capture  — 擷取文字/URL 到 inbox
///   POST /api/rag      — 查詢桌面知識庫，回傳 chunks + context_text
///   POST /api/chat     — 桌面代理推理（SSE streaming）
///   GET  /             — 手機 PWA 快速擷取頁

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::{
    db::AppState,
    providers::llm::{LLMOptions, LLMProvider, StreamToken},
    providers::llm::openai::OpenAiProvider,
    services::rag_engine::RagEngine,
    settings,
};

// ─── Shared State ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct ApiState {
    pool: SqlitePool,
    token: Arc<String>,
    app: AppHandle,
}

// ─── Request / Response Types ──────────────────────────────────────────────

#[derive(Deserialize)]
struct CaptureRequest {
    content: String,
    content_type: Option<String>,
    source_url: Option<String>,
}

#[derive(Deserialize)]
struct RagRequest {
    query: String,
    /// 限制回傳數量，預設 5，最多 10
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    /// [[role, content], ...] — role: "user" | "assistant"
    history: Option<Vec<[String; 2]>>,
}

#[derive(Deserialize)]
struct CreateConversationRequest {
    title: Option<String>,
}

#[derive(Serialize)]
struct ConversationItem {
    id: String,
    title: String,
    summary: Option<String>,
    updated_at: String,
}

#[derive(Serialize)]
struct MessageItem {
    id: String,
    role: String,
    content: String,
    created_at: String,
}

#[derive(Serialize)]
struct SourceItem {
    id: String,
    title: String,
    media_type: Option<String>,
    source_category: String,
    capture_count: i64,
    captured_at: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

// ─── Auth Helper ───────────────────────────────────────────────────────────

fn bearer_ok(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == expected)
        .unwrap_or(false)
}

// ─── Handlers ──────────────────────────────────────────────────────────────

async fn handle_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn handle_capture(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CaptureRequest>,
) -> StatusCode {
    if !bearer_ok(&headers, &s.token) {
        return StatusCode::UNAUTHORIZED;
    }

    let content = req.content.trim().to_string();
    if content.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let content_type = req.content_type.unwrap_or_else(|| {
        if (content.starts_with("http://") || content.starts_with("https://"))
            && !content.contains(' ')
        {
            "url".to_string()
        } else {
            "text".to_string()
        }
    });

    let source_url = req.source_url.unwrap_or_default();
    let id = Uuid::now_v7().to_string();
    let now = Utc::now().to_rfc3339();

    match sqlx::query(
        "INSERT INTO inbox \
         (id, content, content_type, source_exe, source_url, window_title, session_id, status, captured_at) \
         VALUES (?, ?, ?, 'MobileCapture', ?, '手機擷取', '', 'pending', ?)",
    )
    .bind(&id)
    .bind(&content)
    .bind(&content_type)
    .bind(&source_url)
    .bind(&now)
    .execute(&s.pool)
    .await
    {
        Ok(_) => {
            println!("[MobileCapture] inbox: {} ({})", &id[..8], content_type);
            StatusCode::CREATED
        }
        Err(e) => {
            eprintln!("[MobileCapture] DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn handle_rag(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<RagRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if req.query.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let app_state = s.app.state::<AppState>();
    let engine = RagEngine::new(
        app_state.db.clone(),
        app_state.vector_store.clone(),
        app_state.embedder.clone(),
    );

    let context = engine
        .retrieve_context(req.query.trim(), None, None, None)
        .await
        .map_err(|e| {
            eprintln!("[MobileRAG] error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // 在原始 context 基礎上，附加 context_text 供手機端直接注入 prompt
    let _limit = req.limit.unwrap_or(5).min(10);
    let context_text = rag_to_text(&context);

    let response = serde_json::json!({
        "context": context,
        "context_text": context_text,
    });

    Ok(Json(response))
}

async fn handle_chat(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if req.message.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let app_state = s.app.state::<AppState>();

    // 取桌面端 LLM 設定
    let settings = settings::store::get_settings(&app_state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cfg = &settings.ai_models.chat_llm;
    let api_key = cfg.api_key.clone().unwrap_or_default();
    let is_ollama = cfg.provider.to_lowercase().contains("ollama");

    if api_key.is_empty() && !is_ollama {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    // RAG：查詢桌面知識庫取得 context
    let rag_context = {
        let engine = RagEngine::new(
            app_state.db.clone(),
            app_state.vector_store.clone(),
            app_state.embedder.clone(),
        );
        engine
            .retrieve_context(req.message.trim(), None, None, None)
            .await
            .unwrap_or_else(|_| serde_json::json!({}))
    };
    let context_text = rag_to_text(&rag_context);

    // 組裝 system prompt
    let system_prompt = if context_text.is_empty() {
        "你是 InsightCAP 知識助理。根據對話回答用戶問題。".to_string()
    } else {
        format!(
            "你是 InsightCAP 知識助理。以下是來自用戶知識庫的相關資料，請優先參考：\n\n{}\n\n根據上述知識回答用戶的問題。",
            context_text
        )
    };

    // 組裝 history
    let history_vec: Vec<(String, String)> = req
        .history
        .unwrap_or_default()
        .into_iter()
        .map(|[role, content]| (role, content))
        .collect();

    let llm = OpenAiProvider::new(
        api_key,
        cfg.base_url.clone(),
        cfg.model.clone(),
        cfg.provider.clone(),
    );

    // mpsc channel：bridge callback → SSE stream
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    tokio::spawn(async move {
        let sp = system_prompt;
        let hist = history_vec;
        let msg = req.message;
        let opts = LLMOptions {
            stream: true,
            think_mode: Some(false),
            ..LLMOptions::default()
        };

        let _ = llm
            .complete_stream(
                &sp,
                &hist,
                &msg,
                opts,
                move |token| {
                    if let StreamToken::Content(c) = token {
                        let _ = tx.send(c);
                    }
                },
            )
            .await;
        // tx 在此 drop，rx.recv() 將回傳 None，stream 結束
    });

    let sse_stream = stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|token| {
            (Ok::<Event, Infallible>(Event::default().data(token)), rx)
        })
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

async fn handle_pwa() -> Html<&'static str> {
    Html(PWA_HTML)
}

// ─── RAG Context → Plain Text ──────────────────────────────────────────────

fn rag_to_text(ctx: &serde_json::Value) -> String {
    let mut parts: Vec<String> = Vec::new();

    // captures
    if let Some(arr) = ctx["captures"].as_array() {
        for item in arr {
            if let Some(content) = item["content"].as_str() {
                if !content.is_empty() {
                    let title = item["title"].as_str().unwrap_or("擷取內容");
                    parts.push(format!("[{}]\n{}", title, content));
                }
            }
        }
    }

    // memory chunks（patterns / logs / data）
    for key in &["patterns", "logs", "data"] {
        if let Some(arr) = ctx["memory"][key].as_array() {
            for item in arr {
                if let Some(content) = item["content"].as_str() {
                    if !content.is_empty() {
                        let kind = match *key {
                            "patterns" => "Pattern",
                            "logs" => "經驗記錄",
                            _ => "知識",
                        };
                        parts.push(format!("[{}]\n{}", kind, content));
                    }
                }
            }
        }
    }

    parts.join("\n\n---\n\n")
}

// ─── Token Management ──────────────────────────────────────────────────────

async fn load_or_create_token(pool: &SqlitePool) -> String {
    if let Ok(Some(token)) = sqlx::query_scalar::<_, String>(
        "SELECT value FROM settings WHERE key = 'mobile_api_token'",
    )
    .fetch_optional(pool)
    .await
    {
        return token;
    }

    // 用兩個 UUID v7 simple 格式拼成 32 位 hex token
    let token = format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple());
    let now = Utc::now().to_rfc3339();

    let _ = sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ('mobile_api_token', ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(&token)
    .bind(now)
    .execute(pool)
    .await;

    println!("[HTTP] Generated mobile API token: {}…", &token[..8]);
    token
}

// ─── Entry Point ───────────────────────────────────────────────────────────

/// Phase 6: 啟動本地 HTTP API Server（0.0.0.0:3030）
pub async fn start_api_server(app: AppHandle) {
    let pool = app.state::<SqlitePool>().inner().clone();
    let token = load_or_create_token(&pool).await;

    let state = ApiState {
        pool,
        token: Arc::new(token),
        app,
    };

    let router = Router::new()
        .route("/", get(handle_pwa))
        .route("/api/health", get(handle_health))
        .route("/api/capture", post(handle_capture))
        .route("/api/rag", post(handle_rag))
        .route("/api/chat", post(handle_chat))
        .route("/api/conversations", get(handle_list_conversations).post(handle_create_conversation))
        .route("/api/conversations/:id/messages", get(handle_get_messages))
        .route("/api/sources", get(handle_list_sources))
        .with_state(state);

    // 監聽所有介面，讓同 WiFi 的手機能連入
    match TcpListener::bind("0.0.0.0:3030").await {
        Ok(listener) => {
            println!("[HTTP] Mobile API listening on 0.0.0.0:3030");
            let _ = axum::serve(listener, router).await;
        }
        Err(e) => eprintln!("[HTTP] Failed to bind 0.0.0.0:3030 — {}", e),
    }
}

// ─── Conversations ─────────────────────────────────────────────────────────

async fn handle_list_conversations(
    State(s): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ConversationItem>>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, String)>(
        "SELECT id, title, summary, updated_at FROM conversations \
         ORDER BY updated_at DESC LIMIT 50",
    )
    .fetch_all(&s.pool)
    .await
    .map_err(|e| {
        eprintln!("[API/conversations] DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = rows
        .into_iter()
        .map(|(id, title, summary, updated_at)| ConversationItem {
            id,
            title: title.unwrap_or_else(|| "新對話".to_string()),
            summary,
            updated_at,
        })
        .collect();

    Ok(Json(items))
}

async fn handle_create_conversation(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CreateConversationRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let id = uuid::Uuid::now_v7().to_string();
    let title = req.title.unwrap_or_else(|| "新對話".to_string());
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&title)
    .bind(&now)
    .bind(&now)
    .execute(&s.pool)
    .await
    .map_err(|e| {
        eprintln!("[API/conversations] create error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "id": id, "title": title })))
}

async fn handle_get_messages(
    State(s): State<ApiState>,
    headers: HeaderMap,
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<MessageItem>>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, role, content, created_at FROM messages \
         WHERE conversation_id = ? ORDER BY created_at ASC",
    )
    .bind(&conversation_id)
    .fetch_all(&s.pool)
    .await
    .map_err(|e| {
        eprintln!("[API/messages] DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = rows
        .into_iter()
        .map(|(id, role, content, created_at)| MessageItem {
            id,
            role,
            content,
            created_at,
        })
        .collect();

    Ok(Json(items))
}

// ─── Sources ────────────────────────────────────────────────────────────────

async fn handle_list_sources(
    State(s): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SourceItem>>, StatusCode> {
    if !bearer_ok(&headers, &s.token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, i64, String)>(
        "SELECT id, title, media_type, source_category, capture_count, captured_at \
         FROM sources ORDER BY captured_at DESC LIMIT 100",
    )
    .fetch_all(&s.pool)
    .await
    .map_err(|e| {
        eprintln!("[API/sources] DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items = rows
        .into_iter()
        .map(|(id, title, media_type, source_category, capture_count, captured_at)| SourceItem {
            id,
            title,
            media_type,
            source_category,
            capture_count,
            captured_at,
        })
        .collect();

    Ok(Json(items))
}

// ─── PWA HTML（手機快速擷取頁）──────────────────────────────────────────────

const PWA_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-TW">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
  <meta name="apple-mobile-web-app-capable" content="yes">
  <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
  <meta name="mobile-web-app-capable" content="yes">
  <title>InsightCAP</title>
  <style>
    *{box-sizing:border-box;margin:0;padding:0;-webkit-tap-highlight-color:transparent}
    body{
      font-family:-apple-system,BlinkMacSystemFont,'Helvetica Neue',sans-serif;
      background:#0f0f11;color:#e5e5ea;min-height:100dvh;
      padding:20px 16px;
      padding-top:calc(24px + env(safe-area-inset-top));
      padding-bottom:calc(20px + env(safe-area-inset-bottom));
    }
    h1{font-size:22px;font-weight:700;letter-spacing:-.5px;margin-bottom:4px}
    .sub{font-size:13px;color:#636366;margin-bottom:22px}
    .card{background:#1c1c1e;border-radius:12px;padding:14px 16px;margin-bottom:12px}
    label{font-size:11px;font-weight:600;color:#636366;text-transform:uppercase;
          letter-spacing:.5px;display:block;margin-bottom:8px}
    input[type=password],textarea{
      width:100%;background:transparent;border:none;outline:none;
      color:#fff;font-size:16px;font-family:inherit;caret-color:#0a84ff
    }
    textarea{height:130px;resize:none;line-height:1.5}
    ::placeholder{color:#3a3a3c}
    .row{display:flex;align-items:center;gap:8px}
    .row input{flex:1}
    .x-btn{background:none;border:none;color:#48484a;font-size:18px;
            cursor:pointer;padding:2px 6px;flex-shrink:0}
    .saved{font-size:12px;color:#30d158;margin-top:6px;display:none}
    .btn{display:block;width:100%;padding:15px;background:#0a84ff;color:#fff;
         border:none;border-radius:12px;font-size:17px;font-weight:600;
         cursor:pointer;transition:opacity .15s;margin-top:4px}
    .btn:active{opacity:.7}
    .btn:disabled{background:#2c2c2e;color:#48484a;cursor:default;opacity:1}
    .toast{
      position:fixed;bottom:calc(24px + env(safe-area-inset-bottom));
      left:16px;right:16px;padding:14px 16px;border-radius:12px;
      font-size:15px;font-weight:500;text-align:center;
      opacity:0;transform:translateY(8px);
      transition:opacity .2s,transform .2s;pointer-events:none
    }
    .toast.show{opacity:1;transform:translateY(0)}
    .toast.ok{background:#1c3a2a;color:#30d158}
    .toast.err{background:#3a1c1c;color:#ff453a}
  </style>
</head>
<body>
  <h1>InsightCAP</h1>
  <p class="sub">擷取內容到桌面端知識庫</p>

  <div class="card" id="auth-card">
    <label>API Token</label>
    <div class="row">
      <input type="password" id="tok" placeholder="貼上 Token（只需輸入一次）" autocomplete="off">
      <button class="x-btn" onclick="clearTok()">✕</button>
    </div>
    <div class="saved" id="saved">已記住 Token ✓</div>
  </div>

  <div class="card">
    <label>擷取內容</label>
    <textarea id="content" placeholder="貼上文字、網址、想法…"></textarea>
  </div>

  <button class="btn" id="btn" onclick="send()">擷取到知識庫</button>
  <div class="toast" id="toast"></div>

  <script>
    const KEY='ic_tok_v1';
    function init(){
      const t=localStorage.getItem(KEY);
      if(t){document.getElementById('tok').value=t;showSaved();}
      document.getElementById('content').focus();
    }
    function showSaved(){
      document.getElementById('saved').style.display='block';
    }
    function clearTok(){
      localStorage.removeItem(KEY);
      document.getElementById('tok').value='';
      document.getElementById('saved').style.display='none';
      toast('已清除 Token','err');
    }
    async function send(){
      const tok=document.getElementById('tok').value.trim();
      const txt=document.getElementById('content').value.trim();
      const btn=document.getElementById('btn');
      if(!tok){toast('請先輸入 API Token','err');return;}
      if(!txt){toast('請輸入要擷取的內容','err');return;}
      btn.disabled=true;btn.textContent='擷取中…';
      try{
        const r=await fetch('/api/capture',{
          method:'POST',
          headers:{'Content-Type':'application/json','Authorization':'Bearer '+tok},
          body:JSON.stringify({content:txt})
        });
        if(r.status===201){
          localStorage.setItem(KEY,tok);showSaved();
          document.getElementById('content').value='';
          toast('已擷取 ✓','ok');
        }else if(r.status===401){
          toast('Token 無效，請重新輸入','err');
        }else{
          toast('擷取失敗（'+r.status+'）','err');
        }
      }catch{
        toast('連線失敗，請確認在同一 WiFi','err');
      }finally{
        btn.disabled=false;btn.textContent='擷取到知識庫';
      }
    }
    function toast(msg,type){
      const el=document.getElementById('toast');
      el.textContent=msg;el.className='toast '+type+' show';
      clearTimeout(window._t);
      window._t=setTimeout(()=>el.className='toast '+type,2800);
    }
    init();
  </script>
</body>
</html>"#;

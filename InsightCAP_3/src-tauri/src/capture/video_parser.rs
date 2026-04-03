use crate::capture::file_parser::{FileChunk, ParsedDocument};
use md5::{Digest, Md5};
use reqwest::Client;
use std::process::Command;

// ─── Bilibili WBI 簽名模組 ───
// B 站 2023 年起對 player/v2 等 API 加入 WBI 防篡改機制，
// 若請求缺少正確的 w_rid 和 wts 參數，伺服器會返回錯誤或錯位的資料。

/// B 站固定的混淆重排映射表（64 個索引值）
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

/// 從 nav API 回傳的 img_url 和 sub_url 中擷取檔名（不含副檔名）作為 key
fn extract_key_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

/// 用 MIXIN_KEY_ENC_TAB 打亂 (img_key + sub_key) 並截取前 32 字元
fn gen_mixin_key(img_key: &str, sub_key: &str) -> String {
    let raw = format!("{}{}", img_key, sub_key);
    let raw_chars: Vec<char> = raw.chars().collect();
    let mixin: String = MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&i| raw_chars.get(i).copied())
        .collect();
    mixin.chars().take(32).collect()
}

/// 從 B 站 nav API 獲取每日更新的 img_key 和 sub_key
async fn fetch_wbi_keys(
    client: &Client,
    sessdata: Option<&str>,
) -> Result<(String, String), String> {
    let mut req = client.get("https://api.bilibili.com/x/web-interface/nav");
    if let Some(sess) = sessdata {
        req = req.header("Cookie", format!("SESSDATA={}", sess));
    }
    let res = req
        .send()
        .await
        .map_err(|e| format!("Nav API error: {}", e))?;
    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Nav JSON parse error: {}", e))?;

    let img_url = json["data"]["wbi_img"]["img_url"]
        .as_str()
        .ok_or("Missing img_url in nav response")?;
    let sub_url = json["data"]["wbi_img"]["sub_url"]
        .as_str()
        .ok_or("Missing sub_url in nav response")?;

    let img_key = extract_key_from_url(img_url);
    let sub_key = extract_key_from_url(sub_url);

    println!(
        "[WBI] Got keys: img_key={}, sub_key={}",
        &img_key[..img_key.len().min(8)],
        &sub_key[..sub_key.len().min(8)]
    );

    Ok((img_key, sub_key))
}

/// 對一組查詢參數進行 WBI 簽名，返回帶 w_rid 和 wts 的完整查詢字串
fn wbi_sign(params: &[(&str, &str)], mixin_key: &str) -> String {
    let wts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    // 加入 wts，然後按 key 字典序排序
    let mut all_params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    all_params.push(("wts".to_string(), wts.clone()));
    all_params.sort_by(|a, b| a.0.cmp(&b.0));

    // 過濾掉值中含有特殊字元的參數（!'()*）
    let filtered: Vec<_> = all_params
        .iter()
        .filter(|(_, v)| !v.chars().any(|c| matches!(c, '!' | '\'' | '(' | ')' | '*')))
        .collect();

    // 序列化成 URL 查詢字串
    let query: String = filtered
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    // 計算 MD5(query + mixin_key) = w_rid
    let to_hash = format!("{}{}", query, mixin_key);
    let mut hasher = Md5::new();
    hasher.update(to_hash.as_bytes());
    let w_rid = format!("{:x}", hasher.finalize());

    format!("{}&w_rid={}", query, w_rid)
}

/// 尋找 yt-dlp 執行檔：先看應用程式同目錄，再向上尋找，最後看 PATH
fn find_ytdlp() -> Option<std::path::PathBuf> {
    // 1. 先嘗試 InsightCAP 可執行檔同目錄（Tauri 打包後 yt-dlp.exe 會放這裡）
    if let Ok(current_exe) = std::env::current_exe() {
        // 向上最多查 5 層父目錄（支援 dev 模式的 target/debug/ 結構）
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(d) = dir {
                let candidate = d.join("yt-dlp.exe");
                if candidate.exists() {
                    println!("[VIDEO_PARSER] Found yt-dlp at: {:?}", candidate);
                    return Some(candidate);
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    // 2. 再嘗試 PATH
    if Command::new("yt-dlp").arg("--version").output().is_ok() {
        return Some(std::path::PathBuf::from("yt-dlp"));
    }
    None
}

// YouTube extraction：主要使用 yt-dlp，降級使用 HTTP metadata
pub async fn fetch_youtube_subtitles(url: &str) -> Result<String, String> {
    // ─── 方案A：yt-dlp（最可靠）───
    if let Some(ytdlp) = find_ytdlp() {
        println!("[VIDEO_PARSER] 🎬 yt-dlp found at {:?}", ytdlp);
        match fetch_with_ytdlp(&ytdlp, url).await {
            Ok(text) if !text.trim().is_empty() => {
                println!("[VIDEO_PARSER] ✅ yt-dlp extracted: {} chars", text.len());
                return Ok(text);
            }
            Ok(_) => println!("[VIDEO_PARSER] ⚠️ yt-dlp returned empty, falling back..."),
            Err(e) => println!("[VIDEO_PARSER] ⚠️ yt-dlp failed: {}, falling back...", e),
        }
    } else {
        println!("[VIDEO_PARSER] ℹ️ yt-dlp not found, using HTTP fallback.");
    }

    // ─── 方案B（降級）：HTTP 爬取標題+描述 ───
    fetch_youtube_metadata(url).await
}

async fn fetch_with_ytdlp(ytdlp: &std::path::Path, url: &str) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("insightcap_yt");
    let _ = std::fs::create_dir_all(&temp_dir);
    let output_template = temp_dir.join("%(id)s").to_string_lossy().to_string();

    // 步驟1：取得影片資訊 (title, description, id)
    let info_output = tokio::process::Command::new(ytdlp)
        .args(["--dump-json", "--no-playlist", "--skip-download", url])
        .output()
        .await
        .map_err(|e| format!("yt-dlp info error: {}", e))?;

    let mut title = String::new();
    let mut description = String::new();
    let mut channel = String::new();
    let mut video_id = String::new();

    if !info_output.stdout.is_empty() {
        if let Ok(json_str) = String::from_utf8(info_output.stdout) {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&json_str) {
                title = info["title"].as_str().unwrap_or("").to_string();
                description = info["description"].as_str().unwrap_or("").to_string();
                channel = info["uploader"].as_str().unwrap_or("").to_string();
                video_id = info["id"].as_str().unwrap_or("").to_string();
                println!("[VIDEO_PARSER] 📹 Video: {} (id={})", title, video_id);
            }
        }
    }

    // 步驟2：下載字幕（優先用戶上傳字幕，失敗再用自動字幕）
    let _sub_output = tokio::process::Command::new(ytdlp)
        .args([
            "--write-sub",      // 嘗試下載用戶上傳的字幕
            "--write-auto-sub", // 也嘗試自動生成的字幕
            "--sub-langs",
            "zh-HK,zh-TW,zh,en",
            "--sub-format",
            "json3",
            "--skip-download",
            "--no-playlist",
            "-o",
            &output_template,
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp subtitle error: {}", e))?;

    // 步驟3：讀取生成的 json3 字幕檔案
    let transcript = if !video_id.is_empty() {
        let sub_paths = vec![
            temp_dir.join(format!("{}.zh-HK.json3", video_id)),
            temp_dir.join(format!("{}.zh-TW.json3", video_id)),
            temp_dir.join(format!("{}.zh.json3", video_id)),
            temp_dir.join(format!("{}.en.json3", video_id)),
        ];

        let mut found_transcript = String::new();
        for path in &sub_paths {
            if path.exists() {
                println!("[VIDEO_PARSER] 📄 Found subtitle file: {:?}", path);
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(events) = json["events"].as_array() {
                            let mut t = String::new();
                            for event in events {
                                if let Some(segs) = event["segs"].as_array() {
                                    for seg in segs {
                                        if let Some(text) = seg["utf8"].as_str() {
                                            let cleaned = text.replace('\n', " ");
                                            if !cleaned.trim().is_empty() {
                                                t.push_str(&cleaned);
                                                t.push(' ');
                                            }
                                        }
                                    }
                                }
                            }
                            found_transcript = t.trim().to_string();
                            let _ = std::fs::remove_file(path);
                            break;
                        }
                    }
                }
            }
        }
        found_transcript
    } else {
        String::new()
    };

    // 組合最終結果
    let mut result = format!(
        "【YouTube 影片】{}",
        if title.is_empty() {
            url.to_string()
        } else {
            title.clone()
        }
    );
    if !channel.is_empty() {
        result.push_str(&format!("\n頻道：{}", channel));
    }
    if !transcript.is_empty() {
        result.push_str(&format!("\n字幕內容：\n{}", transcript));
    } else if !description.is_empty() {
        // 沒有字幕時用 description 替代（最多 2000 字）
        let desc_preview: String = description.chars().take(2000).collect();
        result.push_str(&format!("\n影片描述：\n{}", desc_preview));
    }
    result.push_str(&format!("\n來源：{}", url));

    if title.is_empty() {
        return Err("yt-dlp could not extract video info".to_string());
    }

    Ok(result)
}

async fn fetch_youtube_metadata(url: &str) -> Result<String, String> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    let res = client
        .get(url)
        .header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
        .send()
        .await
        .map_err(|e| format!("Fetch error: {}", e))?;
    let html = res.text().await.map_err(|e| format!("Read error: {}", e))?;

    let title = html
        .find("<title>")
        .and_then(|s| {
            html[s + 7..]
                .find("</title>")
                .map(|e| html[s + 7..s + 7 + e].trim().to_string())
        })
        .unwrap_or_default()
        .replace(" - YouTube", "");

    let description = html
        .find("og:description")
        .and_then(|s| html[s..].find("content=\"").map(|c| s + c + 9))
        .and_then(|s| {
            html[s..]
                .find('"')
                .map(|e| html[s..s + e].trim().to_string())
        })
        .unwrap_or_default();

    let channel = html
        .find("\"ownerChannelName\":\"")
        .map(|s| {
            let rest = &html[s + 20..];
            rest[..rest.find('"').unwrap_or(50).min(50)].to_string()
        })
        .unwrap_or_default();

    if title.is_empty() {
        return Err("Could not extract YouTube page metadata".to_string());
    }

    let mut result = format!("【YouTube 影片】{}", title);
    if !channel.is_empty() {
        result.push_str(&format!("\n頻道：{}", channel));
    }
    if !description.is_empty() {
        result.push_str(&format!("\n影片描述：\n{}", description));
    }
    result.push_str(&format!("\n來源：{}", url));

    println!(
        "[VIDEO_PARSER] ✅ Extracted YouTube metadata ({} chars)",
        result.len()
    );
    Ok(result)
}

fn sanitize_subtitle_text(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    let mut in_tag = false;

    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => sanitized.push(c),
            _ => {}
        }
    }

    sanitized
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

// Bilibili extraction involves 2 steps: view API for CID -> player API for Subtitle Url -> Download
pub async fn fetch_bilibili_subtitles(
    bvid: &str,
    sessdata: Option<String>,
) -> Result<String, String> {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Client build error: {}", e))?;

    println!("[VIDEO_PARSER] Checking Bilibili BVID: {}", bvid);
    println!("[BILI DEBUG] Input BVID: {}", bvid);

    // 1. Get CID & Metadata
    let view_url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    println!("[BILI DEBUG] View URL: {}", view_url);
    let res = client
        .get(&view_url)
        .send()
        .await
        .map_err(|e| format!("View API Error: {}", e))?;
    let view_json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("JSON Parse Error: {}", e))?;

    if view_json["code"] != 0 {
        return Err(format!("Bilibili API Error: {}", view_json["message"]));
    }

    let title = view_json["data"]["title"]
        .as_str()
        .unwrap_or("Unknown Title");
    let cid = view_json["data"]["cid"]
        .as_i64()
        .ok_or("Could not find CID")?;
    let owner = view_json["data"]["owner"]["name"]
        .as_str()
        .unwrap_or("Unknown");

    println!("[BILI DEBUG] Title from API: {}", title);
    println!("[BILI DEBUG] CID: {}", cid);

    // 2. Get Subtitles List
    // 2. 使用 WBI 簽名請求 Player API（確保字幕資料正確對應）
    let sess_ref = sessdata.as_deref();
    let cid_str = cid.to_string();

    // 嘗試用 WBI 簽名的 wbi/v2 端點
    let player_json: serde_json::Value = match fetch_wbi_keys(&client, sess_ref).await {
        Ok((img_key, sub_key)) => {
            let mixin_key = gen_mixin_key(&img_key, &sub_key);
            let signed_query = wbi_sign(&[("bvid", bvid), ("cid", &cid_str)], &mixin_key);
            let player_url = format!("https://api.bilibili.com/x/player/wbi/v2?{}", signed_query);
            println!("[BILI DEBUG] WBI-signed Player URL: {}", player_url);

            let mut p_req = client
                .get(&player_url)
                .header("Origin", "https://www.bilibili.com");

            if let Some(ref sess) = sessdata {
                p_req = p_req.header("Cookie", format!("SESSDATA={}", sess));
            }

            let p_res = p_req
                .send()
                .await
                .map_err(|e| format!("Player API Error: {}", e))?;
            p_res
                .json()
                .await
                .map_err(|e| format!("JSON Parse Error: {}", e))?
        }
        Err(e) => {
            println!(
                "[BILI DEBUG] WBI key fetch failed: {}. Falling back to unsigned v2.",
                e
            );
            // 降級使用老的 v2 端點
            let player_url = format!(
                "https://api.bilibili.com/x/player/v2?bvid={}&cid={}",
                bvid, cid
            );
            let mut p_req = client
                .get(&player_url)
                .header(
                    "Referer",
                    format!("https://www.bilibili.com/video/{}", bvid),
                )
                .header("Origin", "https://www.bilibili.com");

            if let Some(ref sess) = sessdata {
                p_req = p_req.header("Cookie", format!("SESSDATA={}", sess));
            }

            let p_res = p_req
                .send()
                .await
                .map_err(|e| format!("Player API Error: {}", e))?;
            p_res
                .json()
                .await
                .map_err(|e| format!("JSON Parse Error: {}", e))?
        }
    };

    println!(
        "[BILI DEBUG] player API code: {}, subtitle count: {}",
        player_json["code"],
        player_json["data"]["subtitle"]["subtitles"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    );

    let mut transcript = String::new();
    let mut found_sub = false;

    // 2. 獲取字幕列表並進行優先級排序
    if let Some(subtitles) = player_json["data"]["subtitle"]["subtitles"].as_array() {
        if !subtitles.is_empty() {
            let mut best_sub = None;
            let mut highest_priority = 999; // Lower is better

            for sub in subtitles.iter() {
                if let (Some(lan), lan_doc_opt) = (sub["lan"].as_str(), sub["lan_doc"].as_str()) {
                    let lan_doc = lan_doc_opt.unwrap_or("");
                    let is_ai = lan.contains("ai")
                        || lan_doc.contains("AI")
                        || lan_doc.contains("自动")
                        || lan_doc.contains("自動");

                    let priority = if lan.starts_with("zh") && !is_ai {
                        1
                    } else if lan.starts_with("en") && !is_ai {
                        2
                    } else if lan.starts_with("zh") && is_ai {
                        3
                    } else if lan.contains("ai-zh") || lan.starts_with("ai-cn") {
                        4
                    } else if lan.contains("ai-en") {
                        5
                    } else {
                        6
                    };

                    if priority < highest_priority {
                        highest_priority = priority;
                        best_sub = Some(sub);
                    }
                }
            }

            let selected_sub = best_sub.or_else(|| subtitles.first());

            if let Some(best_sub) = selected_sub {
                if let Some(sub_url) = best_sub["subtitle_url"].as_str() {
                    let mut full_url = sub_url.to_string();
                    if full_url.starts_with("//") {
                        full_url = format!("https:{}", full_url);
                    }
                    println!("[BILI DEBUG] Subtitle URL: {}", full_url);

                    let lan = best_sub["lan"].as_str().unwrap_or("unknown");
                    println!("[VIDEO_PARSER] Select best subtitle track: {}", lan);

                    // 3. 下載字幕 JSON
                    if let Ok(sub_res) = client.get(&full_url).send().await {
                        if let Ok(sub_data) = sub_res.json::<serde_json::Value>().await {
                            if let Some(body) = sub_data["body"].as_array() {
                                for item in body {
                                    if let Some(text) = item["content"].as_str() {
                                        let cleaned = sanitize_subtitle_text(text);
                                        if !cleaned.is_empty() {
                                            transcript.push_str(&cleaned);
                                            transcript.push('\n'); // 使用換行符
                                        }
                                    }
                                }
                                found_sub = true;
                            }
                        }
                    }
                }
            }
        }
    }

    // ─── 字幕內容校驗（防止 B 站 CDN 回傳錯誤影片的字幕）───
    // 若為 AI 字幕，CDN 有機率回傳其他影片的快取內容。
    // 策略：取字幕前 N 句與標題/描述做關鍵詞交叉比對，若完全無關則丟棄。
    if found_sub && !transcript.trim().is_empty() {
        let desc = view_json["data"]["desc"].as_str().unwrap_or("");
        let title_lower = title.to_lowercase();
        let desc_lower = desc.to_lowercase();
        // 從標題和描述中提取具有語義的關鍵字（至少 2 個字元的片段）
        let reference_text = format!("{} {}", title_lower, desc_lower);

        // 取字幕的前 200 個字元作為樣本
        let sample: String = transcript.chars().take(200).collect();
        let sample_lower = sample.to_lowercase();

        // 用標題中的連續 2-4 字元片段去比對字幕
        let mut match_score = 0;
        let title_chars: Vec<char> = title_lower.chars().collect();
        for window_size in [4, 3, 2] {
            if title_chars.len() >= window_size {
                for chunk in title_chars.windows(window_size) {
                    let keyword: String = chunk.iter().collect();
                    // 跳過純標點或空白
                    if keyword
                        .chars()
                        .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
                    {
                        continue;
                    }
                    if sample_lower.contains(&keyword) {
                        match_score += window_size;
                    }
                }
            }
        }

        // 也用描述中的一些片段比對
        let desc_chars: Vec<char> = desc_lower.chars().collect();
        for window_size in [4, 3] {
            if desc_chars.len() >= window_size {
                for chunk in desc_chars.windows(window_size) {
                    let keyword: String = chunk.iter().collect();
                    if keyword
                        .chars()
                        .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
                    {
                        continue;
                    }
                    if sample_lower.contains(&keyword) {
                        match_score += 1; // 描述的權重較低
                    }
                }
            }
        }

        // 額外檢查：字幕本身是否提及了參考文字中的任何片段
        let sample_chars: Vec<char> = sample_lower.chars().collect();
        for window_size in [4, 3] {
            if sample_chars.len() >= window_size {
                for chunk in sample_chars.windows(window_size) {
                    let keyword: String = chunk.iter().collect();
                    if keyword
                        .chars()
                        .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
                    {
                        continue;
                    }
                    if reference_text.contains(&keyword) {
                        match_score += 1;
                    }
                }
            }
        }

        let title_preview: String = title.chars().take(50).collect();
        let sample_preview: String = sample.chars().take(60).collect();
        println!(
            "[VIDEO_PARSER] Subtitle validation: match_score={}, title='{}', sample='{}'",
            match_score, title_preview, sample_preview
        );

        if match_score == 0 {
            println!(
                "[VIDEO_PARSER] ⚠️ Subtitle content does NOT match video title/desc! Discarding as likely CDN cache error."
            );
            found_sub = false;
            transcript.clear();
        }
    }

    // 移除錯誤的 AI 字幕 Fallback 邏輯，AI 字幕已經在上面的 subtitles 列表中處理。

    let mut result = format!("【Bilibili 影片】{}", title);
    result.push_str(&format!("\nUP主：{}", owner));

    if found_sub && !transcript.trim().is_empty() {
        result.push_str("\n字幕內容：\n");
        result.push_str(transcript.trim());
    } else {
        let desc = view_json["data"]["desc"].as_str().unwrap_or("");
        result.push_str("\n(此影片未提供字幕) 影片描述：\n");
        result.push_str(desc);
    }

    result.push_str(&format!("\n來源：https://www.bilibili.com/video/{}", bvid));

    println!(
        "[VIDEO_PARSER] ✅ Extracted Bilibili info ({} chars)",
        result.len()
    );
    Ok(result)
}

/// 解析 URL 內容（YouTube 字幕 / Bilibili 字幕 / 一般網頁）
/// 由 file_parser::parse_content 委派呼叫
pub async fn parse_url_content(
    url_str: &str,
    sessdata: Option<String>,
) -> Result<ParsedDocument, String> {
    // YouTube
    if url_str.contains("youtube.com/watch") || url_str.contains("youtu.be/") {
        match fetch_youtube_subtitles(url_str).await {
            Ok(content) if !content.trim().is_empty() => {
                return Ok(ParsedDocument {
                    chunks: vec![FileChunk {
                        content,
                        chunk_type: "document".to_string(),
                        image_path: None,
                        status: "processed".to_string(),
                    }],
                    title: format!("YouTube Video: {}", url_str),
                });
            }
            Ok(_) => return Err("YouTube content extraction returned empty".to_string()),
            Err(e) => return Err(e),
        }
    }

    // Bilibili
    if url_str.contains("bilibili.com/video/") {
        if let Some(start) = url_str.find("/video/") {
            let rest = &url_str[start + 7..];
            let bvid = rest
                .split('/')
                .next()
                .unwrap_or("")
                .split('?')
                .next()
                .unwrap_or("");
            if !bvid.is_empty() {
                let transcript = fetch_bilibili_subtitles(bvid, sessdata).await?;
                return Ok(ParsedDocument {
                    chunks: vec![FileChunk {
                        content: transcript,
                        chunk_type: "document".to_string(),
                        image_path: None,
                        status: "processed".to_string(),
                    }],
                    title: format!("Bilibili Video: {}", bvid),
                });
            }
        }
    }

    // General Web Page
    crate::capture::readability::scrape_url(url_str).await
}

/// 舊版統一入口，轉發到 file_parser::parse_content（保持 backward compat）
pub async fn parse_temp_content(
    kb_path: &str,
    file_path: Option<String>,
    url: Option<String>,
    sessdata: Option<String>,
) -> Result<ParsedDocument, String> {
    crate::capture::file_parser::parse_content(kb_path, file_path, url, sessdata).await
}

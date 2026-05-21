use crate::capture::file_parser::{FileChunk, ParsedDocument};
use md5::{Digest, Md5};
use reqwest::Client;
use std::path::Path;

const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

fn extract_key_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

fn gen_mixin_key(img_key: &str, sub_key: &str) -> String {
    let raw = format!("{}{}", img_key, sub_key);
    let raw_chars: Vec<char> = raw.chars().collect();
    let mixin: String = MIXIN_KEY_ENC_TAB
        .iter()
        .filter_map(|&i| raw_chars.get(i).copied())
        .collect();
    mixin.chars().take(32).collect()
}

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
    let img_key_preview: String = img_key.chars().take(8).collect();
    let sub_key_preview: String = sub_key.chars().take(8).collect();

    println!(
        "[WBI] Got keys: img_key={}, sub_key={}",
        img_key_preview, sub_key_preview
    );

    Ok((img_key, sub_key))
}

fn wbi_sign(params: &[(&str, &str)], mixin_key: &str) -> String {
    let wts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let mut all_params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    all_params.push(("wts".to_string(), wts.clone()));
    all_params.sort_by(|a, b| a.0.cmp(&b.0));

    let filtered: Vec<_> = all_params
        .iter()
        .filter(|(_, v)| !v.chars().any(|c| matches!(c, '!' | '\'' | '(' | ')' | '*')))
        .collect();

    let query: String = filtered
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    let to_hash = format!("{}{}", query, mixin_key);
    let mut hasher = Md5::new();
    hasher.update(to_hash.as_bytes());
    let w_rid = format!("{:x}", hasher.finalize());

    format!("{}&w_rid={}", query, w_rid)
}

fn find_ytdlp() -> Option<std::path::PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(d) = dir {
                for candidate in [d.join("yt-dlp.exe"), d.join("resources").join("yt-dlp.exe")] {
                    if candidate.exists() {
                        println!("[VIDEO_PARSER] Found yt-dlp at: {:?}", candidate);
                        return Some(candidate);
                    }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }
    if crate::utils::hidden_command::std_command("yt-dlp")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some(std::path::PathBuf::from("yt-dlp"));
    }
    None
}

pub async fn fetch_youtube_subtitles(url: &str) -> Result<String, String> {
    if let Some(ytdlp) = find_ytdlp() {
        println!("[VIDEO_PARSER]    yt-dlp found at {:?}", ytdlp);
        match fetch_with_ytdlp(&ytdlp, url).await {
            Ok(text) if !text.trim().is_empty() => {
                println!("[VIDEO_PARSER]   yt-dlp extracted: {} chars", text.len());
                return Ok(text);
            }
            Ok(_) => println!("[VIDEO_PARSER]    yt-dlp returned empty, falling back..."),
            Err(e) => println!("[VIDEO_PARSER]    yt-dlp failed: {}, falling back...", e),
        }
    } else {
        println!("[VIDEO_PARSER]    yt-dlp not found, using HTTP fallback.");
    }

    fetch_youtube_metadata(url).await
}

async fn fetch_with_ytdlp(ytdlp: &std::path::Path, url: &str) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("insightcap_yt");
    let _ = std::fs::create_dir_all(&temp_dir);
    let output_template = temp_dir.join("%(id)s").to_string_lossy().to_string();

    let ffmpeg_path = ytdlp.parent().unwrap_or(Path::new(".")).join("ffmpeg.exe");

    // 合併 --dump-json 和 --write-sub 為單次 yt-dlp 調用，避免重複下載網頁
    let combined_output = crate::utils::hidden_command::tokio_command(ytdlp)
        .args([
            "--dump-json",
            "--write-sub",
            "--write-auto-sub",
            "--sub-langs",
            "zh-HK,zh-TW,zh-Hans,zh,en.*,en",
            "--sub-format",
            "json3",
            "--skip-download",
            "--no-playlist",
            "--ffmpeg-location",
            ffmpeg_path.to_str().unwrap_or("ffmpeg"),
            "--js-runtimes",
            "node",
            "-o",
            &output_template,
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp error: {}", e))?;

    let mut title = String::new();
    let mut description = String::new();
    let mut channel = String::new();
    let mut video_id = String::new();

    if !combined_output.stdout.is_empty() {
        if let Ok(json_str) = String::from_utf8(combined_output.stdout) {
            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&json_str) {
                title = info["title"].as_str().unwrap_or("").to_string();
                description = info["description"].as_str().unwrap_or("").to_string();
                channel = info["uploader"].as_str().unwrap_or("").to_string();
                video_id = info["id"].as_str().unwrap_or("").to_string();
                println!("[VIDEO_PARSER]    Video: {} (id={})", title, video_id);
            }
        }
    }

    let transcript = if !video_id.is_empty() {
        let sub_paths = vec![
            temp_dir.join(format!("{}.zh-HK.json3", video_id)),
            temp_dir.join(format!("{}.zh-TW.json3", video_id)),
            temp_dir.join(format!("{}.zh-Hans.json3", video_id)),
            temp_dir.join(format!("{}.zh.json3", video_id)),
            temp_dir.join(format!("{}.zh-HK.vtt.json3", video_id)),
            temp_dir.join(format!("{}.en.json3", video_id)),
        ];

        let mut found_transcript = String::new();
        for path in &sub_paths {
            if path.exists() {
                println!("[VIDEO_PARSER]    Found subtitle file: {:?}", path);
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(events) = json["events"].as_array() {
                            let mut t = String::new();
                            for event in events {
                                if let Some(segs) = event["segs"].as_array() {
                                    let mut event_text = String::new();
                                    for seg in segs {
                                        if let Some(text) = seg["utf8"].as_str() {
                                            if text == "\n" {
                                                if !event_text.trim().is_empty() {
                                                    t.push_str(event_text.trim());
                                                    t.push('\n');
                                                    event_text.clear();
                                                }
                                            } else {
                                                let cleaned = text.trim();
                                                if !cleaned.is_empty() {
                                                    event_text.push_str(cleaned);
                                                }
                                            }
                                        }
                                    }
                                    if !event_text.trim().is_empty() {
                                        t.push_str(event_text.trim());
                                        t.push('\n');
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

    let mut result = format!(
        " YouTube    {}",
        if title.is_empty() {
            url.to_string()
        } else {
            title.clone()
        }
    );
    if !channel.is_empty() {
        result.push_str(&format!("\n   {}", channel));
    }
    if !transcript.is_empty() {
        let coalesced = coalesce_subtitle_lines(&transcript);
        result.push_str(&format!("\n字幕内容\n{}", coalesced));
    } else if let Some(whisper_text) = try_transcribe_with_whisper(ytdlp, url, &video_id).await {
        result.push_str("\n字幕內容 (Whisper 轉錄)\n");
        result.push_str(&whisper_text);
    } else if !description.is_empty() {
        result.push_str(&format!("\n描述\n{}", description));
    }
    result.push_str(&format!("\n   {}", url));

    if title.is_empty() {
        return Err("yt-dlp could not extract video info".to_string());
    }

    Ok(result)
}

async fn try_transcribe_with_whisper(ytdlp: &Path, url: &str, video_id: &str) -> Option<String> {
    if video_id.is_empty() {
        return None;
    }

    let model = get_preferred_whisper_model();
    if !crate::whisper_transcribe::is_model_downloaded(model) {
        println!(
            "[VIDEO_PARSER]   Whisper model ({}) not downloaded, skipping.",
            model.filename()
        );
        return None;
    }

    match crate::whisper_transcribe::transcribe_video(ytdlp, url, video_id, model).await {
        Ok(transcript) if !transcript.trim().is_empty() => Some(transcript),
        Ok(_) => {
            println!("[VIDEO_PARSER]   Whisper returned empty transcript");
            None
        }
        Err(error) => {
            println!("[VIDEO_PARSER]   Whisper failed: {}", error);
            None
        }
    }
}

pub fn parse_preferred_whisper_model_name(content: &str) -> Option<&str> {
    let json = serde_json::from_str::<serde_json::Value>(content).ok()?;
    match json["model"].as_str() {
        Some("tiny") => Some("tiny"),
        Some("base") => Some("base"),
        Some("small") => Some("small"),
        Some("medium") => Some("medium"),
        _ => None,
    }
}

fn get_preferred_whisper_model() -> crate::whisper_transcribe::WhisperModel {
    crate::whisper_transcribe::read_model_preference()
}

#[tauri::command]
pub fn set_whisper_model_preference(model_name: String) -> Result<(), String> {
    let model = crate::whisper_transcribe::WhisperModel::from_name(&model_name)
        .ok_or_else(|| format!("Unknown model: {}", model_name))?;
    crate::whisper_transcribe::write_model_preference(model)
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

    let mut result = format!(" YouTube    {}", title);
    if !channel.is_empty() {
        result.push_str(&format!("\n   {}", channel));
    }
    if !description.is_empty() {
        result.push_str(&format!("\n     \n{}", description));
    }
    result.push_str(&format!("\n   {}", url));

    println!(
        "[VIDEO_PARSER]   Extracted YouTube metadata ({} chars)",
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

/// 判斷字元是否為 CJK（中日韓）文字。
/// CJK 字元之間不需要空格分隔。
fn is_cjk_char(c: char) -> bool {
    let u = c as u32;
    // CJK Unified Ideographs, Hiragana, Katakana
    (u >= 0x4E00 && u <= 0x9FFF)
        || (u >= 0x3040 && u <= 0x30FF)
        // CJK Extension A
        || (u >= 0x3400 && u <= 0x4DBF)
}

/// 將一行行的短字幕合併成多個段落，並去除真正的重複內容。
///
/// 去重策略：
/// - 完全相同的相鄰行直接跳過（滾動字幕的逐字更新）
/// - 「上一行包含此行」只在此行夠長（>= MIN_OVERLAP_CHARS）時才視為重複。
///   不設門檻的話，短片段（如「的」「了」「是」）極易被誤判為包含在上一行中，
///   導致大量正常字幕被錯誤丟棄。
///   範例誤殺：prev="今天我們來討論這個話題", t="來討論這個話題的重要性"
///   → t 的後半不在 prev 中，不應跳過，但舊邏輯會跳過。
fn coalesce_subtitle_lines(raw: &str) -> String {
    const MAX_PARA_CHARS: usize = 300;
    // 只有當重複字串長度達此門檻，才視為滾動式重複而跳過
    const MIN_OVERLAP_CHARS: usize = 6;

    let mut deduped: Vec<String> = Vec::new();
    let mut prev = String::new();

    for line in raw.lines() {
        let t = line.trim().to_string();
        if t.is_empty() {
            continue;
        }
        // 完全相同的相鄰行跳過
        if t == prev {
            continue;
        }
        // 只有 t 夠長，且上一行完整包含 t 時，才視為滾動重複跳過
        if t.chars().count() >= MIN_OVERLAP_CHARS && prev.contains(t.as_str()) {
            continue;
        }

        deduped.push(t.clone());
        prev = t;
    }

    // 將短行展開為連續文字，每 MAX_PARA_CHARS 換行
    let mut result = String::new();
    let mut current_para = String::new();

    for segment in deduped {
        if current_para.is_empty() {
            current_para.push_str(&segment);
        } else {
            // 假如上一節末尾是句号結尾，直接換行
            let ends_sentence = current_para
                .chars()
                .last()
                .map(|c| matches!(c, '.' | '!' | '?' | '。' | '！' | '？'))
                .unwrap_or(false);

            if current_para.chars().count() >= MAX_PARA_CHARS || ends_sentence {
                result.push_str(&current_para);
                result.push('\n');
                current_para = segment;
            } else {
                // 將短行展開為連續文字
                // CJK 字元之間不需要空格，CJK↔拉丁之間需要空格
                let prev_is_cjk = current_para
                    .chars()
                    .last()
                    .map(is_cjk_char)
                    .unwrap_or(false);
                let next_is_cjk = segment.chars().next().map(is_cjk_char).unwrap_or(false);
                if prev_is_cjk && next_is_cjk {
                    current_para.push_str(&segment);
                } else {
                    current_para.push(' ');
                    current_para.push_str(&segment);
                }
            }
        }
    }

    if !current_para.is_empty() {
        result.push_str(&current_para);
    }

    result.trim().to_string()
}

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

    let view_url = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    println!("[BILI DEBUG] View URL: {}", view_url);

    let sess_ref = sessdata.as_deref();

    // View API 和 Nav API（WBI keys）互相獨立，並行調用以減少延遲
    let (view_result, wbi_result) = tokio::join!(
        async {
            let res = client
                .get(&view_url)
                .send()
                .await
                .map_err(|e| format!("View API Error: {}", e))?;
            res.json::<serde_json::Value>()
                .await
                .map_err(|e| format!("JSON Parse Error: {}", e))
        },
        fetch_wbi_keys(&client, sess_ref)
    );

    let view_json = view_result?;
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

    let cid_str = cid.to_string();

    let player_json: serde_json::Value = match wbi_result {
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

    if let Some(subtitles) = player_json["data"]["subtitle"]["subtitles"].as_array() {
        if !subtitles.is_empty() {
            let mut best_sub = None;
            let mut highest_priority = 999; // Lower is better

            for sub in subtitles.iter() {
                if let (Some(lan), lan_doc_opt) = (sub["lan"].as_str(), sub["lan_doc"].as_str()) {
                    let lan_doc = lan_doc_opt.unwrap_or("");
                    let is_ai = lan.contains("ai")
                        || lan_doc.contains("AI")
                        || lan_doc.contains("  ")
                        || lan_doc.contains("  ");

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

                    if let Ok(sub_res) = client.get(&full_url).send().await {
                        if let Ok(sub_data) = sub_res.json::<serde_json::Value>().await {
                            if let Some(body) = sub_data["body"].as_array() {
                                for item in body {
                                    if let Some(text) = item["content"].as_str() {
                                        let cleaned = sanitize_subtitle_text(text);
                                        if !cleaned.is_empty() {
                                            transcript.push_str(&cleaned);
                                            transcript.push('\n'); //
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

    if found_sub && !transcript.trim().is_empty() {
        let desc = view_json["data"]["desc"].as_str().unwrap_or("");
        let title_lower = title.to_lowercase();
        let desc_lower = desc.to_lowercase();
        let reference_text = format!("{} {}", title_lower, desc_lower);

        let sample: String = transcript.chars().take(200).collect();
        let sample_lower = sample.to_lowercase();

        let mut match_score = 0;
        let title_chars: Vec<char> = title_lower.chars().collect();
        for window_size in [4, 3, 2] {
            if title_chars.len() >= window_size {
                for chunk in title_chars.windows(window_size) {
                    let keyword: String = chunk.iter().collect();
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
                        match_score += 1; //
                    }
                }
            }
        }

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
                "[VIDEO_PARSER]    Subtitle content does NOT match video title/desc! Discarding as likely CDN cache error."
            );
            found_sub = false;
            transcript.clear();
        }
    }

    let mut result = format!(" Bilibili    {}", title);
    result.push_str(&format!("\nUP  {}", owner));

    if found_sub && !transcript.trim().is_empty() {
        let coalesced = coalesce_subtitle_lines(transcript.trim());
        result.push_str("\n字幕內容\n");
        result.push_str(&coalesced);
    } else {
        let desc = view_json["data"]["desc"].as_str().unwrap_or("");
        result.push_str("\n(無字幕，以下為影片簡介)\n");
        result.push_str(desc);
    }

    result.push_str(&format!("\n   https://www.bilibili.com/video/{}", bvid));

    println!(
        "[VIDEO_PARSER]   Extracted Bilibili info ({} chars)",
        result.len()
    );
    Ok(result)
}

fn clean_inline_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 從 URL 產生可讀標題（fallback 用）
pub fn readable_title_from_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "Untitled URL".to_string();
    }

    let without_scheme = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let host_and_path = without_scheme.split('#').next().unwrap_or(without_scheme);
    let host_and_path = host_and_path.split('?').next().unwrap_or(host_and_path);
    let mut parts = host_and_path.splitn(2, '/');
    let host = parts.next().unwrap_or("").trim();
    let path = parts.next().unwrap_or("").trim();

    if host.is_empty() {
        return trimmed.to_string();
    }

    let slug = path
        .split('/')
        .filter(|s| !s.is_empty())
        .next_back()
        .unwrap_or("")
        .replace(['-', '_'], " ");
    let slug = clean_inline_text(&slug);

    if slug.is_empty() {
        host.to_string()
    } else {
        format!("{} | {}", slug, host)
    }
}

/// 從 video_parser 輸出的格式化內容中提取影片標題
pub fn extract_video_title(content: &str, platform: &str) -> Option<String> {
    let platform_lower = platform.to_lowercase();
    let lines: Vec<String> = content
        .lines()
        .map(clean_inline_text)
        .filter(|line| !line.is_empty())
        .collect();

    for (idx, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        if !line_lower.starts_with(&platform_lower) {
            continue;
        }

        let mut title = line[platform.len()..].trim().to_string();
        if title.is_empty() {
            continue;
        }

        if let Some(channel) = lines.get(idx + 1) {
            let is_url = channel.starts_with("http://") || channel.starts_with("https://");
            if !is_url && channel.len() <= 80 {
                title = format!("{} | {}", title, channel);
            }
        }
        return Some(clean_inline_text(&title));
    }

    None
}

pub async fn parse_url_content(
    url_str: &str,
    sessdata: Option<String>,
) -> Result<ParsedDocument, String> {
    if url_str.contains("youtube.com/watch") || url_str.contains("youtu.be/") {
        match fetch_youtube_subtitles(url_str).await {
            Ok(content) if !content.trim().is_empty() => {
                let title = extract_video_title(&content, "YouTube")
                    .unwrap_or_else(|| readable_title_from_url(url_str));
                return Ok(ParsedDocument {
                    chunks: vec![FileChunk {
                        content,
                        chunk_type: "document".to_string(),
                        source_type: "youtube_subtitle".to_string(),
                        metadata: serde_json::json!({
                            "source_type": "youtube_subtitle",
                            "source_url": url_str,
                        }),
                        image_path: None,
                        status: "processed".to_string(),
                    }],
                    title,
                });
            }
            Ok(_) => return Err("YouTube content extraction returned empty".to_string()),
            Err(e) => return Err(e),
        }
    }

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
                let title = extract_video_title(&transcript, "Bilibili")
                    .unwrap_or_else(|| readable_title_from_url(url_str));
                return Ok(ParsedDocument {
                    chunks: vec![FileChunk {
                        content: transcript,
                        chunk_type: "document".to_string(),
                        source_type: "bilibili_subtitle".to_string(),
                        metadata: serde_json::json!({
                            "source_type": "bilibili_subtitle",
                            "source_url": url_str,
                            "video_id": bvid,
                        }),
                        image_path: None,
                        status: "processed".to_string(),
                    }],
                    title,
                });
            }
        }
    }

    crate::capture::readability::scrape_url(url_str).await
}

pub async fn parse_temp_content(
    kb_path: &str,
    file_path: Option<String>,
    url: Option<String>,
    sessdata: Option<String>,
) -> Result<ParsedDocument, String> {
    crate::capture::file_parser::parse_content(kb_path, file_path, url, sessdata, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_preferred_whisper_model_name_from_config_json() {
        assert_eq!(
            parse_preferred_whisper_model_name(r#"{"model":"small"}"#),
            Some("small")
        );
        assert_eq!(
            parse_preferred_whisper_model_name(r#"{"model":"medium"}"#),
            Some("medium")
        );
        assert_eq!(
            parse_preferred_whisper_model_name(r#"{"model":"ggml-small"}"#),
            None
        );
    }

    #[test]
    fn formats_whisper_timestamp_in_srt_style() {
        assert_eq!(
            crate::whisper_transcribe::format_timestamp_ms(3_723_045),
            "01:02:03.045"
        );
    }
}

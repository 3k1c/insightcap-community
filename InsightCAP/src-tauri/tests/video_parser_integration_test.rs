/// 整合測試：驗證影片解析完整流程
///
/// 執行方式：cargo test --test video_parser_integration_test -- --nocapture
///
/// 測試目標：https://www.youtube.com/watch?v=Y-xp1CglqMA（無內建字幕的影片）
///
/// 預期路徑：
///   1. YouTube → find_ytdlp() → fetch_with_ytdlp()
///   2. 合併的 yt-dlp 調用（--dump-json + --write-sub --write-auto-sub）
///   3. 若無字幕檔案 → try_transcribe_with_whisper（需 whisper-cli + model）
///   4. 若 Whisper 不可用 → 使用 description 作為 fallback
///   5. 若 yt-dlp 不存在 → fetch_youtube_metadata（HTTP fallback）

#[cfg(test)]
mod integration_tests {
    use std::time::Instant;

    /// 測試完整的 YouTube 解析流程（需要 yt-dlp）
    #[tokio::test]
    async fn test_youtube_parse_no_subtitles() {
        let url = "https://www.youtube.com/watch?v=Y-xp1CglqMA";
        println!("\n===== 開始測試 =====");
        println!("URL: {}", url);
        println!("預期：影片預設無字幕，需走 Whisper fallback 或 HTTP 降級\n");

        let start = Instant::now();

        let result = insightcap_lib::capture::video_parser::parse_url_content(url, None).await;

        let elapsed = start.elapsed();
        println!("\n耗時：{:.2} 秒", elapsed.as_secs_f64());

        match &result {
            Ok(doc) => {
                println!("\n✅ 解析成功");
                println!("標題：{}", doc.title);
                println!("chunk 數量：{}", doc.chunks.len());
                for (i, chunk) in doc.chunks.iter().enumerate() {
                    println!("\n--- chunk {} ---", i);
                    println!("類型：{}", chunk.source_type);
                    println!("內容預覽（前 500 字）：\n{}",
                        &chunk.content.chars().take(500).collect::<String>());
                    if chunk.content.len() > 500 {
                        println!("...（總共 {} 字）", chunk.content.len());
                    }
                }
            }
            Err(e) => {
                println!("\n❌ 解析失敗：{}", e);
            }
        }

        // 不強制 assert 成功（取決於環境是否有 yt-dlp / whisper-cli）
        // 但至少要能跑完不 panic
    }

    /// 測試標題提取
    #[test]
    fn test_extract_video_title() {
        // 模擬 fetch_youtube_subtitles 的回傳格式
        let content = " YouTube    Test Video Title\n   Test Channel\n字幕内容\nThis is the transcript.\n   https://www.youtube.com/watch?v=test";

        let title = insightcap_lib::capture::video_parser::extract_video_title(content, "YouTube");
        assert_eq!(title, Some("Test Video Title | Test Channel".to_string()));
    }

    /// 測試 Bilibili BVID 提取
    #[test]
    fn test_extract_bvid_from_url() {
        // 模擬 URL 中的 BVID 解析邏輯
        let url = "https://www.bilibili.com/video/BV1xx411c7mD/";
        let start = url.find("/video/").unwrap();
        let rest = &url[start + 7..];
        let bvid: String = rest
            .split('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("")
            .to_string();
        assert_eq!(bvid, "BV1xx411c7mD");
    }

    /// 診斷：印出所有模型的下載狀態與路徑
    #[test]
    fn debug_whisper_model_paths() {
        let status = insightcap_lib::whisper_transcribe::whisper_model_status();
        println!("\n===== Whisper 模型診斷 =====");
        println!("current_exe: {:?}", std::env::current_exe().ok());
        println!("model_dir: {:?}", insightcap_lib::whisper_transcribe::model_dir());
        println!("config_path: {:?}", insightcap_lib::whisper_transcribe::config_path());
        println!("preferred model: {} ({})",
            insightcap_lib::whisper_transcribe::read_model_preference().config_name(),
            insightcap_lib::whisper_transcribe::read_model_preference().filename(),
        );
        println!("status: {}", status);
    }

    /// 測試 URL 可讀標題產生
    #[test]
    fn test_readable_title_from_url() {
        let title = insightcap_lib::capture::video_parser::readable_title_from_url(
            "https://www.youtube.com/watch?v=Y-xp1CglqMA",
        );
        assert!(!title.is_empty());
        println!("Fallback title: {}", title);
    }

    /// 計時測試：yt-dlp 合併調用的效能
    #[tokio::test]
    async fn benchmark_ytdlp_single_call() {
        let url = "https://www.youtube.com/watch?v=Y-xp1CglqMA";

        // 直接調用 fetch_youtube_subtitles（內部會走 find_ytdlp + fetch_with_ytdlp）
        let start = Instant::now();
        let result = insightcap_lib::capture::video_parser::fetch_youtube_subtitles(url).await;
        let elapsed = start.elapsed();

        println!("\n===== 效能測試 =====");
        println!("fetch_youtube_subtitles 耗時：{:.2} 秒", elapsed.as_secs_f64());

        match result {
            Ok(content) => {
                println!("內容長度：{} 字", content.len());
                // 檢查是否成功取得字幕內容
                if content.contains("字幕内容") || content.contains("字幕內容") {
                    println!("✅ 成功取得字幕");
                } else if content.contains("描述") {
                    println!("⚠️  只有描述（無字幕）");
                } else {
                    println!("⚠️  未知內容格式");
                }
            }
            Err(e) => {
                println!("❌ 失敗：{}", e);
            }
        }
    }
}

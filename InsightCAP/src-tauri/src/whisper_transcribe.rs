use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
}

impl WhisperModel {
    pub fn download_url(&self) -> &'static str {
        match self {
            Self::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            Self::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            Self::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"
            }
            Self::Medium => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin"
            }
        }
    }

    pub fn filename(&self) -> &'static str {
        match self {
            Self::Tiny => "ggml-tiny.bin",
            Self::Base => "ggml-base.bin",
            Self::Small => "ggml-small.bin",
            Self::Medium => "ggml-medium.bin",
        }
    }

    pub fn config_name(&self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Base => "base",
            Self::Small => "small",
            Self::Medium => "medium",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tiny => "Tiny (75 MB)",
            Self::Base => "Base (142 MB)",
            Self::Small => "Small (466 MB)",
            Self::Medium => "Medium (1.5 GB)",
        }
    }

    pub fn default_model() -> Self {
        Self::Medium
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "tiny" => Some(Self::Tiny),
            "base" => Some(Self::Base),
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            _ => None,
        }
    }
}

fn app_data_dir_from_env(local: Option<&Path>, home: Option<&Path>) -> PathBuf {
    if let Some(local) = local {
        return local.join("com.insightcap.app");
    }
    if let Some(home) = home {
        return home.join(".insightcap");
    }
    std::env::temp_dir().join("InsightCAP")
}

fn app_data_dir() -> PathBuf {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    app_data_dir_from_env(local.as_deref(), home.as_deref())
}

pub fn model_dir() -> PathBuf {
    let dir = app_data_dir().join("models");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("models").join("whisper_config.json")
}

pub fn model_path(model: WhisperModel) -> PathBuf {
    model_dir().join(model.filename())
}

pub fn is_model_downloaded(model: WhisperModel) -> bool {
    let path = model_path(model);
    if !path.exists() {
        return false;
    }

    let min_bytes = match model {
        WhisperModel::Tiny => 70 * 1024 * 1024,
        WhisperModel::Base => 130 * 1024 * 1024,
        WhisperModel::Small => 400 * 1024 * 1024,
        WhisperModel::Medium => 1400 * 1024 * 1024,
    };

    std::fs::metadata(path)
        .map(|metadata| metadata.len() >= min_bytes)
        .unwrap_or(false)
}

pub fn read_model_preference() -> WhisperModel {
    let config = std::fs::read_to_string(config_path()).ok();
    let preferred = config
        .as_deref()
        .and_then(crate::capture::video_parser::parse_preferred_whisper_model_name)
        .and_then(WhisperModel::from_name);

    // 若設定的模型已下載，直接使用
    if let Some(ref model) = preferred {
        if is_model_downloaded(*model) {
            return *model;
        }
    }

    // 找第一個已下載的模型（從小到大）
    for model in &[
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
    ] {
        if is_model_downloaded(*model) {
            return *model;
        }
    }

    // 全部未下載，回傳使用者設定值或預設值
    preferred.unwrap_or_else(WhisperModel::default_model)
}

pub fn write_model_preference(model: WhisperModel) -> Result<(), String> {
    let json = serde_json::json!({ "model": model.config_name() });
    std::fs::write(config_path(), json.to_string())
        .map_err(|e| format!("Failed to save whisper config: {}", e))
}

fn delete_model_file_at_path(path: &Path, model: WhisperModel) -> Result<String, String> {
    if !path.exists() {
        return Ok(format!("{} is not downloaded", model.filename()));
    }

    std::fs::remove_file(path)
        .map_err(|e| format!("Failed to delete {}: {}", model.filename(), e))?;
    Ok(format!("{} deleted successfully", model.filename()))
}

pub fn delete_model(model: WhisperModel) -> Result<String, String> {
    delete_model_file_at_path(&model_path(model), model)
}

pub async fn download_model(
    model: WhisperModel,
    progress_callback: impl Fn(u64, u64) + Send + 'static,
) -> Result<PathBuf, String> {
    let dest = model_path(model);
    if is_model_downloaded(model) {
        return Ok(dest);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let response = client
        .get(model.download_url())
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download failed with HTTP {}: {}",
            response.status(),
            model.download_url()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let tmp_dest = dest.with_extension("bin.tmp");

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let mut file = tokio::fs::File::create(&tmp_dest)
        .await
        .map_err(|e| format!("Cannot create temp file {:?}: {}", tmp_dest, e))?;

    let mut stream = response.bytes_stream();
    let mut downloaded = 0_u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream error: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }

    file.flush()
        .await
        .map_err(|e| format!("Flush error: {}", e))?;
    drop(file);

    tokio::fs::rename(&tmp_dest, &dest)
        .await
        .map_err(|e| format!("Rename error: {}", e))?;

    Ok(dest)
}

pub async fn download_audio_as_wav(
    ytdlp: &Path,
    url: &str,
    output_dir: &Path,
    video_id: &str,
) -> Result<PathBuf, String> {
    let unique_id = uuid::Uuid::now_v7().to_string();
    let file_id = format!("{}_{}", video_id, unique_id);
    let wav_path = output_dir.join(format!("{}.wav", file_id));
    let ffmpeg_path = ytdlp.parent().unwrap_or(Path::new(".")).join("ffmpeg.exe");

    let output = crate::utils::hidden_command::tokio_command(ytdlp)
        .args([
            "--no-playlist",
            "-f",
            "bestaudio",
            "-x",
            "--audio-format",
            "wav",
            "--ffmpeg-location",
            ffmpeg_path.to_str().unwrap_or("ffmpeg"),
            "--js-runtimes",
            "node",
            "-o",
            &output_dir
                .join(format!("{}.%(ext)s", file_id))
                .to_string_lossy(),
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp audio download error: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp audio failed: {}", stderr));
    }

    if !wav_path.exists() {
        return Err("yt-dlp did not produce expected audio file".to_string());
    }

    // yt-dlp 下載的可能是 webm/m4a，用 ffmpeg 轉成 16kHz mono WAV
    let converted_path = output_dir.join(format!("{}_16k.wav", file_id));
    let ffmpeg_output = crate::utils::hidden_command::tokio_command(&ffmpeg_path)
        .args([
            "-i",
            wav_path.to_str().unwrap_or("audio"),
            "-ar",
            "16000",
            "-ac",
            "1",
            "-y",
            converted_path.to_str().unwrap_or("out.wav"),
        ])
        .output()
        .await
        .map_err(|e| format!("ffmpeg conversion error: {}", e))?;

    if !ffmpeg_output.status.success() {
        let stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        return Err(format!("ffmpeg conversion failed: {}", stderr));
    }

    let _ = std::fs::remove_file(&wav_path);
    Ok(converted_path)
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

struct WhisperCliOutput {
    text: String,
    segments: Vec<TranscriptSegment>,
}

pub async fn transcribe_wav(
    wav_path: &Path,
    model: WhisperModel,
    language: Option<&str>,
) -> Result<Vec<TranscriptSegment>, String> {
    let output = run_whisper_cli(wav_path, model, language).await?;
    if !output.segments.is_empty() {
        return Ok(output.segments);
    }
    if output.text.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![TranscriptSegment {
        start_ms: 0,
        end_ms: 0,
        text: output.text,
    }])
}

pub async fn transcribe_audio_file(
    audio_path: &Path,
    model: WhisperModel,
    language: Option<&str>,
) -> Result<Vec<TranscriptSegment>, String> {
    let ext = audio_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if whisper_cli_supports_audio_ext(&ext) {
        return transcribe_wav(audio_path, model, language).await;
    }

    let ffmpeg = find_ffmpeg().ok_or(
        "Cannot find runnable ffmpeg. Place a working ffmpeg.exe next to the app or install it in PATH.",
    )?;
    let temp_dir = std::env::temp_dir().join("insightcap_audio_import");
    let _ = std::fs::create_dir_all(&temp_dir);
    let wav_path = temp_dir.join(format!("{}.wav", uuid::Uuid::now_v7()));

    let output = crate::utils::hidden_command::tokio_command(&ffmpeg)
        .args([
            "-i",
            audio_path.to_str().unwrap_or("audio"),
            "-ar",
            "16000",
            "-ac",
            "1",
            "-y",
            wav_path.to_str().unwrap_or("out.wav"),
        ])
        .output()
        .await
        .map_err(|e| format!("ffmpeg conversion error: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg conversion failed: {}", stderr.trim()));
    }

    let result = transcribe_wav(&wav_path, model, language).await;
    let _ = std::fs::remove_file(&wav_path);
    result
}

fn whisper_cli_supports_audio_ext(ext: &str) -> bool {
    matches!(ext, "wav")
}

pub async fn transcribe_video(
    ytdlp: &Path,
    url: &str,
    video_id: &str,
    model: WhisperModel,
) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("insightcap_whisper_audio");
    let _ = std::fs::create_dir_all(&temp_dir);

    let wav_path = download_audio_as_wav(ytdlp, url, &temp_dir, video_id).await?;
    let transcript = run_whisper_cli(&wav_path, model, Some("auto")).await;
    let _ = std::fs::remove_file(&wav_path);
    let transcript = transcript?.text;

    if transcript.trim().is_empty() {
        return Err("Whisper returned empty transcript".to_string());
    }

    Ok(transcript)
}

fn whisper_cli_candidates_from_base(base: &Path) -> Vec<PathBuf> {
    vec![
        base.join("whisper-cli.exe"),
        base.join("main.exe"),
        base.join("whisper.cpp")
            .join("build")
            .join("bin")
            .join("Release")
            .join("whisper-cli.exe"),
        base.join("whisper.cpp")
            .join("build")
            .join("bin")
            .join("Release")
            .join("main.exe"),
        base.join("resources").join("whisper-cli.exe"),
        base.join("resources").join("main.exe"),
    ]
}

fn first_runnable_candidate(
    candidates: Vec<PathBuf>,
    is_runnable: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    candidates
        .into_iter()
        .find(|candidate| candidate.exists() && is_runnable(candidate))
}

fn whisper_cli_is_runnable(path: &Path) -> bool {
    crate::utils::hidden_command::std_command(path)
        .arg("--help")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn ffmpeg_candidates_from_base(base: &Path) -> Vec<PathBuf> {
    vec![
        base.join("ffmpeg.exe"),
        base.join("resources").join("ffmpeg.exe"),
    ]
}

fn ffmpeg_is_runnable(path: &Path) -> bool {
    crate::utils::hidden_command::std_command(path)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn find_whisper_cli() -> Option<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(d) = dir {
                if let Some(candidate) = first_runnable_candidate(
                    whisper_cli_candidates_from_base(&d),
                    whisper_cli_is_runnable,
                ) {
                    return Some(candidate);
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }

    for candidate in ["whisper-cli.exe", "whisper-cli", "main.exe", "main"] {
        let path = PathBuf::from(candidate);
        if whisper_cli_is_runnable(&path) {
            return Some(path);
        }
    }

    None
}

fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(d) = dir {
                if let Some(candidate) =
                    first_runnable_candidate(ffmpeg_candidates_from_base(&d), ffmpeg_is_runnable)
                {
                    return Some(candidate);
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }

    for candidate in ["ffmpeg.exe", "ffmpeg"] {
        if ffmpeg_is_runnable(Path::new(candidate)) {
            return Some(PathBuf::from(candidate));
        }
    }

    None
}

fn whisper_binary_status_from_candidates(candidate: Option<PathBuf>) -> serde_json::Value {
    match candidate {
        Some(path) => serde_json::json!({
            "available": true,
            "path": path.to_string_lossy().to_string(),
        }),
        None => serde_json::json!({
            "available": false,
            "path": serde_json::Value::Null,
        }),
    }
}

fn build_whisper_cli_args(
    wav_path: &Path,
    model: WhisperModel,
    output_base: &Path,
    language: Option<&str>,
) -> Vec<String> {
    // 預設使用實體核心數量來最佳化速度
    let threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(2))
        .unwrap_or(4);

    let mut args = vec![
        "-m".to_string(),
        model_path(model).to_string_lossy().to_string(),
        "-f".to_string(),
        wav_path.to_string_lossy().to_string(),
        "-t".to_string(),
        threads.to_string(),
    ];

    if let Some(lang) = language {
        args.push("-l".to_string());
        args.push(lang.to_string());
    }

    args.push("-otxt".to_string());
    args.push("-osrt".to_string());
    args.push("-of".to_string());
    args.push(output_base.to_string_lossy().to_string());
    args
}

async fn run_whisper_cli(
    wav_path: &Path,
    model: WhisperModel,
    language: Option<&str>,
) -> Result<WhisperCliOutput, String> {
    if !is_model_downloaded(model) {
        return Err(format!(
            "Model not downloaded: {}. Please download it first.",
            model.filename()
        ));
    }

    let whisper_cli = find_whisper_cli()
        .ok_or("Cannot find whisper-cli.exe or main.exe. Place it next to the app binary.")?;

    let output_dir = std::env::temp_dir().join("insightcap_whisper_cli");
    let _ = std::fs::create_dir_all(&output_dir);
    let output_base = output_dir.join(format!("transcript_{}", uuid::Uuid::now_v7()));
    let output_path = output_base.with_extension("txt");
    let srt_path = output_base.with_extension("srt");
    let _ = std::fs::remove_file(&output_path);
    let _ = std::fs::remove_file(&srt_path);

    let args = build_whisper_cli_args(wav_path, model, &output_base, language);
    let output = crate::utils::hidden_command::tokio_command(&whisper_cli)
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("Failed to run Whisper CLI {:?}: {}", whisper_cli, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Whisper CLI failed (status {}): {} {}",
            output.status,
            stderr.trim(),
            stdout.trim()
        ));
    }

    if !output_path.exists() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "Whisper CLI did not produce transcript {:?}: {} {}",
            output_path,
            stderr.trim(),
            stdout.trim()
        ));
    }

    let text = std::fs::read_to_string(&output_path)
        .map(|text| text.trim().to_string())
        .map_err(|e| format!("Failed to read Whisper transcript {:?}: {}", output_path, e))?;
    let segments = std::fs::read_to_string(&srt_path)
        .map(|content| parse_srt_segments(&content))
        .unwrap_or_default();

    Ok(WhisperCliOutput { text, segments })
}

pub fn format_timestamp_ms(ms: i64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}

fn parse_srt_timestamp(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(',', ".");
    let mut parts = normalized.split([':', '.']);
    let hours: i64 = parts.next()?.parse().ok()?;
    let minutes: i64 = parts.next()?.parse().ok()?;
    let seconds: i64 = parts.next()?.parse().ok()?;
    let millis: i64 = parts.next()?.parse().ok()?;
    Some((((hours * 60 + minutes) * 60 + seconds) * 1000) + millis)
}

fn parse_srt_segments(content: &str) -> Vec<TranscriptSegment> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split("\n\n")
        .filter_map(|block| {
            let mut lines = block.lines().map(str::trim).filter(|line| !line.is_empty());
            let first = lines.next()?;
            let timing = if first.contains("-->") {
                first
            } else {
                lines.next()?
            };
            let (start_raw, end_raw) = timing.split_once("-->")?;
            let start_ms = parse_srt_timestamp(start_raw)?;
            let end_ms = parse_srt_timestamp(end_raw)?;
            let text = lines.collect::<Vec<_>>().join(" ").trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(TranscriptSegment {
                start_ms,
                end_ms,
                text,
            })
        })
        .collect()
}

#[tauri::command]
pub fn whisper_model_status() -> serde_json::Value {
    serde_json::json!({
        "tiny": {
            "downloaded": is_model_downloaded(WhisperModel::Tiny),
            "filename": WhisperModel::Tiny.filename(),
            "display_name": WhisperModel::Tiny.display_name(),
        },
        "base": {
            "downloaded": is_model_downloaded(WhisperModel::Base),
            "filename": WhisperModel::Base.filename(),
            "display_name": WhisperModel::Base.display_name(),
        },
        "small": {
            "downloaded": is_model_downloaded(WhisperModel::Small),
            "filename": WhisperModel::Small.filename(),
            "display_name": WhisperModel::Small.display_name(),
        },
        "medium": {
            "downloaded": is_model_downloaded(WhisperModel::Medium),
            "filename": WhisperModel::Medium.filename(),
            "display_name": WhisperModel::Medium.display_name(),
        },
        "default": WhisperModel::default_model().config_name(),
    })
}

#[tauri::command]
pub fn whisper_binary_status() -> serde_json::Value {
    whisper_binary_status_from_candidates(find_whisper_cli())
}

#[tauri::command]
pub async fn whisper_download_model(
    app: tauri::AppHandle,
    model_name: String,
) -> Result<String, String> {
    use tauri::Emitter;

    let model = WhisperModel::from_name(&model_name)
        .ok_or_else(|| format!("Unknown model: {}", model_name))?;

    if is_model_downloaded(model) {
        return Ok(format!("{} already downloaded", model.filename()));
    }

    let app_clone = app.clone();
    let event_model_name = model_name.clone();
    download_model(model, move |downloaded, total| {
        let percent = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0) as u64
        } else {
            0
        };

        let _ = app_clone.emit(
            "whisper-download-progress",
            serde_json::json!({
                "model": event_model_name,
                "downloaded": downloaded,
                "total": total,
                "percent": percent,
            }),
        );
    })
    .await?;

    Ok(format!("{} downloaded successfully", model.filename()))
}

#[tauri::command]
pub fn whisper_delete_model(model_name: String) -> Result<String, String> {
    let model = WhisperModel::from_name(&model_name)
        .ok_or_else(|| format!("Unknown model: {}", model_name))?;
    delete_model(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_names() {
        assert_eq!(WhisperModel::from_name("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(WhisperModel::from_name("base"), Some(WhisperModel::Base));
        assert_eq!(WhisperModel::from_name("small"), Some(WhisperModel::Small));
        assert_eq!(
            WhisperModel::from_name("medium"),
            Some(WhisperModel::Medium)
        );
        assert_eq!(WhisperModel::from_name("large"), None);
    }

    #[test]
    fn deletes_model_file_at_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let model_file = temp.path().join(WhisperModel::Tiny.filename());
        std::fs::write(&model_file, "model").expect("write model");

        let deleted =
            delete_model_file_at_path(&model_file, WhisperModel::Tiny).expect("delete model");

        assert_eq!(deleted, "ggml-tiny.bin deleted successfully");
        assert!(!model_file.exists());
    }

    #[test]
    fn formats_timestamps() {
        assert_eq!(format_timestamp_ms(3_723_045), "01:02:03.045");
    }

    #[test]
    fn resolves_whisper_cli_candidates_near_binary() {
        let base = Path::new(r"C:\app");
        let candidates = whisper_cli_candidates_from_base(base);
        assert_eq!(
            candidates,
            vec![
                base.join("whisper-cli.exe"),
                base.join("main.exe"),
                base.join("whisper.cpp")
                    .join("build")
                    .join("bin")
                    .join("Release")
                    .join("whisper-cli.exe"),
                base.join("whisper.cpp")
                    .join("build")
                    .join("bin")
                    .join("Release")
                    .join("main.exe"),
                base.join("resources").join("whisper-cli.exe"),
                base.join("resources").join("main.exe"),
            ]
        );
    }

    #[test]
    fn resolves_ffmpeg_candidates_near_binary_and_resources() {
        let base = Path::new(r"C:\app");
        let candidates = ffmpeg_candidates_from_base(base);

        assert_eq!(
            candidates,
            vec![
                base.join("ffmpeg.exe"),
                base.join("resources").join("ffmpeg.exe")
            ]
        );
    }

    #[test]
    fn selects_first_existing_runnable_candidate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let broken = temp.path().join("broken-whisper-cli.exe");
        let ok = temp.path().join("ok-whisper-cli.exe");
        std::fs::write(&broken, "").expect("write broken");
        std::fs::write(&ok, "").expect("write ok");

        let candidates = vec![
            temp.path().join("missing-whisper-cli.exe"),
            broken,
            ok.clone(),
        ];

        let selected = first_runnable_candidate(candidates, |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("ok-"))
        });

        assert_eq!(selected, Some(ok));
    }

    #[test]
    fn only_sends_wav_directly_to_whisper_cli() {
        assert!(whisper_cli_supports_audio_ext("wav"));
        assert!(!whisper_cli_supports_audio_ext("mp3"));
        assert!(!whisper_cli_supports_audio_ext("flac"));
        assert!(!whisper_cli_supports_audio_ext("ogg"));
        assert!(!whisper_cli_supports_audio_ext("m4a"));
        assert!(!whisper_cli_supports_audio_ext("webm"));
    }

    #[test]
    fn stores_models_under_tauri_app_data_dir_on_windows() {
        let local = Path::new(r"C:\Users\dev\AppData\Local");

        assert_eq!(
            app_data_dir_from_env(Some(local), None),
            local.join("com.insightcap.app")
        );
    }

    #[test]
    fn builds_whisper_cli_args_with_model_and_output_dir() {
        let args = build_whisper_cli_args(
            Path::new(r"C:\tmp\audio.wav"),
            WhisperModel::Medium,
            Path::new(r"C:\tmp\out"),
            Some("yue"),
        );

        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).max(2))
            .unwrap_or(4);

        assert_eq!(
            args,
            vec![
                "-m".to_string(),
                model_path(WhisperModel::Medium)
                    .to_string_lossy()
                    .to_string(),
                "-f".to_string(),
                r"C:\tmp\audio.wav".to_string(),
                "-t".to_string(),
                threads.to_string(),
                "-l".to_string(),
                "yue".to_string(),
                "-otxt".to_string(),
                "-osrt".to_string(),
                "-of".to_string(),
                r"C:\tmp\out".to_string(),
            ]
        );
    }

    #[test]
    fn parses_srt_segments_with_timestamps() {
        let srt = "1\n00:00:01,200 --> 00:00:03,450\nHello there.\n\n2\n00:00:03,500 --> 00:00:05,000\nSecond line.\n";
        let segments = parse_srt_segments(srt);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_ms, 1200);
        assert_eq!(segments[0].end_ms, 3450);
        assert_eq!(segments[0].text, "Hello there.");
        assert_eq!(segments[1].start_ms, 3500);
        assert_eq!(segments[1].end_ms, 5000);
        assert_eq!(segments[1].text, "Second line.");
    }

    #[test]
    fn whisper_binary_status_reports_missing_for_empty_candidates() {
        let status = whisper_binary_status_from_candidates(None);
        assert_eq!(status["available"], false);
        assert_eq!(status["path"], serde_json::Value::Null);
    }

    #[test]
    fn youtube_whisper_external_commands_use_hidden_spawn_helpers() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let raw_tokio_command_new = ["tokio::process::Command", "::new("].concat();
        let raw_std_command_new = ["std::process::Command", "::new("].concat();
        let raw_std_command_import = ["use std::process", "::Command;"].concat();

        for rel_path in ["src/whisper_transcribe.rs", "src/capture/video_parser.rs"] {
            let source = std::fs::read_to_string(manifest_dir.join(rel_path))
                .unwrap_or_else(|error| panic!("read {rel_path}: {error}"));

            assert!(
                !source.contains(&raw_tokio_command_new),
                "{rel_path} should use hidden tokio command helpers"
            );
            assert!(
                !source.contains(&raw_std_command_new),
                "{rel_path} should use hidden std command helpers"
            );
            assert!(
                !source.contains(&raw_std_command_import),
                "{rel_path} should not import raw std Command"
            );
        }
    }
}

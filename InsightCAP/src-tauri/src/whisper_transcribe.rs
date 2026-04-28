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
            Self::Tiny => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin"
            }
            Self::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"
            }
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

pub fn model_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let models_dir = dir.join("whisper_models");
            let _ = std::fs::create_dir_all(&models_dir);
            return models_dir;
        }
    }

    let fallback = std::env::temp_dir().join("insightcap_whisper_models");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

pub fn config_path() -> PathBuf {
    model_dir()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("whisper_config.json")
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
    config
        .as_deref()
        .and_then(crate::capture::video_parser::parse_preferred_whisper_model_name)
        .and_then(WhisperModel::from_name)
        .unwrap_or_else(WhisperModel::default_model)
}

pub fn write_model_preference(model: WhisperModel) -> Result<(), String> {
    let json = serde_json::json!({ "model": model.config_name() });
    std::fs::write(config_path(), json.to_string())
        .map_err(|e| format!("Failed to save whisper config: {}", e))
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
    let wav_path = output_dir.join(format!("{}.wav", video_id));

    let output = tokio::process::Command::new(ytdlp)
        .args([
            "--no-playlist",
            "-x",
            "--audio-format",
            "wav",
            "--postprocessor-args",
            "ffmpeg:-ar 16000 -ac 1",
            "-o",
            wav_path.to_str().unwrap_or("audio.wav"),
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
        return Err("yt-dlp did not produce expected wav file".to_string());
    }

    Ok(wav_path)
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

pub async fn transcribe_wav(
    wav_path: &Path,
    model: WhisperModel,
    language: Option<&str>,
) -> Result<Vec<TranscriptSegment>, String> {
    let transcript = run_whisper_cli(wav_path, model, language).await?;
    if transcript.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![TranscriptSegment {
        start_ms: 0,
        end_ms: 0,
        text: transcript,
    }])
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
    let transcript = run_whisper_cli(&wav_path, model, None).await;
    let _ = std::fs::remove_file(&wav_path);
    let transcript = transcript?;

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

fn find_whisper_cli() -> Option<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut dir = current_exe.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(d) = dir {
                for candidate in whisper_cli_candidates_from_base(&d) {
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
                dir = d.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }

    for candidate in ["whisper-cli.exe", "whisper-cli", "main.exe", "main"] {
        if std::process::Command::new(candidate)
            .arg("--help")
            .output()
            .is_ok()
        {
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
    output_dir: &Path,
    language: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        model_path(model).to_string_lossy().to_string(),
        "-f".to_string(),
        wav_path.to_string_lossy().to_string(),
    ];

    if let Some(lang) = language {
        args.push("-l".to_string());
        args.push(lang.to_string());
    }

    args.push("-otxt".to_string());
    args.push("-of".to_string());
    args.push(output_dir.join("transcript").to_string_lossy().to_string());
    args
}

async fn run_whisper_cli(
    wav_path: &Path,
    model: WhisperModel,
    language: Option<&str>,
) -> Result<String, String> {
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
    let output_path = output_dir.join("transcript.txt");
    let _ = std::fs::remove_file(&output_path);

    let args = build_whisper_cli_args(wav_path, model, &output_dir, language);
    let output = tokio::process::Command::new(&whisper_cli)
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

    std::fs::read_to_string(&output_path)
        .map(|text| text.trim().to_string())
        .map_err(|e| format!("Failed to read Whisper transcript {:?}: {}", output_path, e))
}

pub fn format_timestamp_ms(ms: i64) -> String {
    let total_secs = ms / 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    let millis = ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_names() {
        assert_eq!(WhisperModel::from_name("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(WhisperModel::from_name("base"), Some(WhisperModel::Base));
        assert_eq!(WhisperModel::from_name("small"), Some(WhisperModel::Small));
        assert_eq!(WhisperModel::from_name("medium"), Some(WhisperModel::Medium));
        assert_eq!(WhisperModel::from_name("large"), None);
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
                base.join("whisper.cpp").join("build").join("bin").join("Release").join("whisper-cli.exe"),
                base.join("whisper.cpp").join("build").join("bin").join("Release").join("main.exe"),
                base.join("resources").join("whisper-cli.exe"),
                base.join("resources").join("main.exe"),
            ]
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

        assert_eq!(
            args,
            vec![
                "-m".to_string(),
                model_path(WhisperModel::Medium).to_string_lossy().to_string(),
                "-f".to_string(),
                r"C:\tmp\audio.wav".to_string(),
                "-l".to_string(),
                "yue".to_string(),
                "-otxt".to_string(),
                "-of".to_string(),
                r"C:\tmp\out\transcript".to_string(),
            ]
        );
    }

    #[test]
    fn whisper_binary_status_reports_missing_for_empty_candidates() {
        let status = whisper_binary_status_from_candidates(None);
        assert_eq!(status["available"], false);
        assert_eq!(status["path"], serde_json::Value::Null);
    }
}

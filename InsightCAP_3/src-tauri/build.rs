use std::path::PathBuf;

fn main() {
    // 自動下載 pdfium.dll（Windows x64）
    download_pdfium_if_needed();

    tauri_build::build()
}

fn download_pdfium_if_needed() {
    // 只在 Windows 上執行
    if !cfg!(target_os = "windows") {
        return;
    }

    // 目標位置：target/{profile}/ 目錄下
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    // OUT_DIR 通常是 target/debug/build/<pkg>/out，往上三層到 target/debug/
    let profile_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Cannot find profile dir")
        .to_path_buf();

    let dll_dest = profile_dir.join("pdfium.dll");

    if dll_dest.exists() {
        println!("cargo:warning=pdfium.dll 已存在於 {:?}，跳過下載", dll_dest);
        return;
    }

    // 同時也複製到 src-tauri/ 目錄（供 tauri bundle resources 使用）
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let src_tauri_dll = PathBuf::from(&manifest_dir).join("pdfium.dll");

    println!("cargo:warning=正在下載 pdfium.dll ...");

    let url = "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-win-x64.tgz";
    let dest_str = dll_dest.to_string_lossy().replace('\\', "\\\\");
    let src_tauri_str = src_tauri_dll.to_string_lossy().replace('\\', "\\\\");

    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$tmp = [System.IO.Path]::GetTempPath()
$tgz = Join-Path $tmp 'pdfium-win-x64.tgz'
$extract = Join-Path $tmp 'pdfium-extract'

Write-Host 'Downloading pdfium-win-x64.tgz...'
Invoke-WebRequest -Uri '{url}' -OutFile $tgz -UseBasicParsing

if (Test-Path $extract) {{ Remove-Item $extract -Recurse -Force }}
New-Item -ItemType Directory -Path $extract | Out-Null

Write-Host 'Extracting...'
tar -xzf $tgz -C $extract

# 搜尋解壓後的 pdfium.dll（不假設路徑）
$dll = Get-ChildItem -Path $extract -Filter 'pdfium.dll' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $dll) {{
    Write-Host 'pdfium.dll not found in archive, listing contents:'
    Get-ChildItem -Path $extract -Recurse | Select-Object FullName
    exit 1
}}

Write-Host "Found DLL at: $dll"
Copy-Item $dll '{dest}'
Copy-Item $dll '{src_tauri}'
Write-Host 'Done.'
"#,
        url = url,
        dest = dest_str,
        src_tauri = src_tauri_str,
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=pdfium.dll 下載成功");
        }
        Ok(s) => {
            println!(
                "cargo:warning=pdfium.dll 下載失敗（exit code: {}），PDF 功能將無法使用",
                s
            );
        }
        Err(e) => {
            println!("cargo:warning=無法執行 PowerShell：{}，PDF 功能將無法使用", e);
        }
    }
}

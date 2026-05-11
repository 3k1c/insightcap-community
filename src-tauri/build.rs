use std::path::PathBuf;

fn main() {
    download_pdfium_if_needed();
    tauri_build::build();
}

fn download_pdfium_if_needed() {
    if !cfg!(target_os = "windows") {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let profile_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Cannot find profile dir")
        .to_path_buf();

    let dll_dest = profile_dir.join("pdfium.dll");
    if dll_dest.exists() {
        println!(
            "cargo:warning=pdfium.dll already exists at {:?}, skipping download",
            dll_dest
        );
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let src_tauri_dll = PathBuf::from(&manifest_dir).join("pdfium.dll");

    println!("cargo:warning=Downloading pdfium.dll...");

    let url =
        "https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-win-x64.tgz";
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
            println!("cargo:warning=pdfium.dll downloaded successfully");
        }
        Ok(s) => {
            println!(
                "cargo:warning=pdfium.dll download failed, exit code: {}. PDF extraction may be unavailable",
                s
            );
        }
        Err(e) => {
            println!(
                "cargo:warning=failed to run PowerShell: {}. PDF extraction may be unavailable",
                e
            );
        }
    }
}

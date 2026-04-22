#![cfg(target_os = "macos")]

use std::fs::File;
use std::io::Write;
use std::process::Command;
use uuid::Uuid; // Assuming uuid crate is added to Cargo.toml

pub async fn recognize_text_from_bytes(image_data: &[u8]) -> Result<String, String> {
    let temp_dir = std::env::temp_dir();
    let id = Uuid::now_v7().to_string();
    let temp_path = temp_dir.join(format!("{}.png", id));
    let script_path = temp_dir.join(format!("{}.swift", id));

    let mut temp_file = File::create(&temp_path)
        .map_err(|e| format!("Failed to create temporary image file: {}", e))?;

    temp_file
        .write_all(image_data)
        .map_err(|e| format!("Failed to write image bytes: {}", e))?;

    let swift_script = r#"
import Cocoa
import Vision

guard CommandLine.arguments.count > 1 else {
    fputs("No image path provided", stderr)
    exit(1)
}

let imagePath = CommandLine.arguments[1]

guard let image = NSImage(contentsOfFile: imagePath),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
    fputs("Could not load image", stderr)
    exit(1)
}

let request = VNRecognizeTextRequest { (request, error) in
    if let error = error {
        fputs("Vision error: \(error.localizedDescription)", stderr)
        return
    }
    
    guard let observations = request.results as? [VNRecognizedTextObservation] else {
        return
    }
    
    var fullText = ""
    for observation in observations {
        let topCandidate = observation.topCandidates(1).first
        if let recognizedText = topCandidate?.string {
            fullText += recognizedText + "\n"
        }
    }
    print(fullText)
}

request.recognitionLanguages = ["zh-Hant", "zh-Hans", "en-US"]
request.usesLanguageCorrection = true

let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
} catch {
    fputs("Failed to perform OCR: \(error.localizedDescription)", stderr)
    exit(1)
}
"#;

    let mut script_file = File::create(&script_path)
        .map_err(|e| format!("Failed to create Swift script file: {}", e))?;

    script_file
        .write_all(swift_script.as_bytes())
        .map_err(|e| format!("Failed to write Swift script: {}", e))?;

    let output = Command::new("swift")
        .arg(&script_path)
        .arg(&temp_path)
        .output()
        .map_err(|e| {
            format!(
                "Failed to run Swift OCR script (check Command Line Tools): {}",
                e
            )
        })?;

    let _ = std::fs::remove_file(&temp_path);
    let _ = std::fs::remove_file(&script_path);

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("macOS Vision OCR failed: {}", err))
    }
}

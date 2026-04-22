use arboard::Clipboard;
use std::path::PathBuf;

pub enum ClipboardContent {
    Files(Vec<PathBuf>),
    Data {
        text: Option<String>,
        has_image: bool,
        image_bytes: Option<Vec<u8>>,
    },
}

pub fn clear_clipboard() -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard init error: {}", e))?;
    clipboard
        .clear()
        .map_err(|e| format!("Clipboard clear error: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn get_copied_files() -> Option<Vec<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    unsafe {
        if OpenClipboard(HWND::default()).is_err() {
            return None;
        }

        let handle = GetClipboardData(15);
        if handle.is_err() {
            let _ = CloseClipboard();
            return None;
        }

        let handle = handle.unwrap();
        if handle.is_invalid() {
            let _ = CloseClipboard();
            return None;
        }

        let hdrop = HDROP(handle.0 as *mut _);
        let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        if count == 0 {
            let _ = CloseClipboard();
            return None;
        }

        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            let length = DragQueryFileW(hdrop, i, None);
            if length == 0 {
                continue;
            }
            let mut buf = vec![0u16; (length + 1) as usize];
            if DragQueryFileW(hdrop, i, Some(&mut buf)) > 0 {
                if let Some(pos) = buf.iter().position(|&c| c == 0) {
                    buf.truncate(pos);
                }
                paths.push(PathBuf::from(std::ffi::OsString::from_wide(&buf)));
            }
        }

        let _ = CloseClipboard();
        if paths.is_empty() {
            None
        } else {
            Some(paths)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_copied_files() -> Option<Vec<PathBuf>> {
    None
}

pub fn read_clipboard() -> Result<ClipboardContent, String> {
    if let Some(paths) = get_copied_files() {
        return Ok(ClipboardContent::Files(paths));
    }

    let mut clipboard = Clipboard::new().map_err(|e| format!("Clipboard init error: {}", e))?;

    let text = match clipboard.get_text() {
        Ok(t) if !t.trim().is_empty() => Some(t),
        _ => None,
    };

    let (has_image, image_bytes) = match clipboard.get_image() {
        Ok(img) => (true, Some(img.bytes.into_owned())),
        Err(_) => (false, None),
    };

    if text.is_some() || has_image {
        Ok(ClipboardContent::Data {
            text,
            has_image,
            image_bytes,
        })
    } else {
        Err("Clipboard is empty or contains unsupported format".to_string())
    }
}

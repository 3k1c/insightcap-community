use active_win_pos_rs::get_active_window as get_active_win;

pub struct WindowMetadata {
    pub title: String,
    pub app_name: String,
    pub pid: u32,
    pub url: String,
}

pub fn get_active_window() -> WindowMetadata {
    match get_active_win() {
        Ok(win) => {
            // Minimal heuristic for extracting URL from title if using a browser (Windows often appends " - BrowserName")
            let title = win.title;
            let url = "".to_string(); // Deep URL extraction needs accessibility APIs, keeping it simple for MVP

            WindowMetadata {
                title,
                app_name: win.app_name,
                pid: win.process_id as u32,
                url,
            }
        }
        Err(e) => {
            eprintln!("Failed to get active window: {:?}", e);
            WindowMetadata {
                title: "Unknown".to_string(),
                app_name: "Unknown".to_string(),
                pid: 0,
                url: "".to_string(),
            }
        }
    }
}

use std::process::Command;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn prompt_accessibility() {
    let check_script = r#"
        tell application "System Events"
            return UI elements enabled
        end tell
    "#;

    let output = Command::new("osascript")
        .args(["-e", check_script])
        .output();

    if output.map(|o| o.status.success()).unwrap_or(false) {
        return;
    }

    if let Err(e) = Command::new("open").args(["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"]).spawn() {
        log::error!("[Accessibility] Failed to open System Settings: {}", e);
    }
}

#[tauri::command]
pub fn check_accessibility_permission() -> bool {
    check_accessibility()
}

#[tauri::command]
pub fn request_accessibility_permission() {
    prompt_accessibility();
}

#[tauri::command]
pub fn open_accessibility_settings() {
    prompt_accessibility();
}
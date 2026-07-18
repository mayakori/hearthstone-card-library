#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![health_check])
        .run(tauri::generate_context!())
        .expect("failed to run Hearthstone Card Lab");
}

#[cfg(test)]
mod tests {
    use super::health_check;

    #[test]
    fn backend_health_check_is_ok() {
        assert_eq!(health_check(), "ok");
    }
}

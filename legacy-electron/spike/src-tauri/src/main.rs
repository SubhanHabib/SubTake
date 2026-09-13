use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
#[tauri::command]
fn show_hud(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<bool, String> {
    if window.label() != "main" {
        return Err("Editor only".into());
    }
    if let Some(hud) = app.get_webview_window("hud") {
        hud.close().map_err(|e| e.to_string())?;
        return Ok(false);
    }
    let mut url = app
        .get_webview_window("main")
        .ok_or("No editor")?
        .url()
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut().append_pair("hud", "1");
    WebviewWindowBuilder::new(&app, "hud", WebviewUrl::External(url))
        .title("SubTake HUD")
        .inner_size(340.0, 110.0)
        .always_on_top(true)
        .resizable(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(true)
}
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![show_hud])
        .setup(|app| {
            let value = std::env::var("SUBTAKE_SPIKE_URL")?;
            let url: tauri::Url = value.parse()?;
            if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
                return Err("Expected loopback spike URL".into());
            }
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("SubTake Shell Spike")
                .inner_size(1100.0, 800.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri spike failed");
}

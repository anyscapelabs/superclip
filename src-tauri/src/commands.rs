use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, State};

use crate::clipboard::{self, ClipboardBackend};
use crate::store::{ClipItem, Store};

const PASTE_DELAY: Duration = Duration::from_millis(200);

#[tauri::command]
pub fn get_history(state: State<Mutex<Store>>) -> Vec<ClipItem> {
    state.lock().unwrap().history.items.clone()
}

#[tauri::command]
pub async fn paste_item(id: String, app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    write_and_bump_in_background(id, app.clone()).await?;

    let target = app.state::<Mutex<Option<String>>>().lock().unwrap().clone();
    schedule_paste(target, app);
    Ok(())
}

#[tauri::command]
pub fn toggle_pin(id: String, state: State<Mutex<Store>>) {
    state.lock().unwrap().toggle_pin(&id);
}

#[tauri::command]
pub fn clear_history(state: State<Mutex<Store>>) {
    state.lock().unwrap().clear_unpinned();
}

#[tauri::command]
pub async fn get_image(id: String, app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || encode_image_base64(&app, &id))
        .await
        .map_err(|error| error.to_string())?
}

fn encode_image_base64(app: &AppHandle, id: &str) -> Result<String, String> {
    use base64::Engine as _;

    let image = app
        .state::<Mutex<Store>>()
        .lock()
        .unwrap()
        .image_bytes(id)
        .ok_or_else(|| "image not found".to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(image))
}

async fn write_and_bump_in_background(id: String, app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        write_item(&app, &id)?;
        app.state::<Mutex<Store>>().lock().unwrap().bump(&id);
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn write_item(app: &AppHandle, id: &str) -> Result<(), String> {
    let item = app
        .state::<Mutex<Store>>()
        .lock()
        .unwrap()
        .get(id)
        .ok_or_else(|| "item not found".to_string())?;

    if item.image.is_some() {
        write_image(app, &item)
    } else {
        write_text(app, &item.text)
    }
}

fn write_text(app: &AppHandle, text: &str) -> Result<(), String> {
    app.state::<Mutex<clipboard::text::TextClipboard>>()
        .lock()
        .unwrap()
        .write(text)
}

fn write_image(app: &AppHandle, item: &ClipItem) -> Result<(), String> {
    let png = app
        .state::<Mutex<Store>>()
        .lock()
        .unwrap()
        .image_bytes(&item.id)
        .ok_or_else(|| "image file missing".to_string())?;

    app.state::<Mutex<clipboard::image::ImageClipboard>>()
        .lock()
        .unwrap()
        .write(&png)
}

fn schedule_paste(target: Option<String>, app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(PASTE_DELAY);
        let backend = app.state::<Mutex<Box<dyn ClipboardBackend>>>();
        let _ = backend.lock().unwrap().send_paste(target.as_deref());
    });
}

use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, State, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

mod clipboard;
mod detect;
mod store;
mod watcher;

use clipboard::{detect_backend, ClipboardBackend};
use store::{ClipItem, Store};

// --- Paste timing & retry -------------------------------------------------
// Initial delay after the picker hides: gives the WM time to release input
// focus from the picker before we re-focus the target app. Too short and the
// activation races the hide.
const INITIAL_PASTE_DELAY: Duration = Duration::from_millis(200);

fn center_on_primary(win: &tauri::WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = win.primary_monitor()? {
        let area = monitor.work_area();
        let size = win.outer_size()?;
        let x = area.size.width.saturating_sub(size.width) / 2;
        let y = area.size.height.saturating_sub(size.height) / 2;
        win.set_position(PhysicalPosition::new(x as i32, y as i32))?;
    }
    Ok(())
}

fn toggle_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible()? {
            win.hide()?;
        } else {
            #[cfg(target_os = "linux")]
            if let Some(id) = crate::clipboard::x11::active_window_id() {
                let prev = app.state::<Mutex<Option<String>>>();
                *prev.lock().unwrap() = Some(id);
            }

            win.unminimize()?;
            center_on_primary(&win)?;
            win.set_always_on_top(true)?;
            win.show()?;
            win.set_focus()?;

            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

            let win_clone = win.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(60));
                let _ = win_clone.set_focus();
            });
        }
    }
    Ok(())
}

#[tauri::command]
fn get_history(state: State<Mutex<Store>>) -> Vec<ClipItem> {
    state.lock().unwrap().history.items.clone()
}

#[tauri::command]
fn copy_item(
    id: String,
    state: State<Mutex<Store>>,
    clipboard: State<Mutex<Box<dyn ClipboardBackend>>>,
) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    {
        let mut cb = clipboard.lock().unwrap();
        if let Some(entry) = item.image.as_ref() {
            let png = state
                .lock()
                .unwrap()
                .image_bytes(&id)
                .ok_or_else(|| "image file missing".to_string())?;
            cb.set_image(&png)?;
            state.lock().unwrap().add_image(
                &id,
                &png,
                entry.width,
                entry.height,
                entry.name.clone(),
            );
        } else {
            cb.set_text(item.text.as_str())?;
            state.lock().unwrap().add_text(&item.text, &item.kind);
        }
    }
    Ok(())
}

/// Returns an item's stored image as a base64-encoded PNG so the frontend can
/// render a preview without exposing filesystem paths.
#[tauri::command]
fn get_image(id: String, state: State<Mutex<Store>>) -> Result<String, String> {
    use base64::Engine as _;
    let store = state.lock().unwrap();
    let png = store
        .image_bytes(&id)
        .ok_or_else(|| "image not found".to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

fn send_paste_key(prev_window: Option<String>, app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(INITIAL_PASTE_DELAY);
        let backend = app.state::<Mutex<Box<dyn ClipboardBackend>>>();
        let _ = backend.lock().unwrap().send_paste(prev_window.as_deref());
    });
}

#[tauri::command]
fn paste_item(
    id: String,
    app: tauri::AppHandle,
    state: State<Mutex<Store>>,
    clipboard: State<Mutex<Box<dyn ClipboardBackend>>>,
) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    // The clipboard is set synchronously (and BEFORE the window hides) so the
    // target app is guaranteed to find the data when the paste key lands.
    // Background-threading this made image and text paste flaky.
    {
        let mut cb = clipboard.lock().unwrap();
        if let Some(entry) = item.image.as_ref() {
            let png = state
                .lock()
                .unwrap()
                .image_bytes(&id)
                .ok_or_else(|| "image file missing".to_string())?;
            cb.set_image(&png)?;
            state.lock().unwrap().add_image(
                &id,
                &png,
                entry.width,
                entry.height,
                entry.name.clone(),
            );
        } else {
            cb.set_text(item.text.as_str())?;
            state.lock().unwrap().add_text(&item.text, &item.kind);
        }
    }

    let prev_window = app.state::<Mutex<Option<String>>>().lock().unwrap().clone();

    // Exactly one activation happens per paste, inside the async paste flow
    // (send_synthetic_paste). Doing it here too would race the picker hide.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    send_paste_key(prev_window, app.clone());
    Ok(())
}

#[tauri::command]
fn toggle_pin(id: String, state: State<Mutex<Store>>) -> Result<(), String> {
    let mut store = state.lock().unwrap();
    store.toggle_pin(&id);
    Ok(())
}

#[tauri::command]
fn clear_history(state: State<Mutex<Store>>) {
    let mut store = state.lock().unwrap();
    store.clear_unpinned();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--from-autostart"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                app.set_activation_policy(ActivationPolicy::Accessory);
            }

            let store = Store::new(app.path().app_data_dir()?.join("history.json"));
            app.manage(Mutex::new(store));
            app.manage(Mutex::new(None::<String>));
            app.manage(Mutex::new(detect_backend()));
            watcher::start(app.handle().clone());

            // Start quietly with the OS on login (enabled once unless the user
            // has explicitly turned it off). The window stays hidden and the
            // app keeps living in the system tray.
            if !std::env::args().any(|a| a == "--from-autostart") {
                let _ = app.autolaunch().enable();
            }

            let open = MenuItem::with_id(app, "open", "Open Superclip", true, None::<&str>)?;
            let check_update = MenuItem::with_id(
                app,
                "check_update",
                "Check for Updates…",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &check_update, &quit])?;

            let _tray = TrayIconBuilder::with_id("superclip-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Superclip")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        let _ = toggle_window(app);
                    }
                    // Ask the frontend to run the update flow (check → download → install
                    // → relaunch). Show the dedicated updater window first so the
                    // user gets a clear, separate status card.
                    "check_update" => {
                        if let Some(win) = app.get_webview_window("updater") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                        let _ = app.emit("updater:check", ());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);
            // A second instance (e.g. `--from-autostart` plus a manual launch,
            // or two dev runs) may find the hotkey already taken. Registering
            // must not bring the whole app down with a panic — the picker stays
            // reachable from the tray, and only that one instance owns the key.
            if let Err(e) = app.global_shortcut().register(shortcut) {
                eprintln!("superclip: hotkey already registered: {e}");
            }

            let win = app.get_webview_window("main").unwrap();
            win.hide()?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_history,
            copy_item,
            paste_item,
            toggle_pin,
            clear_history,
            get_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

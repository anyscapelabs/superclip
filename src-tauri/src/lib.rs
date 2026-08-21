use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

const FOCUS_ATTEMPTS: u8 = 8;
const FOCUS_ATTEMPT_DELAY: Duration = Duration::from_millis(40);

mod clipboard;
mod commands;
mod detect;
mod store;
mod updater;
mod watcher;

use clipboard::detect_backend;
use store::Store;

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

fn focus_when_mapped(app: &tauri::AppHandle) {
    let app = app.clone();
    let focused = Arc::new(AtomicBool::new(false));

    std::thread::spawn(move || {
        for _ in 0..FOCUS_ATTEMPTS {
            let handle = app.clone();
            let observed = focused.clone();
            let _ = app.run_on_main_thread(move || {
                let Some(win) = handle.get_webview_window("main") else {
                    return;
                };
                if win.is_focused().unwrap_or(false) {
                    observed.store(true, Ordering::Release);
                } else if win.is_visible().unwrap_or(false) {
                    let _ = win.set_focus();
                }
            });

            std::thread::sleep(FOCUS_ATTEMPT_DELAY);
            if focused.load(Ordering::Acquire) {
                return;
            }
        }
    });
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
            let _ = app.emit("palette-shown", ());

            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

            focus_when_mapped(app);
        }
    }
    Ok(())
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
            app.manage(Mutex::new(crate::clipboard::text::TextClipboard::new()));
            app.manage(Mutex::new(crate::clipboard::image::ImageClipboard::new()));
            app.manage(crate::updater::PendingUpdate::default());
            watcher::start(app.handle().clone());

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
            commands::get_history,
            commands::paste_item,
            commands::toggle_pin,
            commands::clear_history,
            commands::get_image,
            updater::get_channel,
            updater::set_channel,
            updater::check_update,
            updater::install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

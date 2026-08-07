use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, State, WindowEvent,
};

mod detect;
mod store;
mod watcher;

use store::{ClipItem, Store};

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
            if let Some(id) = active_window_id() {
                let prev = app.state::<Mutex<Option<String>>>();
                *prev.lock().unwrap() = Some(id);
            }

            win.unminimize()?;
            center_on_primary(&win)?;
            win.set_always_on_top(true)?;
            win.show()?;
            win.set_focus()?;

            #[cfg(target_os = "macos")]
            app.activate();

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
fn copy_item(id: String, state: State<Mutex<Store>>) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(item.text.clone()).map_err(|e| e.to_string())?;
    state.lock().unwrap().add(item.text.clone(), item.kind.clone());
    Ok(())
}

#[cfg(target_os = "linux")]
fn active_window_id() -> Option<String> {
    let out = std::process::Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(target_os = "linux")]
fn window_class(id: &str) -> String {
    std::process::Command::new("xprop")
        .args(["-id", id, "WM_CLASS"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_lowercase()
}

#[cfg(target_os = "linux")]
fn is_terminal(class: &str) -> bool {
    const TERMS: &[&str] = &[
        "gnome-terminal",
        "gnome.terminal",
        "org.gnome.terminal",
        "x-terminal",
        "konsole",
        "xterm",
        "uxterm",
        "alacritty",
        "kitty",
        "wezterm",
        "terminator",
        "tilix",
        "st",
        "urxvt",
        "rxvt",
        "foot",
        "ghostty",
        "xfce4-terminal",
        "lxterminal",
        "mate-terminal",
        "pantheon-terminal",
        "contour",
    ];
    TERMS.iter().any(|t| class.contains(t)) || class.contains("terminal")
}

#[cfg(target_os = "linux")]
fn paste_into_terminal(text: &str, target: Option<&str>) {
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "primary"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();

        if let Some(id) = target {
            let _ = std::process::Command::new("xdotool")
                .args(["windowactivate", id])
                .status();
            std::thread::sleep(Duration::from_millis(50));
            let _ = std::process::Command::new("xdotool")
                .args(["click", "--window", id, "2"])
                .status();
        } else {
            let _ = std::process::Command::new("xdotool").args(["click", "2"]).status();
        }
        return;
    }
    let _ = std::process::Command::new("xdotool").args(["keydown", "ctrl+shift"]).status();
    let _ = std::process::Command::new("xdotool").args(["key", "v"]).status();
    let _ = std::process::Command::new("xdotool").args(["keyup", "ctrl+shift"]).status();
}

fn send_paste_key(text: String, prev_window: Option<String>) {
    let _ = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        #[cfg(target_os = "linux")]
        {
            let target = prev_window.filter(|s| !s.is_empty());
            let class = target.as_deref().map(window_class).unwrap_or_default();
            if is_terminal(&class) {
                paste_into_terminal(&text, target.as_deref());
            } else {
                if let Some(id) = &target {
                    let _ = std::process::Command::new("xdotool")
                        .args(["windowactivate", id])
                        .status();
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = std::process::Command::new("xdotool")
                    .args(["key", "--clearmodifiers", "ctrl+v"])
                    .status();
            }
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')",
                ])
                .status();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .args([
                    "-e",
                    "tell application \"System Events\" to keystroke \"v\" using command down",
                ])
                .status();
        }
    });
}

#[tauri::command]
fn paste_item(id: String, app: tauri::AppHandle, state: State<Mutex<Store>>) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(item.text.clone()).map_err(|e| e.to_string())?;
    state.lock().unwrap().add(item.text.clone(), item.kind.clone());

    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    let prev_window = app
        .state::<Mutex<Option<String>>>()
        .lock()
        .unwrap()
        .clone();
    send_paste_key(item.text.clone(), prev_window);
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
            watcher::start(app.handle().clone());

            let open = MenuItem::with_id(app, "open", "Open ClipMate", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let _tray = TrayIconBuilder::with_id("clipmate-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("ClipMate")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        let _ = toggle_window(app);
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
            app.global_shortcut().register(shortcut)?;

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
            clear_history
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

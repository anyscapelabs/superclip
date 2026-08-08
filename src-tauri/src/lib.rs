use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
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

// --- Paste timing & retry -------------------------------------------------
// Initial delay after the picker hides: gives the WM time to release X input
// focus from the picker before we re-focus the target app. Too short and the
// activation races the hide.
const INITIAL_PASTE_DELAY: Duration = Duration::from_millis(200);
// Settle time after a WM-level focus change. Chromium/Electron pages can lag
// behind the WM: the window is active but the page's focused element isn't
// ready to accept a paste yet.
const FOCUS_SETTLE_DELAY: Duration = Duration::from_millis(100);
// Bounded focus-verification retries. We re-check that the target window
// actually has focus (a successful `windowactivate` is not proof of focus),
// retry once, then give up rather than polling forever.
const MAX_FOCUS_ATTEMPTS: u32 = 2;

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
fn copy_item(
    id: String,
    state: State<Mutex<Store>>,
    clipboard: State<Mutex<arboard::Clipboard>>,
) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    clipboard
        .lock()
        .unwrap()
        .set_text(item.text.clone())
        .map_err(|e| e.to_string())?;
    state
        .lock()
        .unwrap()
        .add(item.text.clone(), item.kind.clone());
    Ok(())
}

#[cfg(target_os = "linux")]
fn active_window_id() -> Option<String> {
    // NOTE: a browser window with multiple tabs is a single X11 window — all
    // tabs share one window id. We can only target the window, not a specific
    // tab, so a paste lands in whichever tab is focused/visible at paste time.
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

// Reads WM_CLASS via xprop. Kept (unused) for v0.2 terminal-paste support,
// which needs it to distinguish terminal apps from GUI apps. Do not delete.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
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
static PASTE_LOG_FAILED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
fn paste_log(msg: &str) {
    use std::time::{SystemTime, UNIX_EPOCH};
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/superclip-paste.log")
    {
        Ok(mut f) => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let _ = writeln!(f, "{ms}: {msg}");
        }
        Err(e) => {
            // Fail loudly exactly once per app run so a broken/unwritable log
            // can't silently fool a debugging session, but never crash the
            // paste feature itself.
            if !PASTE_LOG_FAILED.swap(true, Ordering::SeqCst) {
                eprintln!("superclip: warning: cannot write /tmp/superclip-paste.log: {e}");
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn activate_window(id: &str) {
    // windowactivate --sync blocks until the WM has raised+activated the
    // window; windowfocus --sync then pins X input focus to it so synthetic
    // keys land on the right app. Neither returning Ok proves focus landed —
    // send_synthetic_paste verifies that separately.
    let act = std::process::Command::new("xdotool")
        .args(["windowactivate", "--sync", id])
        .status();
    let foc = std::process::Command::new("xdotool")
        .args(["windowfocus", "--sync", id])
        .status();
    paste_log(&format!(
        "activate id={id} windowactivate={:?} windowfocus={:?}",
        act.map(|s| s.success()),
        foc.map(|s| s.success())
    ));
    std::thread::sleep(FOCUS_SETTLE_DELAY);
}

#[cfg(target_os = "linux")]
fn focused_window_id() -> Option<String> {
    let out = std::process::Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    paste_log(&format!("getactivewindow => {id:?}"));
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(target_os = "linux")]
fn send_synthetic_paste(target: Option<&str>) {
    // Terminal paste (xclip PRIMARY + middle-click) is deferred to v0.2; this
    // phase only handles browsers and GUI apps via a synthetic Ctrl+V.
    let Some(id) = target.map(str::trim).filter(|s| !s.is_empty()) else {
        // We never learned which window the user was in before opening the
        // picker (active_window_id() failed). Best we can do is paste into
        // whatever currently has focus; keep it explicit in the log.
        paste_log("no target window captured, falling back to ambient focus paste");
        let key = std::process::Command::new("xdotool")
            .args(["key", "ctrl+v"])
            .status();
        paste_log(&format!(
            "ambient key status={:?}",
            key.map(|s| s.success())
        ));
        return;
    };

    // A browser window with multiple tabs is a single X11 window — all tabs
    // share one window id. We can only target the window, not a specific tab,
    // so the paste lands in whichever tab is actually focused at paste time.
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        activate_window(id);
        let focused = focused_window_id();
        if focused.as_deref() == Some(id) {
            break;
        }
        paste_log(&format!(
            "focus mismatch: focused={focused:?} want={id} attempt={attempts}"
        ));
        if attempts >= MAX_FOCUS_ATTEMPTS {
            break;
        }
    }

    // Send Ctrl+V directly to the target window (XSendEvent, so it does not
    // depend on a keyboard grab / XTEST). Focus was verified above, so this is
    // belt-and-suspenders on top of a focused-window send.
    let key = std::process::Command::new("xdotool")
        .args(["key", "--window", id, "ctrl+v"])
        .status();
    paste_log(&format!(
        "key --window {id} ctrl+v status={:?}",
        key.map(|s| s.success())
    ));
}

fn send_paste_key(prev_window: Option<String>) {
    std::thread::spawn(move || {
        std::thread::sleep(INITIAL_PASTE_DELAY);
        #[cfg(target_os = "linux")]
        {
            let target = prev_window.filter(|s| !s.is_empty());
            paste_log(&format!("send_paste_key target={target:?}"));
            send_synthetic_paste(target.as_deref());
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
fn paste_item(
    id: String,
    app: tauri::AppHandle,
    state: State<Mutex<Store>>,
    clipboard: State<Mutex<arboard::Clipboard>>,
) -> Result<(), String> {
    let item = {
        let store = state.lock().unwrap();
        store.get(&id).ok_or_else(|| "item not found".to_string())?
    };
    clipboard
        .lock()
        .unwrap()
        .set_text(item.text.clone())
        .map_err(|e| e.to_string())?;
    state
        .lock()
        .unwrap()
        .add(item.text.clone(), item.kind.clone());

    let prev_window = app.state::<Mutex<Option<String>>>().lock().unwrap().clone();

    // Exactly one activation happens per paste, inside the async paste flow
    // (send_synthetic_paste). Doing it here too would race the picker hide.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    send_paste_key(prev_window);
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
            app.manage(Mutex::new(
                arboard::Clipboard::new().expect("failed to init clipboard"),
            ));
            watcher::start(app.handle().clone());

            let open = MenuItem::with_id(app, "open", "Open Superclip", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            let _tray = TrayIconBuilder::with_id("superclip-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Superclip")
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

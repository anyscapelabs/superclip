use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::store::{Store, UpdateChannel};

const BETA_ENDPOINT: &str =
    "https://github.com/anyscapelabs/superclip/releases/download/beta/latest.json";

#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub fn get_channel(state: State<Mutex<Store>>) -> UpdateChannel {
    state.lock().unwrap().channel()
}

#[tauri::command]
pub fn set_channel(channel: UpdateChannel, app: AppHandle) {
    app.state::<Mutex<Store>>()
        .lock()
        .unwrap()
        .set_channel(channel);
    app.state::<PendingUpdate>().0.lock().unwrap().take();
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let channel = app.state::<Mutex<Store>>().lock().unwrap().channel();

    let mut builder = app.updater_builder();
    if channel == UpdateChannel::Beta {
        let endpoint = Url::parse(BETA_ENDPOINT).map_err(|error| error.to_string())?;
        builder = builder
            .endpoints(vec![endpoint])
            .map_err(|error| error.to_string())?;
    }

    let update = builder
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;

    let info = update.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
    });
    *app.state::<PendingUpdate>().0.lock().unwrap() = update;

    Ok(info)
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<PendingUpdate>()
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "no update ready to install".to_string())?;

    let reporter = app.clone();
    let mut downloaded: u64 = 0;

    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = reporter.emit("updater:progress", DownloadProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|error| error.to_string())
}

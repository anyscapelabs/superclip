import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export type UpdateStatus =
  | { state: "checking" }
  | { state: "uptodate"; version: string }
  | { state: "downloading"; version: string; percent: number }
  | { state: "installing"; version: string }
  | { state: "error"; message: string };

export async function checkForUpdates(onStatus: (s: UpdateStatus) => void): Promise<void> {
  onStatus({ state: "checking" });

  let update;
  try {
    update = await check();
  } catch (e) {
    onStatus({ state: "error", message: String(e) });
    return;
  }

  if (!update) {
    onStatus({ state: "uptodate", version: await getVersion() });
    return;
  }

  onStatus({ state: "downloading", version: update.version, percent: 0 });
  await update.downloadAndInstall((event) => {
    if (event.event === "Progress") {
      onStatus({ state: "downloading", version: update.version, percent: 0 });
    }
  });

  onStatus({ state: "installing", version: update.version });
  await relaunch();
}
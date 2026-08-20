import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateChannel = "stable" | "beta";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "uptodate"; version: string }
  | { state: "downloading"; version: string; percent: number | null }
  | { state: "ready"; version: string }
  | { state: "error"; message: string };

interface Progress {
  downloaded: number;
  total: number | null;
}

export function getChannel(): Promise<UpdateChannel> {
  return invoke<UpdateChannel>("get_channel");
}

export function setChannel(channel: UpdateChannel): Promise<void> {
  return invoke<void>("set_channel", { channel });
}

export function restart(): Promise<void> {
  return relaunch();
}

export async function checkForUpdates(onStatus: (s: UpdateStatus) => void): Promise<void> {
  onStatus({ state: "checking" });

  let update: UpdateInfo | null;
  try {
    update = await invoke<UpdateInfo | null>("check_update");
  } catch (e) {
    onStatus({ state: "error", message: String(e) });
    return;
  }

  if (!update) {
    onStatus({ state: "uptodate", version: await getVersion() });
    return;
  }

  const { version } = update;
  onStatus({ state: "downloading", version, percent: null });

  const unlisten = await listen<Progress>("updater:progress", ({ payload }) => {
    const { downloaded, total } = payload;
    const percent = total ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
    onStatus({ state: "downloading", version, percent });
  });

  try {
    await invoke("install_update");
    onStatus({ state: "ready", version });
  } catch (e) {
    onStatus({ state: "error", message: String(e) });
  } finally {
    unlisten();
  }
}

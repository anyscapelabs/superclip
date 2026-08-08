import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { BiCheck, BiErrorCircle, BiLoaderAlt } from "react-icons/bi";
import { checkForUpdates, type UpdateStatus } from "./updater";

function statusLine(s: UpdateStatus | null): { text: string; kind: "pending" | "success" | "error" } {
  switch (s?.state) {
    case "checking":
      return { text: "Checking for updates…", kind: "pending" };
    case "uptodate":
      return { text: `You're up to date on v${s.version}`, kind: "success" };
    case "downloading":
      return { text: `Downloading v${s.version}…`, kind: "pending" };
    case "installing":
      return { text: `Installing v${s.version} — relaunching…`, kind: "pending" };
    case "error":
      return { text: "Update check failed.", kind: "error" };
    default:
      return { text: "Checking for updates…", kind: "pending" };
  }
}

function UpdaterApp() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const line = statusLine(status);

  useEffect(() => {
    const un = listen("updater:check", () => {
      checkForUpdates(setStatus).catch((e) => {
        console.error("update check failed", e);
        setStatus({ state: "error", message: String(e) });
      });
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Dismiss the card automatically once a terminal state (up to date or
  // failure) has been shown long enough for the user to read it.
  useEffect(() => {
    if (status?.state !== "uptodate" && status?.state !== "error") return;
    const t = setTimeout(() => getCurrentWindow().hide(), 2500);
    return () => clearTimeout(t);
  }, [status]);

  return (
    <main className="app">
      <div className="glass">
        <div className="flex flex-col items-center justify-center px-8 py-8 text-center">
          <h1 className="text-lg font-medium text-white">Superclip Update</h1>
          <div className="mt-4 flex items-center gap-2.5">
            {line.kind === "pending" && (
              <BiLoaderAlt className="animate-spin text-xl text-white/70" size={20} />
            )}
            {line.kind === "success" && <BiCheck className="text-xl text-[#863bff]" size={20} />}
            {line.kind === "error" && <BiErrorCircle className="text-xl text-red-400" size={20} />}
            <span className="text-[14px] text-white/75">{line.text}</span>
          </div>
        </div>
      </div>
    </main>
  );
}

export default UpdaterApp;
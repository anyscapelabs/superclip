import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { BiCheck, BiErrorCircle, BiLoaderAlt, BiX } from "react-icons/bi";
import {
  checkForUpdates,
  getChannel,
  restart,
  setChannel,
  type UpdateChannel,
  type UpdateStatus,
} from "./updater";

const CHANNELS: { id: UpdateChannel; label: string; blurb: string }[] = [
  {
    id: "stable",
    label: "Stable",
    blurb: "Tested releases only. Recommended for everyday use.",
  },
  {
    id: "beta",
    label: "Beta",
    blurb: "Pre-release builds, first access to new features. May be unstable.",
  },
];

function StatusRow({ status }: { status: UpdateStatus }) {
  if (status.state === "idle") {
    return <span className="text-[13px] text-white/40">Not checked yet.</span>;
  }

  const spinning = status.state === "checking" || status.state === "downloading";
  const failed = status.state === "error";

  return (
    <div className="flex min-w-0 items-center gap-2.5">
      {spinning && <BiLoaderAlt className="shrink-0 animate-spin text-white/70" size={18} />}
      {failed && <BiErrorCircle className="shrink-0 text-red-400" size={18} />}
      {!spinning && !failed && <BiCheck className="shrink-0 text-[#863bff]" size={18} />}
      <span className="min-w-0 truncate text-[13px] text-white/75">
        {status.state === "checking" && "Checking for updates…"}
        {status.state === "uptodate" && `You're up to date on v${status.version}`}
        {status.state === "downloading" && `Downloading v${status.version}…`}
        {status.state === "ready" && `v${status.version} installed — restart to finish`}
        {status.state === "error" && "Update check failed"}
      </span>
    </div>
  );
}

function UpdaterApp() {
  const [status, setStatus] = useState<UpdateStatus>({ state: "idle" });
  const [channel, setActiveChannel] = useState<UpdateChannel>("stable");
  const [version, setVersion] = useState("");

  const check = useCallback(() => {
    checkForUpdates(setStatus).catch((e) => {
      console.error("update check failed", e);
      setStatus({ state: "error", message: String(e) });
    });
  }, []);

  useEffect(() => {
    getVersion().then(setVersion);
    getChannel().then(setActiveChannel);
  }, []);

  useEffect(() => {
    const un = listen("updater:check", check);
    return () => {
      un.then((f) => f());
    };
  }, [check]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") getCurrentWindow().hide();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const switchChannel = (next: UpdateChannel) => {
    if (next === channel) return;
    setActiveChannel(next);
    setStatus({ state: "idle" });
    setChannel(next).catch((e) => console.error("set_channel:", e));
  };

  const busy = status.state === "checking" || status.state === "downloading";
  const percent = status.state === "downloading" ? status.percent : null;
  const active = CHANNELS.find((c) => c.id === channel);

  return (
    <main className="app">
      <div className="glass px-5 py-4">
        <header className="flex items-center justify-between">
          <div className="flex items-baseline gap-2">
            <h1 className="text-[15px] font-medium text-white">Superclip</h1>
            <span className="text-[12px] text-white/35">{version && `v${version}`}</span>
          </div>
          <button
            type="button"
            aria-label="Close"
            onClick={() => getCurrentWindow().hide()}
            className="cursor-pointer rounded text-white/35 transition-colors hover:text-white"
          >
            <BiX size={20} />
          </button>
        </header>

        <div className="mt-3 border-t border-white/5 pt-3.5">
          <StatusRow status={status} />
          {percent !== null && (
            <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full rounded-full bg-[#863bff] transition-[width] duration-200"
                style={{ width: `${percent}%` }}
              />
            </div>
          )}
        </div>

        <section className="mt-5">
          <h2 className="text-[11px] font-medium uppercase tracking-wider text-white/40">
            Update channel
          </h2>
          <div className="mt-2 flex gap-0.5 rounded-lg bg-black/25 p-0.5">
            {CHANNELS.map((c) => (
              <button
                key={c.id}
                type="button"
                disabled={busy}
                onClick={() => switchChannel(c.id)}
                className={`flex-1 cursor-pointer rounded-[6px] py-1.5 text-[12px] transition-colors disabled:cursor-default ${
                  channel === c.id
                    ? "border border-white/10 bg-white/10 text-white"
                    : "border border-transparent text-white/45 hover:text-white/70"
                }`}
              >
                {c.label}
              </button>
            ))}
          </div>
          <p className="mt-2 text-[11px] leading-[1.45] text-white/35">{active?.blurb}</p>
        </section>

        <footer className="mt-auto flex items-center justify-end pt-4">
          {status.state === "ready" ? (
            <button
              type="button"
              onClick={() => restart()}
              className="cursor-pointer rounded-lg bg-[#863bff] px-3.5 py-1.5 text-[12px] font-medium text-white shadow-[0_2px_0_rgba(0,0,0,0.45)] transition-opacity hover:opacity-90"
            >
              Restart now
            </button>
          ) : (
            <button
              type="button"
              onClick={check}
              disabled={busy}
              className="cursor-pointer rounded-lg border border-white/15 bg-gradient-to-b from-white/20 to-white/5 px-3.5 py-1.5 text-[12px] text-neutral-200 shadow-[0_2px_0_rgba(0,0,0,0.45)] transition-opacity hover:opacity-90 disabled:cursor-default disabled:opacity-40"
            >
              Check for updates
            </button>
          )}
        </footer>
      </div>
    </main>
  );
}

export default UpdaterApp;

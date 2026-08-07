import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import SearchBar from "./components/SearchBar";
import Clipboard from "./components/Clipboard";
import Footer from "./components/Footer";
import type { ClipItem } from "./types";

function relTime(ts: number): string {
  const m = Math.floor((Date.now() - ts) / 60000);
  if (m < 60) return `${Math.max(m, 1)}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

function App() {
  const [items, setItems] = useState<ClipItem[]>([]);
  const [selected, setSelected] = useState(0);

  const refresh = useCallback(async () => {
    setItems(await invoke<ClipItem[]>("get_history"));
  }, []);

  useEffect(() => {
    refresh();
    const un = listen("clipboard-updated", refresh);
    return () => {
      un.then((f) => f());
    };
  }, [refresh]);

  useEffect(() => {
    if (selected >= items.length) setSelected(Math.max(0, items.length - 1));
  }, [items, selected]);

  const paste = useCallback(
    (id: string) => {
      invoke("paste_item", { id }).then(() => {
        refresh();
        setSelected(0);
      });
    },
    [refresh],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, items.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (e.key === "Enter" && items[selected]) {
        e.preventDefault();
        paste(items[selected].id);
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "x") {
        e.preventDefault();
        invoke("clear_history").then(refresh);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [items, selected, paste, refresh]);

  return (
    <main className="app">
      <div className="glass">
        <div className="border-b border-white/5 px-3 pt-4 pb-3">
          <SearchBar />
        </div>
        <Clipboard
          items={items}
          selected={selected}
          onSelect={setSelected}
          onItemClick={paste}
          relTime={relTime}
        />
        <Footer
          count={items.length}
          onPaste={() => items[selected] && paste(items[selected].id)}
          onClear={() => invoke("clear_history").then(refresh)}
        />
      </div>
    </main>
  );
}

export default App;

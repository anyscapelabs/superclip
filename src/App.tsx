import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Fuse from "fuse.js";
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
  const [query, setQuery] = useState("");

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

  const filtered = useMemo(() => {
    const q = query.trim();
    let src = items;
    if (q) {
      const fuse = new Fuse(items, {
        keys: ["text"],
        threshold: 0.35,
        ignoreLocation: true,
      });
      src = fuse.search(q).map((r) => r.item);
    }
    // Pinned items always render above the recent list, keeping their own
    // relative order.
    return [...src.filter((i) => i.pinned), ...src.filter((i) => !i.pinned)];
  }, [items, query]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    if (selected >= filtered.length) setSelected(Math.max(0, filtered.length - 1));
  }, [filtered.length, selected]);

  const paste = useCallback(
    (id: string) => {
      invoke("paste_item", { id }).then(() => {
        refresh();
        setSelected(0);
      });
    },
    [refresh],
  );

  const togglePin = useCallback(
    (id: string) => {
      invoke("toggle_pin", { id }).then(refresh);
    },
    [refresh],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (e.key === "Enter" && filtered[selected]) {
        e.preventDefault();
        paste(filtered[selected].id);
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "x") {
        e.preventDefault();
        invoke("clear_history").then(refresh);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [filtered, selected, paste, refresh]);

  return (
    <main className="app">
      <div className="glass">
        <div className="border-b border-white/5 px-3 pt-4 pb-3">
          <SearchBar value={query} onChange={setQuery} />
        </div>
        <Clipboard
          items={filtered}
          selected={selected}
          onSelect={setSelected}
          onItemClick={paste}
          onTogglePin={togglePin}
          relTime={relTime}
        />
        <Footer
          count={items.length}
          onPaste={() => filtered[selected] && paste(filtered[selected].id)}
          onClear={() => invoke("clear_history").then(refresh)}
        />
      </div>
    </main>
  );
}

export default App;

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { IoImage } from "react-icons/io5";
import type { ClipItem } from "../types";

interface Props {
  item?: ClipItem;
}

const POINTER_SETTLE_MS = 70;
const MAX_CACHED_PREVIEWS = 12;

function base64Bytes(b64: string): number {
  const padding = b64.endsWith("==") ? 2 : b64.endsWith("=") ? 1 : 0;
  return Math.floor((b64.length * 3) / 4) - padding;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)} KB`;
  const mb = kb / 1024;
  return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

function formatStamp(ts: number): string {
  const date = new Date(ts);
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
    hour: "2-digit",
    minute: "2-digit",
  });
}

function contentType(item: ClipItem): string {
  if (item.type === "image") return "PNG image";
  return item.type === "code" ? "Code" : "Text";
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2 py-[3px]">
      <dt className="shrink-0 text-white/35">{label}</dt>
      <dd className="min-w-0 truncate text-right text-white/65">{value}</dd>
    </div>
  );
}

function Preview({ item }: Props) {
  const cache = useRef(new Map<string, string>());
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  const id = item?.id;
  const isImage = item?.type === "image";

  useEffect(() => {
    setFailed(false);
    if (!id || !isImage) {
      setSrc(null);
      return;
    }

    const cached = cache.current.get(id);
    if (cached) {
      setSrc(cached);
      return;
    }

    setSrc(null);
    let live = true;
    const timer = window.setTimeout(() => {
      invoke<string>("get_image", { id })
        .then((b64) => {
          const url = `data:image/png;base64,${b64}`;
          if (cache.current.size >= MAX_CACHED_PREVIEWS) {
            const oldest = cache.current.keys().next().value;
            if (oldest !== undefined) cache.current.delete(oldest);
          }
          cache.current.set(id, url);
          if (live) setSrc(url);
        })
        .catch((e) => {
          console.error("get_image:", e);
          if (live) setFailed(true);
        });
    }, POINTER_SETTLE_MS);

    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [id, isImage]);

  if (!item) {
    return (
      <aside className="flex w-[300px] shrink-0 items-center justify-center border-l border-white/5 text-[12px] text-white/25">
        No preview
      </aside>
    );
  }

  const image = item.image;

  return (
    <aside className="flex w-[300px] shrink-0 flex-col border-l border-white/5">
      <div className="flex min-h-0 flex-1 items-center justify-center p-3">
        {isImage ? (
          <div className="flex h-full w-full items-center justify-center overflow-hidden">
            {src ? (
              <img
                src={src}
                alt={image?.name || "Clipboard image"}
                className="max-h-full max-w-full rounded-md object-contain shadow-[0_2px_12px_rgba(0,0,0,0.35)]"
                draggable={false}
              />
            ) : (
              <div className="flex flex-col items-center gap-1.5 text-white/25">
                <IoImage size={22} />
                <span className="text-[11px]">
                  {failed ? "Preview unavailable" : "Loading…"}
                </span>
              </div>
            )}
          </div>
        ) : (
          <div className="superclip-scroll h-full w-full overflow-y-auto">
            <p
              className={`whitespace-pre-wrap break-words text-[12px] leading-[1.45] text-neutral-300 ${
                item.type === "code" ? "font-mono" : ""
              }`}
            >
              {item.text}
            </p>
          </div>
        )}
      </div>

      <dl className="border-t border-white/5 px-3 py-2 text-[11px]">
        <Row label="Content type" value={contentType(item)} />
        {isImage && (
          <Row
            label="Dimensions"
            value={image ? `${image.width} × ${image.height}` : "—"}
          />
        )}
        {isImage && (
          <Row label="Size" value={src ? formatBytes(base64Bytes(src.split(",")[1] ?? "")) : "—"} />
        )}
        <Row label="Copied" value={formatStamp(item.time)} />
      </dl>
    </aside>
  );
}

export default Preview;

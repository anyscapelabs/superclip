import { useEffect, useRef } from "react";
import { AiFillCode } from "react-icons/ai";
import { TbClipboardTextFilled } from "react-icons/tb";
import type { ClipItem } from "../types";

interface Props {
  items: ClipItem[];
  selected: number;
  onSelect: (i: number) => void;
  onItemClick: (id: string) => void;
  relTime: (ts: number) => string;
}

function Clipboard({ items, selected, onSelect, onItemClick, relTime }: Props) {
  const itemRefs = useRef<(HTMLLIElement | null)[]>([]);

  useEffect(() => {
    itemRefs.current[selected]?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  return (
    <div className="min-h-0 flex-1 overflow-hidden">
      <h2 className="pl-[18px] pr-3 pt-2 pb-1 mt-2 text-xs font-medium uppercase tracking-wider text-white/40">
        Recent
      </h2>
      <ul className="clipmate-scroll min-h-0 flex-1 overflow-y-auto p-1.5 pt-0">
        {items.length === 0 && (
          <li className="px-3 py-2 text-[14px] text-white/35">
            No clipboard items yet — copy something!
          </li>
        )}
        {items.map((item, i) => (
          <li
            key={item.id}
            ref={(el) => {
              itemRefs.current[i] = el;
            }}
            onMouseEnter={() => onSelect(i)}
            onClick={() => onItemClick(item.id)}
            className={`flex cursor-pointer items-center gap-3 rounded-lg px-3 py-2 text-[14px] transition-colors ${
              selected === i
                ? "bg-white/10 text-white"
                : "text-neutral-300 hover:bg-white/5"
            }`}
          >
            {item.type === "code" ? (
              <AiFillCode
                className={`shrink-0 ${selected === i ? "text-white/80" : "text-white/50"}`}
                size={16}
              />
            ) : (
              <TbClipboardTextFilled
                className={`shrink-0 ${selected === i ? "text-white/80" : "text-white/50"}`}
                size={16}
              />
            )}
            <span className="min-w-0 flex-1 truncate">{item.text}</span>
            <span className="shrink-0 text-xs text-white/35">{relTime(item.time)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default Clipboard;

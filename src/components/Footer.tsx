import { PiArrowBendDownRightBold } from "react-icons/pi";
import { BiCommand } from "react-icons/bi";

interface Props {
  count: number;
  onPaste: () => void;
  onClear: () => void;
}

function Footer({ count, onPaste, onClear }: Props) {
  return (
    <footer className="flex items-center justify-between border-t border-white/5 px-3 py-2">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <div className="flex h-5 w-5 items-center justify-center rounded-md border border-white/15 bg-gradient-to-b from-white/20 to-white/5 shadow-[0_2px_0_rgba(0,0,0,0.45)]">
            <BiCommand className="text-white/50" size={12} />
          </div>
          <div className="flex h-5 w-5 items-center justify-center rounded-md border border-white/15 bg-gradient-to-b from-white/20 to-white/5 text-[11px] text-neutral-300 shadow-[0_2px_0_rgba(0,0,0,0.45)]">
            P
          </div>
          <span className="ml-1 text-xs text-neutral-300">Pin</span>
        </div>

        <button
          type="button"
          onClick={onPaste}
          className="flex cursor-pointer items-center gap-1.5"
        >
          <div className="flex h-5 w-5 items-center justify-center rounded-md border border-white/15 bg-gradient-to-b from-white/20 to-white/5 shadow-[0_2px_0_rgba(0,0,0,0.45)]">
            <PiArrowBendDownRightBold className="text-white/50" size={12} />
          </div>
          <span className="ml-1 text-xs text-neutral-300">Paste</span>
        </button>

        <button
          type="button"
          onClick={onClear}
          className="flex cursor-pointer items-center gap-1.5"
        >
          <div className="flex h-5 w-5 items-center justify-center rounded-md border border-white/15 bg-gradient-to-b from-white/20 to-white/5 shadow-[0_2px_0_rgba(0,0,0,0.45)]">
            <BiCommand className="text-white/50" size={12} />
          </div>
          <div className="flex h-5 w-5 items-center justify-center rounded-md border border-white/15 bg-gradient-to-b from-white/20 to-white/5 text-[11px] text-neutral-300 shadow-[0_2px_0_rgba(0,0,0,0.45)]">
            X
          </div>
          <span className="ml-1 text-xs text-neutral-300">Clear</span>
        </button>
      </div>

      <span className="text-xs text-white/40">{count} items</span>
    </footer>
  );
}

export default Footer;

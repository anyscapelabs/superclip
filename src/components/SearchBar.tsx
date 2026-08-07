import { FiSearch } from "react-icons/fi";

interface Props {
  value: string;
  onChange: (v: string) => void;
}

function SearchBar({ value, onChange }: Props) {
  return (
    <div className="flex w-full items-center gap-2 pl-2">
      <FiSearch className="shrink-0 text-white/50" size={16} />
      <input
        className="min-w-0 flex-1 bg-transparent text-[15px] leading-none text-neutral-200 outline-none placeholder:text-white/35"
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Search clipboard history…"
        spellCheck={false}
      />
    </div>
  );
}

export default SearchBar;

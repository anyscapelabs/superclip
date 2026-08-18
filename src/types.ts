export interface ClipItem {
  id: string;
  text: string;
  type: "text" | "code" | "image";
  time: number;
  pinned: boolean;
  image?: ImageEntry | null;
}

export interface ImageEntry {
  path: string;
  width: number;
  height: number;
  name: string;
}

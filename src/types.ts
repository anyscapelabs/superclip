export interface ClipItem {
  id: string;
  text: string;
  type: "text" | "code";
  time: number;
  pinned: boolean;
}

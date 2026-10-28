import type { EntryDto } from "../types/index.js";

export type FileIconKind = "folder" | "archive" | "text" | "disc" | "game" | "image" | "audio" | "file";
const archive = new Set(["zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst", "tgz"]);
const text = new Set(["txt", "md", "nfo", "log", "ini", "cfg", "json", "xml", "yaml", "yml", "csv", "cue", "gdi", "m3u", "m3u8"]);
const disc = new Set(["iso", "chd", "cso", "rvz", "gcz", "nrg", "mdf"]);
const game = new Set(["nes", "sfc", "smc", "gb", "gbc", "gba", "nds", "n64", "z64", "v64", "sms", "gg", "gen", "pce", "rom"]);
const image = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"]);
const audio = new Set(["mp3", "flac", "wav", "ogg", "m4a"]);

export function fileIconKind(entry: Pick<EntryDto, "isDir" | "name" | "extension">): FileIconKind {
  if (entry.isDir) return "folder";
  const ext = (entry.extension || entry.name.match(/\.([^.]+)$/)?.[1] || "").replace(/^\./, "").toLowerCase();
  if (archive.has(ext)) return "archive";
  if (text.has(ext)) return "text";
  if (disc.has(ext)) return "disc";
  if (game.has(ext)) return "game";
  if (image.has(ext)) return "image";
  if (audio.has(ext)) return "audio";
  return "file";
}

import { AppId } from '../types';
import { openApp } from '../stores/windowManager';
import { SystemBridge, shellQuote } from './systemBridge';
import { configStore } from './configStore';

/**
 * ONE place that decides which application opens a file — used by Explorer,
 * the Desktop and anything else that "double-clicks" a file.
 *
 * Previously only text/* reached an in-shell app; images went to a read-only
 * preview pane and EVERYTHING else (archives, video, audio…) was handed to
 * `xdg-open`, i.e. to whatever the host distro had registered — never to
 * Blue Images / Blue Archive / Blue Video / Blue Music. Now the shell's own
 * apps are the defaults, and `xdg-open` is only the last resort for formats
 * the shell has no app for (PDF, office documents…).
 */
export interface OpenableFile { path: string; name: string; mime_type?: string; }

const ext = (n: string) => (n.includes('.') ? n.slice(n.lastIndexOf('.') + 1).toLowerCase() : '');
const IMAGE_EXT = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'svg', 'avif', 'tiff', 'tif', 'ico']);
const VIDEO_EXT = new Set(['mp4', 'webm', 'mkv', 'avi', 'mov', 'flv', 'wmv', 'm4v', 'ogv']);
const AUDIO_EXT = new Set(['mp3', 'flac', 'wav', 'ogg', 'oga', 'opus', 'm4a', 'aac', 'wma']);
const ARCHIVE_EXT = new Set(['zip', 'tar', 'gz', 'bz2', 'xz', '7z', 'rar', 'zst', 'lz4', 'tgz', 'tbz2', 'txz']);
const HACKER_EXT = new Set(['h#', 'hl', 'hcs', 'hk', 'hacker']);
const ARCHIVE_MIME = /(zip|x-tar|x-7z|x-rar|gzip|x-bzip|x-xz|zstd|x-compressed)/;

export type FileKind = 'image' | 'video' | 'audio' | 'archive' | 'text' | 'hacker' | 'blue' | 'other';

export function classifyFile(f: OpenableFile): FileKind {
  const e = ext(f.name);
  const m = f.mime_type ?? '';
  if (e === 'blue') return 'blue';
  if (HACKER_EXT.has(e)) return 'hacker';
  // `.ts` is ambiguous (TypeScript vs MPEG transport stream): trust the MIME type.
  if (m.startsWith('image/') || IMAGE_EXT.has(e)) return 'image';
  if (m.startsWith('video/') || (VIDEO_EXT.has(e))) return 'video';
  if (m.startsWith('audio/') || AUDIO_EXT.has(e)) return 'audio';
  if (ARCHIVE_EXT.has(e) || ARCHIVE_MIME.test(m) || /\.tar\.(gz|xz|bz2|zst)$/i.test(f.name)) return 'archive';
  if (m.startsWith('text/') || m === 'application/json' || m === 'application/xml' || m.endsWith('+xml') || m === 'application/x-shellscript') return 'text';
  return 'other';
}

function textEditorApp() {
  return (configStore.get().defaultTextEditor ?? 'notepad') === 'blue_code' ? AppId.BLUE_CODE : AppId.NOTEPAD;
}

/** Returns false when no in-shell app handles the file (caller may show its own fallback). */
export async function openFileWithDefaultApp(f: OpenableFile, openBlueManifest?: (f: OpenableFile) => void): Promise<boolean> {
  const launch = (app: AppId) => openApp(app, false, undefined, { openPath: f.path });
  switch (classifyFile(f)) {
    case 'image': launch(AppId.BLUE_IMAGES); return true;
    case 'video': launch(AppId.BLUE_VIDEOS); return true;
    case 'audio': launch(AppId.BLUE_MUSIC); return true;
    case 'archive': launch(AppId.BLUE_ARCHIVE); return true;
    case 'text':
    case 'hacker': openApp(textEditorApp(), false, undefined, { openPath: f.path }); return true;
    case 'blue':
      if (openBlueManifest) openBlueManifest(f); else openApp(textEditorApp(), false, undefined, { openPath: f.path });
      return true;
    default:
      // Last resort: the host's default handler (PDF reader, office suite…).
      await SystemBridge.executeCommand(`xdg-open ${shellQuote(f.path)} >/dev/null 2>&1 &`).catch(() => {});
      return false;
  }
}

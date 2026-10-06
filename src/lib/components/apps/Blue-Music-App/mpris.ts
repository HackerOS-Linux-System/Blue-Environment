import { SystemBridge } from '../../../utils/systemBridge';

export interface MprisPush {
  hasTrack: boolean; playing: boolean; title: string; artist: string; album: string; artUrl: string;
  lengthSecs: number; positionSecs: number; volume: number; shuffle: boolean; loopStatus: 'None' | 'Track' | 'Playlist';
  canNext: boolean; canPrev: boolean; seeked?: boolean;
}
export type MprisCmd =
  | { cmd: 'play' } | { cmd: 'pause' } | { cmd: 'playPause' } | { cmd: 'stop' } | { cmd: 'next' } | { cmd: 'previous' } | { cmd: 'raise' }
  | { cmd: 'seek'; value: number } | { cmd: 'setPosition'; value: number } | { cmd: 'setVolume'; value: number }
  | { cmd: 'setShuffle'; value: boolean } | { cmd: 'setLoop'; value: string };

/** Registers the player on the session bus and routes remote commands to `onCommand`. Resolves to a disposer. */
export async function startMpris(onCommand: (c: MprisCmd) => void): Promise<() => void> {
  if (!SystemBridge.isTauri()) return () => {};
  let unlisten: (() => void) | null = null;
  try {
    const { listen } = await import('@tauri-apps/api/event');
    unlisten = await listen<MprisCmd>('music-mpris', (e) => onCommand(e.payload));
    await SystemBridge.invokeCommand('music_mpris_start');
  } catch { /* no session bus — the player simply isn't remote-controllable */ }
  return () => { unlisten?.(); SystemBridge.invokeCommand('music_mpris_stop').catch(() => {}); };
}

export const pushMpris = (u: MprisPush): Promise<unknown> => SystemBridge.invokeCommand('music_mpris_update', { update: u }).catch(() => {});

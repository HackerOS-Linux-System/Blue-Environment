export type RemoteAction = 'back' | 'home' | 'menu' | 'select' | 'play_pause' | 'next' | 'prev' | 'stop';

interface KeyLike { key?: string; code?: string; keyCode?: number }

// By `KeyboardEvent.key`. Several spellings per button because browsers/toolkits
// name the same evdev key differently (BrowserBack vs XF86Back vs GoBack).
const BY_KEY: Record<string, RemoteAction> = {
  BrowserBack: 'back', GoBack: 'back', XF86Back: 'back', Back: 'back', Exit: 'back', Escape: 'back',
  Home: 'home', BrowserHome: 'home', XF86HomePage: 'home', XF86Home: 'home', GoHome: 'home',
  ContextMenu: 'menu', Menu: 'menu', MediaTopMenu: 'menu', Guide: 'menu', Info: 'menu', F1: 'menu', Settings: 'menu',
  Select: 'select',
  MediaPlayPause: 'play_pause', MediaPlay: 'play_pause', MediaPause: 'play_pause',
  MediaTrackNext: 'next', MediaFastForward: 'next',
  MediaTrackPrevious: 'prev', MediaRewind: 'prev',
  MediaStop: 'stop',
};

// Legacy `keyCode`s that smart-TV remotes still send (the web has no names for them).
const BY_KEYCODE: Record<number, RemoteAction> = {
  461: 'back',    // LG webOS Back
  10009: 'back',  // Samsung Tizen Back
  27: 'back',     // Esc
  36: 'home',     // Home
  457: 'menu',    // Info (webOS)
  10252: 'play_pause', // Tizen MediaPlayPause
  415: 'play_pause', 19: 'play_pause', 179: 'play_pause',
  417: 'next', 412: 'prev', 413: 'stop',
};

/** The action for a key event, or `null` if it isn't a remote/TV key. */
export function remoteAction(ev: KeyLike): RemoteAction | null {
  if (ev.key && BY_KEY[ev.key]) return BY_KEY[ev.key];
  if (ev.keyCode !== undefined && BY_KEYCODE[ev.keyCode]) return BY_KEYCODE[ev.keyCode];
  return null;
}

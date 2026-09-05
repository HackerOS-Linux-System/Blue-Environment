export enum AppId {
  TERMINAL = 'terminal',
  AI_ASSISTANT = 'ai_assistant',
  EXPLORER = 'explorer',
  SETTINGS = 'settings',
  ABOUT = 'about',
  BLUE_WEB = 'blue_web',
  CALCULATOR = 'calculator',
  SYSTEM_MONITOR = 'system_monitor',
  NOTEPAD = 'notepad',
  BLUE_DOCS = 'blue_docs',
  BLUE_CODE = 'blue_code',
  BLUE_SOFTWARE = 'blue_software',
  MAIL = 'mail',
  BLUE_EDIT = 'blue_edit',
  BLUE_IMAGES = 'blue_images',
  BLUE_VIDEOS = 'blue_videos',
  BLUE_MUSIC = 'blue_music',
  BLUE_SCREEN = 'blue_screen',
  BLUE_ARCHIVE = 'blue_archive',
  BLUE_TRANSLATE = 'blue_translate',
  BLUE_INSTALLER = 'blue_installer',
  BLUE_PARTITION_MANAGER = 'blue_partition_manager',
  BLUE_DOWNLOADER = 'blue_downloader',
  CAMERA = 'camera',
  BLUE_PLAY = 'blue_play',
  BLUE_CALENDAR = 'blue_calendar',
  BLUE_TASKS = 'blue_tasks',
  BLUE_NOTIFICATIONS = 'blue_notifications',
  BLUE_WELCOME = 'blue_welcome',
  BLUE_EMOJI = 'blue_emoji',
  BLUE_NEWS = 'blue_news',
  BLUE_MESSAGES = 'blue_messages',
  BLUE_CONNECT = 'blue_connect',
  BLUE_ACCOUNTS = 'blue_accounts',
  BLUE_VIRT = 'blue_virt',
  EXTERNAL = 'external',
}

export interface DesktopEntry {
  id: string;
  name: string;
  comment: string;
  icon: string;
  exec: string;
  categories: string[];
  desktop_file?: string;
  is_external?: boolean;
}

export interface AppDefinition {
  id: AppId | string;
  title: string;
  /** A Svelte component constructor (icon), or a string URL/path. */
  icon: any;
  /** A Svelte component constructor for the app's window content. `null` = not yet ported to Svelte. */
  component?: any;
  isExternal?: boolean;
  externalPath?: string;
  defaultWidth?: number;
  defaultHeight?: number;
}

export interface WindowState {
  id: string;
  appId: string;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
  isMinimized: boolean;
  isMaximized: boolean;
  /** Android-style Picture-in-Picture: small, floating, always-on-top. */
  isPiP?: boolean;
  prePiPGeometry?: { x: number; y: number; width: number; height: number };
  zIndex: number;
  isExternal: boolean;
  workspace: number;
  externalWindowId?: string;
  pid?: number;
  /** Extra data passed to the app at launch time — e.g. `{ openPath }`
   * so Notepad/Blue Code can open a specific file immediately instead
   * of starting empty. Spread onto the app component's props alongside
   * `windowId` (see App.svelte). */
  launchArgs?: Record<string, unknown>;
}

export interface AppsEnabled {
  blueAI: boolean;
  blueCode: boolean;
  blueSoftware: boolean;
  mail: boolean;
  calculator: boolean;
  notepad: boolean;
  systemMonitor: boolean;
  explorer: boolean;
  terminal: boolean;
  blueWeb: boolean;
  about: boolean;
}

export interface AIConfig {
  service: string;
  model: string;
  apiKey: string;
  /** True once the user has been through the provider/model picker at
   * least once. Used to decide whether to show the Setup screen again
   * on next open. */
  configured?: boolean;
  /** If false, Blue AI shows the Setup screen every time it's opened
   * instead of remembering the last choice. */
  rememberChoice?: boolean;
}

export interface UserConfig {
  wallpaper: string;
  theme: string;
  themeName: string;
  accentColor: string;
  displayScale: number;
  customThemes?: ThemeDefinition[];
  /** See systemBridge.ts's UserConfig — same field, duplicated here
   * since this file keeps its own parallel UserConfig definition
   * rather than importing systemBridge's. */
  shellThemeId?: string;
  installedPlugins?: import('./data/builtinPlugins').InstalledPlugin[];
  desktopPath: string;
  panelEnabled: boolean;
  panelPosition: string;
  panelSize: number;
  panelOpacity: number;
  language: string;
  nightLightEnabled: boolean;
  nightLightTemperature: number;
  nightLightSchedule: 'manual' | 'sunset';
  nightLightStartHour: number;
  nightLightEndHour: number;
  appsEnabled?: Record<string, boolean>;
  aiConfig?: AIConfig;
  accounts?: Record<string, any>;
  /** User-defined Explorer sidebar shortcuts (absolute or HOME-relative paths). */
  customBookmarks?: string[];
  weatherEnabled?: boolean;
  weatherCity?: string;
  weatherUnit?: 'celsius' | 'fahrenheit';
  clipboardHoverPreviewEnabled?: boolean;
  networkHoverInfoEnabled?: boolean;
  /** App IDs pinned to the center of the panel. Previously only settable
   * by hand-editing the config file — now has real Settings UI (see
   * PanelSection.svelte). */
  pinnedApps?: string[];
  /** Which app opens when you double-click a text file in Explorer.
   * Previously this always fell back to a read-only preview pane inside
   * Explorer itself — never a real editor. */
  defaultTextEditor?: 'notepad' | 'blue_code';
  /** Per-game stats for Blue Play's built-in original games, keyed by
   * game id (e.g. 'snake', '2048'). Also used for external games (see
   * blueGamesLibrary) so both share one recently-played/high-score model. */
  blueGames?: Record<string, { highScore: number; playCount: number; lastPlayed?: string; playtimeSeconds?: number }>;
  /** User-added external games (native Linux binaries or Windows .exe
   * run through Wine/Proton/umu), managed from Blue Play's Library tab. */
  blueGamesLibrary?: BlueGameLibraryEntry[];
  /** Desktop/file-manager icon size in pixels — see the Icons settings
   * section (formerly "Personalization"). */
  iconSize?: number;
  /** X cursor theme name, applied via `set_cursor_theme` (writes
   * `~/.icons/default/index.theme`) — system-wide, not just Blue
   * Environment's own UI. */
  cursorTheme?: string;
  /** Selected filesystem theme package id (`/usr/share/themes/<id>/`),
   * applied globally by SystemThemeStyle.svelte — independent of
   * `shellThemeId` (an app-bundled theme), see that component's doc. */
  systemThemeId?: string | null;
}

export interface ThemeDefinition {
  id: string;
  name: string;
  type?: 'builtin' | 'custom';
  css?: string;
  colors: Record<string, string>;
}

/** A filesystem theme package (`/usr/share/themes/<id>/`) as returned by
 * `list_system_themes`/`load_system_theme` — see src-tauri/src/themes.rs.
 * Distinct from `ThemeDefinition` (custom accent themes) and
 * `ShellTheme` in builtinThemes.ts (app-bundled shell themes). */
export interface SystemTheme {
  id: string;
  name: string;
  author: string;
  version: string;
  description: string;
  effects: {
    blur: boolean;
    transparency: boolean;
    animations: boolean;
    cornerStyle: string;
    accentColor?: string;
  };
  css: string;
  previewDataUrl?: string;
}

export interface PowerProfile {
  name: string;
  active: boolean;
  icon?: string;
  description: string;
}

export interface AppProps {
  windowId: string;
  id?: string;
  onClose?: () => void;
}

export interface Notification {
  id: string;
  title: string;
  message?: string;
  body?: string;
  appId?: string;
  app?: string;
  timestamp: number;
  read: boolean;
  icon?: string;
  actions?: { label: string; action: string }[];
}

export interface SystemStats {
  cpu: number;
  ram: number;
  battery: number;
  isCharging: boolean;
  volume: number;
  brightness: number;
  wifiSSID: string;
  kernel: string;
  sessionType: string;
  uptime?: number;
  totalRam?: number;
  cpuModel?: string;
  diskUsage?: string;
  gpuModel?: string;
  hostname?: string;
}

export interface BlueGameLibraryEntry {
  id: string;
  title: string;
  kind: 'native' | 'windows';
  execPath: string;
  /** Only meaningful for kind === 'windows': which runtime to launch it
   * with, and that runtime's resolved binary path (for Proton, which
   * needs a specific version's binary, not just "proton" on PATH). */
  runtime?: 'wine' | 'proton' | 'umu';
  runtimePath?: string;
  addedAt: string;
}

export interface AIMessage {
  role: 'user' | 'assistant';
  content: string;
}

export interface ExternalWindow {
  id: string;
  pid: number;
  title: string;
  class: string;
  iconPath: string;
  isMinimized: boolean;
  desktop: number;
}

export interface AICallRequest {
  service: string;
  apiKey: string;
  model: string;
  messages: AIMessage[];
}

export type PowerAction = 'shutdown' | 'reboot' | 'suspend' | 'hibernate';

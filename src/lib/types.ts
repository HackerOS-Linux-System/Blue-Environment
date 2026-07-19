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
  CAMERA = 'camera',
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
}

export interface UserConfig {
  wallpaper: string;
  theme: string;
  themeName: string;
  accentColor: string;
  displayScale: number;
  customThemes?: ThemeDefinition[];
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
}

export interface ThemeDefinition {
  id: string;
  name: string;
  type?: 'builtin' | 'custom';
  css?: string;
  colors: Record<string, string>;
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

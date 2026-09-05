import {
  Terminal, Bot, FolderOpen, Settings, Info, Box, Globe, Calculator, Activity,
  FileText, FileCode, Package, Mail, Camera, Image, Video, Archive, ScanLine, Languages, HardDrive, Gamepad2,
  CalendarDays,
  ListChecks,
  Bell,
  Sparkles,
  Smile,
  Music,
  Newspaper,
  MessageSquare,
  Radio,
  KeyRound,
  MonitorPlay,
  Download,
} from 'lucide-svelte';
import type { AppDefinition } from './types';
import { AppId } from './types';

import CalculatorApp from './components/apps/CalculatorApp.svelte';
import AboutApp from './components/apps/AboutApp.svelte';
import NotepadApp from './components/apps/NotepadApp/NotepadApp.svelte';
import CameraApp from './components/apps/CameraApp.svelte';
import TerminalAppLazy from './components/apps/Terminal-App/TerminalAppLazy.svelte';
import BlueCalendarAppLazy from './components/apps/Blue-Calendar-App/BlueCalendarAppLazy.svelte';
import BlueMusicApp from './components/apps/Blue-Music-App/BlueMusicApp.svelte';
import BlueTasksAppLazy from './components/apps/Blue-Tasks-App/BlueTasksAppLazy.svelte';
import BlueNotificationsAppLazy from './components/apps/Blue-Notifications-App/BlueNotificationsAppLazy.svelte';
import BlueWelcomeAppLazy from './components/apps/Blue-Welcome-App/BlueWelcomeAppLazy.svelte';
import BlueEmojiAppLazy from './components/apps/Blue-Emoji-App/BlueEmojiAppLazy.svelte';
import BlueNewsAppLazy from './components/apps/Blue-News-App/BlueNewsAppLazy.svelte';
import BlueAI from './components/apps/Blue-AI/BlueAI.svelte';
import BlueVideoApp from './components/apps/Blue-Video/BlueVideoApp.svelte';
import BlueArchiveApp from './components/apps/Blue-Archive-App/BlueArchiveApp.svelte';
import BlueDownloaderApp from './components/apps/Blue-Downloader-App/BlueDownloaderApp.svelte';
import BlueSoftwareApp from './components/apps/Blue-Software/BlueSoftwareApp.svelte';
import BlueWebApp from './components/apps/Blue-Web/BlueWebApp.svelte';
import BlueImagesApp from './components/apps/Blue-Images-App/BlueImagesApp.svelte';
import BlueScreenshot from './components/apps/Blue-Screenshot/BlueScreenshot.svelte';
import SystemMonitorApp from './components/apps/System-Monitor-App/SystemMonitorApp.svelte';
import ExplorerApp from './components/apps/Explorer-App/ExplorerApp.svelte';
import MailApp from './components/apps/Mail-App/MailApp.svelte';
import SettingsApp from './components/apps/Settings-App/SettingsApp.svelte';
import BlueDocsApp from './components/apps/Blue-Docs-App/BlueDocsApp.svelte';
import BlueCodeAppLazy from './components/apps/Blue-Code-App/BlueCodeAppLazy.svelte';
import BlueTranslateApp from './components/apps/Blue-Translate/BlueTranslateApp.svelte';
import BluePartitionManager from './components/apps/Blue-Partition-Manager/BluePartitionManager.svelte';
import BluePlay from './components/apps/Blue-Play/BluePlay.svelte';
import BlueMessagesAppLazy from './components/apps/Blue-Messages-App/BlueMessagesAppLazy.svelte';
import BlueConnectAppLazy from './components/apps/Blue-Connect/default/BlueConnectAppLazy.svelte';
import BlueAccountsAppLazy from './components/apps/Blue-Accounts/BlueAccountsAppLazy.svelte';
import BlueVirtAppLazy from './components/apps/Blue-Virt/BlueVirtAppLazy.svelte';

// Not currently imported anywhere in the app (checked — configStore.ts/
// App.svelte resolve the wallpaper dynamically via
// `SystemBridge.resolveDefaultWallpaper()` instead, see that function's
// doc). Left defined for now since removing an exported constant is a
// bigger, separately-reviewable change than fixing what it points at —
// but no longer a hardcoded path that might not exist on the running
// system (that was a real, reproduced bug when this constant *was*
// still wired into App.svelte's initial wallpaper value — see that
// file's own fix). Empty string, matching `configStore.ts`'s
// `DEFAULT_CONFIG.wallpaper` convention for "resolve this for real
// before trusting it."
export const WALLPAPER_URL = '';

export const THEMES = {
  'blue-default': { name: 'Blue Glass', bg: 'bg-slate-900', accent: 'blue' },
  cyberpunk: { name: 'Cyberpunk', bg: 'bg-zinc-950', accent: 'yellow' },
  dracula: { name: 'Dracula', bg: 'bg-[#282a36]', accent: 'purple' },
  'light-glass': { name: 'Light Glass', bg: 'bg-slate-200', accent: 'blue' },
};

export const APPS: Record<AppId, AppDefinition> = {
  [AppId.TERMINAL]: { id: AppId.TERMINAL, title: 'Terminal', icon: Terminal, component: TerminalAppLazy, defaultWidth: 680, defaultHeight: 480 },
  [AppId.BLUE_WEB]: { id: AppId.BLUE_WEB, title: 'Blue Web', icon: Globe, component: BlueWebApp, defaultWidth: 1000, defaultHeight: 700 },
  [AppId.EXPLORER]: { id: AppId.EXPLORER, title: 'Files', icon: FolderOpen, component: ExplorerApp, defaultWidth: 820, defaultHeight: 560 },
  [AppId.CALCULATOR]: { id: AppId.CALCULATOR, title: 'Calculator', icon: Calculator, component: CalculatorApp, defaultWidth: 320, defaultHeight: 460 },
  [AppId.CAMERA]: { id: AppId.CAMERA, title: 'Camera', icon: Camera, component: CameraApp, defaultWidth: 720, defaultHeight: 560 },
  [AppId.SYSTEM_MONITOR]: { id: AppId.SYSTEM_MONITOR, title: 'System Monitor', icon: Activity, component: SystemMonitorApp, defaultWidth: 820, defaultHeight: 600 },
  [AppId.AI_ASSISTANT]: { id: AppId.AI_ASSISTANT, title: 'Blue AI', icon: Bot, component: BlueAI, defaultWidth: 500, defaultHeight: 700 },
  [AppId.SETTINGS]: { id: AppId.SETTINGS, title: 'Settings', icon: Settings, component: SettingsApp, defaultWidth: 860, defaultHeight: 620 },
  [AppId.ABOUT]: { id: AppId.ABOUT, title: 'About Blue', icon: Info, component: AboutApp, defaultWidth: 420, defaultHeight: 360 },
  [AppId.NOTEPAD]: { id: AppId.NOTEPAD, title: 'Notepad', icon: FileText, component: NotepadApp, defaultWidth: 600, defaultHeight: 400 },
  [AppId.BLUE_DOCS]: { id: AppId.BLUE_DOCS, title: 'Blue Docs', icon: FileText, component: BlueDocsApp, defaultWidth: 980, defaultHeight: 720 },
  [AppId.BLUE_CODE]: { id: AppId.BLUE_CODE, title: 'Blue Code', icon: FileCode, component: BlueCodeAppLazy, defaultWidth: 900, defaultHeight: 700 },
  [AppId.BLUE_SOFTWARE]: { id: AppId.BLUE_SOFTWARE, title: 'Blue Software', icon: Package, component: BlueSoftwareApp, defaultWidth: 800, defaultHeight: 600 },
  [AppId.MAIL]: { id: AppId.MAIL, title: 'Mail', icon: Mail, component: MailApp, defaultWidth: 1000, defaultHeight: 700 },
  [AppId.EXTERNAL]: { id: AppId.EXTERNAL, title: 'External App', icon: Box, isExternal: true },
  [AppId.BLUE_EDIT]: { id: AppId.BLUE_EDIT, title: 'Blue Edit', icon: Box, isExternal: true, externalPath: 'blue-edit' },
  [AppId.BLUE_IMAGES]: { id: AppId.BLUE_IMAGES, title: 'Blue Images', icon: Image, component: BlueImagesApp, defaultWidth: 900, defaultHeight: 640 },
  [AppId.BLUE_VIDEOS]: { id: AppId.BLUE_VIDEOS, title: 'Blue Video', icon: Video, component: BlueVideoApp, defaultWidth: 900, defaultHeight: 640 },
  [AppId.BLUE_CALENDAR]: { id: AppId.BLUE_CALENDAR, title: 'Blue Calendar', icon: CalendarDays, component: BlueCalendarAppLazy, defaultWidth: 900, defaultHeight: 640 },
  [AppId.BLUE_TASKS]: { id: AppId.BLUE_TASKS, title: 'Blue Tasks', icon: ListChecks, component: BlueTasksAppLazy, defaultWidth: 760, defaultHeight: 600 },
  [AppId.BLUE_NOTIFICATIONS]: { id: AppId.BLUE_NOTIFICATIONS, title: 'Blue Notifications', icon: Bell, component: BlueNotificationsAppLazy, defaultWidth: 620, defaultHeight: 560 },
  [AppId.BLUE_WELCOME]: { id: AppId.BLUE_WELCOME, title: 'Blue Welcome', icon: Sparkles, component: BlueWelcomeAppLazy, defaultWidth: 640, defaultHeight: 560 },
  [AppId.BLUE_EMOJI]: { id: AppId.BLUE_EMOJI, title: 'Blue Emoji', icon: Smile, component: BlueEmojiAppLazy, defaultWidth: 380, defaultHeight: 520 },
  [AppId.BLUE_NEWS]: { id: AppId.BLUE_NEWS, title: 'Blue News', icon: Newspaper, component: BlueNewsAppLazy, defaultWidth: 920, defaultHeight: 640 },
  [AppId.BLUE_MUSIC]: { id: AppId.BLUE_MUSIC, title: 'Blue Music', icon: Music, component: BlueMusicApp, defaultWidth: 420, defaultHeight: 620 },
  [AppId.BLUE_SCREEN]: { id: AppId.BLUE_SCREEN, title: 'Blue Screenshot', icon: ScanLine, component: BlueScreenshot, defaultWidth: 760, defaultHeight: 600 },
  [AppId.BLUE_ARCHIVE]: { id: AppId.BLUE_ARCHIVE, title: 'Blue Archive', icon: Archive, component: BlueArchiveApp, defaultWidth: 760, defaultHeight: 560 },
  [AppId.BLUE_DOWNLOADER]: { id: AppId.BLUE_DOWNLOADER, title: 'Blue Downloader', icon: Download, component: BlueDownloaderApp, defaultWidth: 480, defaultHeight: 620 },
  [AppId.BLUE_TRANSLATE]: { id: AppId.BLUE_TRANSLATE, title: 'Translate', icon: Languages, component: BlueTranslateApp, defaultWidth: 760, defaultHeight: 520 },
  [AppId.BLUE_INSTALLER]: { id: AppId.BLUE_INSTALLER, title: 'Install Blue Environment', icon: Box, isExternal: true },
  [AppId.BLUE_PARTITION_MANAGER]: { id: AppId.BLUE_PARTITION_MANAGER, title: 'Blue Partition Manager', icon: HardDrive, component: BluePartitionManager, defaultWidth: 820, defaultHeight: 600 },
  [AppId.BLUE_PLAY]: { id: AppId.BLUE_PLAY, title: 'Blue Play', icon: Gamepad2, component: BluePlay, defaultWidth: 780, defaultHeight: 640 },
  [AppId.BLUE_MESSAGES]: { id: AppId.BLUE_MESSAGES, title: 'Blue Messages', icon: MessageSquare, component: BlueMessagesAppLazy, defaultWidth: 880, defaultHeight: 640 },
  [AppId.BLUE_CONNECT]: { id: AppId.BLUE_CONNECT, title: 'Blue Connect', icon: Radio, component: BlueConnectAppLazy, defaultWidth: 720, defaultHeight: 560 },
  [AppId.BLUE_ACCOUNTS]: { id: AppId.BLUE_ACCOUNTS, title: 'Blue Accounts', icon: KeyRound, component: BlueAccountsAppLazy, defaultWidth: 820, defaultHeight: 600 },
  [AppId.BLUE_VIRT]: { id: AppId.BLUE_VIRT, title: 'Blue Virt', icon: MonitorPlay, component: BlueVirtAppLazy, defaultWidth: 900, defaultHeight: 640 },
};

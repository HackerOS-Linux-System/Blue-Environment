import { Terminal, Bot, FolderOpen, Settings, Info, Box, Globe, Calculator, Activity, Monitor, Smartphone } from 'lucide-react';
import { AppDefinition, AppId } from './types';

import GeminiAssistantApp from './components/apps/GeminiAssistantApp';
import ExplorerApp from './components/apps/ExplorerApp';
import SettingsApp from './components/apps/SettingsApp';
import AboutApp from './components/apps/AboutApp';
import BlueWebApp from './components/apps/BlueWebApp';
import CalculatorApp from './components/apps/CalculatorApp';
import SystemMonitorApp from './components/apps/SystemMonitorApp';
import BlueConnectApp from './components/apps/BlueConnectApp';

export const WALLPAPER_URL = "https://images.unsplash.com/photo-1451187580459-43490279c0fa?q=80&w=2072&auto=format&fit=crop";

export const THEMES = {
    'blue-default': { name: 'Blue Glass',  bg: 'bg-slate-900',    accent: 'blue'   },
    'cyberpunk':    { name: 'Cyberpunk',   bg: 'bg-zinc-950',     accent: 'yellow' },
    'dracula':      { name: 'Dracula',     bg: 'bg-[#282a36]',    accent: 'purple' },
    'light-glass':  { name: 'Light Glass', bg: 'bg-slate-200',    accent: 'blue'   },
};

// ── Blue Edit, Blue Screen, Blue Media are now EXTERNAL apps ──────────────
// Their binaries live in ~/.hackeros/Blue-Environment/apps/<name>/<name>
// They are launched via launch_process like any system app
// ─────────────────────────────────────────────────────────────────────────

export const APPS: Record<AppId, AppDefinition> = {
    [AppId.TERMINAL]: {
        id: AppId.TERMINAL,
        title: 'Terminal',
        icon: Terminal,
        isExternal: true,
        component: undefined,
    },
    [AppId.BLUE_WEB]: {
        id: AppId.BLUE_WEB,
        title: 'Blue Web',
        icon: Globe,
        component: BlueWebApp,
        defaultWidth: 1000,
        defaultHeight: 700,
    },
    [AppId.EXPLORER]: {
        id: AppId.EXPLORER,
        title: 'Files',
        icon: FolderOpen,
        component: ExplorerApp,
        defaultWidth: 820,
        defaultHeight: 560,
    },
    [AppId.CALCULATOR]: {
        id: AppId.CALCULATOR,
        title: 'Calculator',
        icon: Calculator,
        component: CalculatorApp,
        defaultWidth: 320,
        defaultHeight: 460,
    },
    [AppId.SYSTEM_MONITOR]: {
        id: AppId.SYSTEM_MONITOR,
        title: 'System Monitor',
        icon: Activity,
        component: SystemMonitorApp,
        defaultWidth: 820,
        defaultHeight: 600,
    },
    [AppId.BLUE_CONNECT]: {
        id: AppId.BLUE_CONNECT,
        title: 'Blue Connect',
        icon: Smartphone,
        component: BlueConnectApp,
        defaultWidth: 760,
        defaultHeight: 560,
    },
    [AppId.AI_ASSISTANT]: {
        id: AppId.AI_ASSISTANT,
        title: 'Blue AI',
        icon: Bot,
        component: GeminiAssistantApp,
        defaultWidth: 460,
        defaultHeight: 660,
    },
    [AppId.SETTINGS]: {
        id: AppId.SETTINGS,
        title: 'Settings',
        icon: Settings,
        component: SettingsApp,
        defaultWidth: 860,
        defaultHeight: 620,
    },
    [AppId.ABOUT]: {
        id: AppId.ABOUT,
        title: 'About Blue',
        icon: Info,
        component: AboutApp,
        defaultWidth: 420,
        defaultHeight: 360,
    },
    [AppId.EXTERNAL]: {
        id: AppId.EXTERNAL,
        title: 'External App',
        icon: Box,
        isExternal: true,
    },
    // Blue Edit, Blue Screen, Blue Images, Blue Videos, Blue Music
    // are intentionally REMOVED from here — they launch as external processes
    [AppId.BLUE_EDIT]: {
        id: AppId.BLUE_EDIT,
        title: 'Blue Edit',
        icon: Box,
        isExternal: true,
        externalPath: 'blue-edit',
    },
    [AppId.BLUE_IMAGES]: {
        id: AppId.BLUE_IMAGES,
        title: 'Blue Images',
        icon: Box,
        isExternal: true,
        externalPath: 'blue-images',
    },
    [AppId.BLUE_VIDEOS]: {
        id: AppId.BLUE_VIDEOS,
        title: 'Blue Videos',
        icon: Box,
        isExternal: true,
        externalPath: 'blue-videos',
    },
    [AppId.BLUE_MUSIC]: {
        id: AppId.BLUE_MUSIC,
        title: 'Blue Music',
        icon: Box,
        isExternal: true,
        externalPath: 'blue-music',
    },
    [AppId.BLUE_SCREEN]: {
        id: AppId.BLUE_SCREEN,
        title: 'Blue Screen',
        icon: Monitor,
        isExternal: true,
        externalPath: 'blue-screen',
    },
};

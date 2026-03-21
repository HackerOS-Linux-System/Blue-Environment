import { Terminal, Bot, FolderOpen, Settings, Info, Box, Globe, Calculator, Activity, Monitor, FileText, FileCode, Package, Mail } from 'lucide-react';
import { AppDefinition, AppId } from './types';

import TerminalApp from './components/apps/TerminalApp';
import BlueAI from './components/apps/BlueAI';
import ExplorerApp from './components/apps/ExplorerApp';
import SettingsApp from './components/apps/SettingsApp';
import AboutApp from './components/apps/AboutApp';
import BlueWebApp from './components/apps/BlueWebApp';
import CalculatorApp from './components/apps/CalculatorApp';
import SystemMonitorApp from './components/apps/SystemMonitorApp';
import NotepadApp from './components/apps/NotepadApp';
import BlueCodeApp from './components/apps/BlueCodeApp';
import BlueSoftwareApp from './components/apps/BlueSoftwareApp';
import MailApp from './components/apps/MailApp';

export const WALLPAPER_URL = "file:///usr/share/wallpapers/default.png";

export const THEMES = {
    'blue-default': { name: 'Blue Glass',  bg: 'bg-slate-900',    accent: 'blue'   },
    'cyberpunk':    { name: 'Cyberpunk',   bg: 'bg-zinc-950',     accent: 'yellow' },
    'dracula':      { name: 'Dracula',     bg: 'bg-[#282a36]',    accent: 'purple' },
    'light-glass':  { name: 'Light Glass', bg: 'bg-slate-200',    accent: 'blue' },
};

export const APPS: Record<AppId, AppDefinition> = {
    [AppId.TERMINAL]: {
        id: AppId.TERMINAL,
        title: 'Terminal',
        icon: Terminal,
        component: TerminalApp,
        defaultWidth: 680,
            defaultHeight: 480,
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
    [AppId.AI_ASSISTANT]: {
        id: AppId.AI_ASSISTANT,
        title: 'Blue AI',
        icon: Bot,
        component: BlueAI,
        defaultWidth: 500,
            defaultHeight: 700,
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
    [AppId.NOTEPAD]: {
        id: AppId.NOTEPAD,
        title: 'Notepad',
        icon: FileText,
        component: NotepadApp,
        defaultWidth: 600,
            defaultHeight: 400,
    },
    [AppId.BLUE_CODE]: {
        id: AppId.BLUE_CODE,
        title: 'Blue Code',
        icon: FileCode,
        component: BlueCodeApp,
        defaultWidth: 900,
            defaultHeight: 700,
    },
    [AppId.BLUE_SOFTWARE]: {
        id: AppId.BLUE_SOFTWARE,
        title: 'Blue Software',
        icon: Package,
        component: BlueSoftwareApp,
        defaultWidth: 800,
            defaultHeight: 600,
    },
    [AppId.MAIL]: {
        id: AppId.MAIL,
        title: 'Mail',
        icon: Mail,
        component: MailApp,
        defaultWidth: 1000,
            defaultHeight: 700,
    },
    [AppId.EXTERNAL]: {
        id: AppId.EXTERNAL,
        title: 'External App',
        icon: Box,
        isExternal: true,
    },
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

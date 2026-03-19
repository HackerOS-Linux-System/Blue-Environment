import { ReactNode } from 'react';
import { LucideIcon } from 'lucide-react'; // jeśli używamy

export enum AppId {
    TERMINAL       = 'terminal',
    AI_ASSISTANT   = 'ai_assistant',
    EXPLORER       = 'explorer',
    SETTINGS       = 'settings',
    ABOUT          = 'about',
    BLUE_WEB       = 'blue_web',
    // Internal apps
    CALCULATOR     = 'calculator',
    SYSTEM_MONITOR = 'system_monitor',
    // External standalone apps (in ~/.hackeros/Blue-Environment/apps/)
    BLUE_EDIT      = 'blue_edit',
    BLUE_IMAGES    = 'blue_images',
    BLUE_VIDEOS    = 'blue_videos',
    BLUE_MUSIC     = 'blue_music',
    BLUE_SCREEN    = 'blue_screen',
    EXTERNAL       = 'external',
    NOTEPAD        = 'notepad',
}

export interface DesktopEntry {
    id: string;
    name: string;
    comment: string;
    icon: string;
    exec: string;
    categories: string[];
}

export interface AppDefinition {
    id: AppId | string;
    title: string;
    icon: React.ComponentType<any> | string;
    component?: React.ComponentType<any>;
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
    zIndex: number;
    isExternal: boolean;
    workspace: number;
}

export interface UserConfig {
    wallpaper: string;
    theme: 'dark' | 'light';
    themeName: string;
    accentColor: string;
    displayScale: number;
    customThemes?: ThemeDefinition[];
}

export interface ThemeDefinition {
    id: string;
    name: string;
    type: 'builtin' | 'custom';
    css?: string;
    colors?: {
        primary: string;
        secondary: string;
        text: string;
        accent: string;
    };
}

export interface PowerProfile {
    name: string;                // 'power-saver', 'balanced', 'performance'
    active: boolean;
    icon?: string;               // nazwa ikony (np. 'Battery', 'Wind', 'Zap')
    description: string;
}

export interface AppProps {
    windowId: string;
}

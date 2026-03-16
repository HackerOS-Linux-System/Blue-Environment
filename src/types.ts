import { ReactNode } from 'react';

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
    BLUE_CONNECT   = 'blue_connect',
    // External standalone apps (in ~/.hackeros/Blue-Environment/apps/)
    BLUE_EDIT      = 'blue_edit',
    BLUE_IMAGES    = 'blue_images',
    BLUE_VIDEOS    = 'blue_videos',
    BLUE_MUSIC     = 'blue_music',
    BLUE_SCREEN    = 'blue_screen',
    EXTERNAL       = 'external',
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
    externalPath?: string;      // name under ~/.hackeros/Blue-Environment/apps/
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
    themeName: 'blue-default' | 'cyberpunk' | 'dracula' | 'light-glass';
    accentColor: string;
    displayScale: number;
}

export interface AppProps {
    windowId: string;
}

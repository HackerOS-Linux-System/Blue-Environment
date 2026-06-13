import type { LucideIcon } from 'lucide-react';

export interface TabButtonProps {
    icon: LucideIcon;
    label: string;
    isActive: boolean;
    onClick: () => void;
}

export type SettingsTab =
    | 'display' | 'personalization' | 'wifi' | 'bluetooth'
    | 'power' | 'panel' | 'language' | 'nightLight'
    | 'apps' | 'monitors' | 'printers' | 'users'
    | 'accounts' | 'about';

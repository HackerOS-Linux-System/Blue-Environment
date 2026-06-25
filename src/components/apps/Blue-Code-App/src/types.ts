export interface FileNode {
    name: string;
    path: string;
    type: 'file' | 'directory';
    children?: FileNode[];
    expanded?: boolean;
}

export interface OpenFile {
    path: string;
    content: string;
    modified: boolean;
    language: string;
}

export interface SearchResult { file: string; line: number; content: string; }

export interface Diagnostic {
    file: string;
    line: number;
    col: number;
    message: string;
    severity: 'error' | 'warning' | 'info';
}

export type SidebarTab = 'files' | 'search' | 'git' | 'dev';

export interface CommandEntry {
    id: string;
    label: string;
    shortcut?: string;
    action: () => void;
}

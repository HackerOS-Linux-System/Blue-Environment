export interface FileEntry {
    name: string; path: string; is_dir: boolean;
    size: string; mime_type: string; modified?: string;
}
export interface Tab { id: string; path: string; history: string[]; historyIndex: number; }
export interface Notif { id: string; type: 'success'|'error'|'info'; message: string; }
export type SortKey = 'name'|'size'|'modified'|'type';
export type ViewMode = 'grid'|'list'|'columns';

export const BOOKMARKS = [
    { name: 'Home',      path: 'HOME',           iconName: 'Home' },
    { name: 'Desktop',   path: 'HOME/Desktop',   iconName: 'Folder' },
    { name: 'Documents', path: 'HOME/Documents', iconName: 'FileText' },
    { name: 'Downloads', path: 'HOME/Downloads', iconName: 'Download' },
    { name: 'Pictures',  path: 'HOME/Pictures',  iconName: 'Image' },
    { name: 'Music',     path: 'HOME/Music',     iconName: 'Music' },
    { name: 'Videos',    path: 'HOME/Videos',    iconName: 'Video' },
] as const;

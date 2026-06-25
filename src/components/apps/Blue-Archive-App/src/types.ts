export interface ArchiveEntry { name: string; path: string; size: number; is_dir: boolean; }
export interface ArchiveInfo  { entries: ArchiveEntry[]; total_files: number; error?: string; }
export interface ArchiveState { path: string; name: string; info: ArchiveInfo; }

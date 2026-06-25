import { useState, useCallback, useEffect } from 'react';
import { SystemBridge } from '../../../../utils/systemBridge';
import { useDialog } from '../../../../contexts/DialogContext';
import { FileNode } from './types';

const LAST_WORKSPACE_KEY = 'blue_code_last_workspace';

export function useFileTree() {
    const dialog = useDialog();
    const [rootPath, setRootPath]   = useState('');
    const [fileTree, setFileTree]   = useState<FileNode[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    // Tracks the last directory the user interacted with (clicked a folder,
    // or the parent of a clicked file) so "New File" / "New Folder" create
    // inside the right place instead of always dropping things at the
    // workspace root.
    const [selectedDir, setSelectedDir] = useState('');

    const buildNodes = (files: { name: string; path: string; is_dir: boolean }[]): FileNode[] =>
        files
            .map(f => ({
                name: f.name, path: f.path,
                type: (f.is_dir ? 'directory' : 'file') as FileNode['type'],
                expanded: false,
                children: f.is_dir ? [] : undefined,
            }))
            .sort((a, b) => {
                if (a.type !== b.type) return a.type === 'directory' ? -1 : 1;
                return a.name.localeCompare(b.name);
            });

    const loadTree = useCallback(async (path: string) => {
        setIsLoading(true);
        try {
            const entries = await SystemBridge.getFiles(path);
            setFileTree(buildNodes(entries));
        } catch { /* directory may not exist yet — leave tree empty */ }
        setIsLoading(false);
    }, []);

    // Restore the last opened workspace, or fall back to the home directory.
    useEffect(() => {
        const last = localStorage.getItem(LAST_WORKSPACE_KEY);
        if (last) {
            setRootPath(last);
            setSelectedDir(last);
            loadTree(last);
        } else {
            SystemBridge.getDefaultDesktopPath().then(p => {
                const home = p.replace('/Desktop', '');
                setRootPath(home);
                setSelectedDir(home);
                loadTree(home);
            });
        }
    }, [loadTree]);

    /** Opens a native folder picker (Tauri dialog) and switches the workspace to it. */
    const openWorkspace = useCallback(async () => {
        const dir = await SystemBridge.pickDirectory();
        if (!dir) return;
        setRootPath(dir);
        setSelectedDir(dir);
        localStorage.setItem(LAST_WORKSPACE_KEY, dir);
        await loadTree(dir);
    }, [loadTree]);

    const loadChildren = useCallback(async (node: FileNode) => {
        if (node.type !== 'directory' || (node.children && node.children.length > 0)) return;
        const files = await SystemBridge.getFiles(node.path);
        node.children = buildNodes(files);
        setFileTree(t => [...t]);
    }, []);

    const toggleDir = useCallback(async (node: FileNode) => {
        if (node.type !== 'directory') return;
        setSelectedDir(node.path);
        node.expanded = !node.expanded;
        if (node.expanded && (!node.children || node.children.length === 0)) await loadChildren(node);
        else setFileTree(t => [...t]);
    }, [loadChildren]);

    /** Creates a new file inside `parentDir` (defaults to the currently selected directory, not always the workspace root). */
    const createFile = useCallback(async (parentDir?: string): Promise<string | null> => {
        const dir = parentDir || selectedDir || rootPath;
        const name = await dialog.prompt({
            title: 'New File',
            label: `In ${dir}`,
            placeholder: 'untitled.ts',
            defaultValue: 'untitled.ts',
            confirmLabel: 'Create',
        });
        if (!name) return null;
        const path = `${dir}/${name}`;
        await SystemBridge.writeFile(path, '');
        await loadTree(rootPath);
        return path;
    }, [dialog, selectedDir, rootPath, loadTree]);

    const createFolder = useCallback(async (parentDir?: string) => {
        const dir = parentDir || selectedDir || rootPath;
        const name = await dialog.prompt({
            title: 'New Folder',
            label: `In ${dir}`,
            placeholder: 'new-folder',
            defaultValue: 'New Folder',
            confirmLabel: 'Create',
        });
        if (!name) return;
        await SystemBridge.createFolder(dir, name);
        await loadTree(rootPath);
    }, [dialog, selectedDir, rootPath, loadTree]);

    const renameNode = useCallback(async (node: FileNode): Promise<string | null> => {
        const name = await dialog.prompt({
            title: node.type === 'directory' ? 'Rename Folder' : 'Rename File',
            defaultValue: node.name,
            confirmLabel: 'Rename',
        });
        if (!name || name === node.name) return null;
        const parentDir = node.path.slice(0, node.path.length - node.name.length - 1);
        const newPath = `${parentDir}/${name}`;
        await SystemBridge.moveFile(node.path, newPath);
        await loadTree(rootPath);
        return newPath;
    }, [dialog, rootPath, loadTree]);

    const deleteNode = useCallback(async (node: FileNode): Promise<boolean> => {
        const ok = await dialog.confirm({
            title: node.type === 'directory' ? 'Delete folder' : 'Delete file',
            message: `Delete "${node.name}"? This cannot be undone.`,
            confirmLabel: 'Delete',
            danger: true,
        });
        if (!ok) return false;
        await SystemBridge.deleteFile(node.path);
        await loadTree(rootPath);
        return true;
    }, [dialog, rootPath, loadTree]);

    return {
        rootPath, fileTree, isLoading, selectedDir, setSelectedDir,
        loadTree, toggleDir, openWorkspace,
        createFile, createFolder, renameNode, deleteNode,
    };
}

export type FileTreeState = ReturnType<typeof useFileTree>;

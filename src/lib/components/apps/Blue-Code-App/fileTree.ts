import { writable, get } from 'svelte/store';
import { SystemBridge } from '../../../utils/systemBridge';
import { dialogPrompt, dialogConfirm } from '../../../stores/dialog';
import type { FileNode } from './types';

const LAST_WORKSPACE_KEY = 'blue_code_last_workspace';

export function createFileTree() {
  const rootPath = writable('');
  const fileTree = writable<FileNode[]>([]);
  const isLoading = writable(false);
  const selectedDir = writable('');

  function buildNodes(files: { name: string; path: string; is_dir: boolean }[]): FileNode[] {
    return files
      .map((f) => ({ name: f.name, path: f.path, type: (f.is_dir ? 'directory' : 'file') as FileNode['type'], expanded: false, children: f.is_dir ? [] : undefined }))
      .sort((a, b) => (a.type !== b.type ? (a.type === 'directory' ? -1 : 1) : a.name.localeCompare(b.name)));
  }

  async function loadTree(path: string) {
    isLoading.set(true);
    try {
      const entries = await SystemBridge.getFiles(path);
      fileTree.set(buildNodes(entries));
    } catch { /* directory may not exist yet — leave tree empty */ }
    isLoading.set(false);
  }

  function init() {
    const last = localStorage.getItem(LAST_WORKSPACE_KEY);
    if (last) {
      rootPath.set(last); selectedDir.set(last); loadTree(last);
    } else {
      SystemBridge.getDefaultDesktopPath().then((p) => {
        const home = p.replace('/Desktop', '');
        rootPath.set(home); selectedDir.set(home); loadTree(home);
      });
    }
  }

  async function openWorkspace() {
    const dir = await SystemBridge.pickDirectory('blue-code.open-folder');
    if (!dir) return;
    rootPath.set(dir); selectedDir.set(dir);
    localStorage.setItem(LAST_WORKSPACE_KEY, dir);
    await loadTree(dir);
  }

  async function loadChildren(node: FileNode) {
    if (node.type !== 'directory' || (node.children && node.children.length > 0)) return;
    const files = await SystemBridge.getFiles(node.path);
    node.children = buildNodes(files);
    fileTree.update((t) => [...t]);
  }

  async function toggleDir(node: FileNode) {
    if (node.type !== 'directory') return;
    selectedDir.set(node.path);
    node.expanded = !node.expanded;
    if (node.expanded && (!node.children || node.children.length === 0)) await loadChildren(node);
    else fileTree.update((t) => [...t]);
  }

  async function createFile(parentDir?: string): Promise<string | null> {
    const dir = parentDir || get(selectedDir) || get(rootPath);
    const name = await dialogPrompt({ title: 'New File', label: `In ${dir}`, placeholder: 'untitled.ts', defaultValue: 'untitled.ts', confirmLabel: 'Create' });
    if (!name) return null;
    const path = `${dir}/${name}`;
    await SystemBridge.writeFile(path, '');
    await loadTree(get(rootPath));
    return path;
  }

  async function createFolder(parentDir?: string) {
    const dir = parentDir || get(selectedDir) || get(rootPath);
    const name = await dialogPrompt({ title: 'New Folder', label: `In ${dir}`, placeholder: 'new-folder', defaultValue: 'New Folder', confirmLabel: 'Create' });
    if (!name) return;
    await SystemBridge.createFolder(dir, name);
    await loadTree(get(rootPath));
  }

  async function renameNode(node: FileNode): Promise<string | null> {
    const name = await dialogPrompt({ title: node.type === 'directory' ? 'Rename Folder' : 'Rename File', defaultValue: node.name, confirmLabel: 'Rename' });
    if (!name || name === node.name) return null;
    const parentDir = node.path.slice(0, node.path.length - node.name.length - 1);
    const newPath = `${parentDir}/${name}`;
    await SystemBridge.moveFile(node.path, newPath);
    await loadTree(get(rootPath));
    return newPath;
  }

  async function deleteNode(node: FileNode): Promise<boolean> {
    const ok = await dialogConfirm({ title: node.type === 'directory' ? 'Delete folder' : 'Delete file', message: `Delete "${node.name}"? This cannot be undone.`, confirmLabel: 'Delete', danger: true });
    if (!ok) return false;
    await SystemBridge.deleteFile(node.path);
    await loadTree(get(rootPath));
    return true;
  }

  init();

  return { rootPath, fileTree, isLoading, selectedDir, loadTree, toggleDir, openWorkspace, createFile, createFolder, renameNode, deleteNode };
}

export type FileTreeState = ReturnType<typeof createFileTree>;

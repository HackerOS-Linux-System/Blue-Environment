import React from 'react';
import { Folder, ChevronRight, ChevronDown, FileCode, Trash2, Edit2 } from 'lucide-react';
import { FileNode } from './types';

interface Props {
    nodes: FileNode[];
    level?: number;
    onOpenFile: (path: string) => void;
    onToggleDir: (node: FileNode) => void;
    onRename: (node: FileNode) => void;
    onDelete: (node: FileNode) => void;
    selectedDir: string;
}

const FileTreeView: React.FC<Props> = ({ nodes, level = 0, onOpenFile, onToggleDir, onRename, onDelete, selectedDir }) => (
    <>
        {nodes.map(node => (
            <div key={node.path}>
                <div
                    className={`flex items-center gap-1 py-0.5 px-1 rounded cursor-pointer hover:bg-white/5 group text-sm ${
                        node.type === 'directory' && node.path === selectedDir ? 'bg-blue-600/10' : ''
                    }`}
                    style={{ paddingLeft: `${level * 12 + 4}px` }}
                    onDoubleClick={() => node.type === 'file' && onOpenFile(node.path)}
                    onClick={() => node.type === 'directory' && onToggleDir(node)}
                >
                    {node.type === 'directory' && (
                        <span className="text-slate-500 w-4 shrink-0">
                            {node.expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                        </span>
                    )}
                    {node.type === 'directory'
                        ? <Folder size={14} className="text-blue-400 shrink-0" />
                        : <FileCode size={14} className="text-yellow-400 shrink-0" />}
                    <span className="truncate flex-1">{node.name}</span>
                    <div className="flex gap-0.5 opacity-0 group-hover:opacity-100 ml-auto shrink-0">
                        {node.type === 'file' && (
                            <button onClick={e => { e.stopPropagation(); onOpenFile(node.path); }}
                                className="p-0.5 hover:bg-white/10 rounded text-slate-500" title="Open">
                                <FileCode size={11} />
                            </button>
                        )}
                        <button onClick={e => { e.stopPropagation(); onRename(node); }}
                            className="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-blue-400" title="Rename">
                            <Edit2 size={11} />
                        </button>
                        <button onClick={e => { e.stopPropagation(); onDelete(node); }}
                            className="p-0.5 hover:bg-white/10 rounded text-slate-500 hover:text-red-400" title="Delete">
                            <Trash2 size={11} />
                        </button>
                    </div>
                </div>
                {node.type === 'directory' && node.expanded && node.children && (
                    <FileTreeView
                        nodes={node.children} level={level + 1}
                        onOpenFile={onOpenFile} onToggleDir={onToggleDir}
                        onRename={onRename} onDelete={onDelete} selectedDir={selectedDir}
                    />
                )}
            </div>
        ))}
    </>
);

export default FileTreeView;

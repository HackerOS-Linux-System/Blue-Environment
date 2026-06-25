import { useState, useRef, useCallback, useEffect } from 'react';
import type { OnMount, Monaco } from '@monaco-editor/react';
import { SystemBridge } from '../../../../utils/systemBridge';
import { OpenFile, Diagnostic } from './types';
import { getLang, LSP_LANGS } from './languageMap';

export function useEditorFiles(rootPath: string) {
    const [openFiles, setOpenFiles] = useState<OpenFile[]>([]);
    const [activeIdx, setActiveIdx] = useState(0);
    const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
    const [lspStatus, setLspStatus] = useState<Record<string, boolean>>({});
    const [editorTheme, setEditorTheme] = useState('blue-dark');
    const [fontSize, setFontSize] = useState(13);
    const [cursorPos, setCursorPos] = useState({ line: 1, col: 1 });
    const editorRef = useRef<any>(null);
    const monacoRef = useRef<Monaco | null>(null);

    const startLsp = useCallback(async (language: string, root: string) => {
        if (lspStatus[language]) return;
        if (!LSP_LANGS.includes(language)) return;
        const res = await SystemBridge.startLanguageServer(language, root);
        if (res.success) setLspStatus(prev => ({ ...prev, [language]: true }));
    }, [lspStatus]);

    const openFile = useCallback(async (path: string) => {
        setOpenFiles(prev => {
            const existing = prev.findIndex(f => f.path === path);
            if (existing >= 0) { setActiveIdx(existing); return prev; }
            return prev;
        });
        const existingIdx = openFiles.findIndex(f => f.path === path);
        if (existingIdx >= 0) { setActiveIdx(existingIdx); return; }

        const content = await SystemBridge.readFile(path);
        const language = getLang(path);
        setOpenFiles(prev => {
            const next = [...prev, { path, content, modified: false, language }];
            setActiveIdx(next.length - 1);
            return next;
        });
        startLsp(language, rootPath);
    }, [openFiles, rootPath, startLsp]);

    const closeFile = useCallback((idx: number) => {
        setOpenFiles(prev => {
            if (prev.length <= 1) return prev.length === 1 ? [] : prev;
            return prev.filter((_, i) => i !== idx);
        });
        setActiveIdx(i => Math.max(0, i === idx ? i - 1 : i > idx ? i - 1 : i));
    }, []);

    const runLint = useCallback(async (idx: number) => {
        const file = openFiles[idx];
        if (!file) return;
        const newDiags: Diagnostic[] = [];

        if (file.language === 'rust') {
            const res = await SystemBridge.executeCommand(`cd "${rootPath}" && cargo check --message-format=short 2>&1 | head -20`);
            const out = res.stdout || res.stderr || '';
            for (const line of out.split('\n')) {
                const m = line.match(/^(.+):(\d+):(\d+):\s*(error|warning):\s*(.+)$/);
                if (m) newDiags.push({ file: m[1], line: parseInt(m[2]), col: parseInt(m[3]), severity: m[4] as any, message: m[5] });
            }
        } else if (file.language === 'typescript' || file.language === 'javascript') {
            const res = await SystemBridge.executeCommand(`npx --yes tsc --noEmit --allowJs "${file.path}" 2>&1 | head -20`);
            const out = res.stdout || res.stderr || '';
            for (const line of out.split('\n')) {
                const m = line.match(/^(.+)\((\d+),(\d+)\):\s*(error|warning)\s+\w+:\s*(.+)$/);
                if (m) newDiags.push({ file: m[1], line: parseInt(m[2]), col: parseInt(m[3]), severity: m[4] as any, message: m[5] });
            }
        } else if (file.language === 'python') {
            const res = await SystemBridge.executeCommand(`cd "${rootPath}" && python3 -m py_compile "${file.path}" 2>&1 | head -20`);
            const out = res.stdout || res.stderr || '';
            for (const line of out.split('\n')) {
                const m = line.match(/File "(.+)", line (\d+)/);
                if (m) newDiags.push({ file: m[1], line: parseInt(m[2]), col: 1, severity: 'error', message: out.split('\n').pop() || 'Syntax error' });
            }
        }

        setDiagnostics(prev => [...prev.filter(d => d.file !== file.path), ...newDiags]);

        if (monacoRef.current && editorRef.current) {
            const monaco = monacoRef.current;
            const model = editorRef.current.getModel();
            if (model) {
                const markers = newDiags
                    .filter(d => d.file === file.path || file.path.endsWith(d.file))
                    .map(d => ({
                        severity: d.severity === 'error' ? monaco.MarkerSeverity.Error : monaco.MarkerSeverity.Warning,
                        startLineNumber: d.line, startColumn: d.col,
                        endLineNumber: d.line, endColumn: d.col + 30,
                        message: d.message,
                    }));
                monaco.editor.setModelMarkers(model, 'blue-lint', markers);
            }
        }
    }, [openFiles, rootPath]);

    const saveFile = useCallback(async (idx: number) => {
        const file = openFiles[idx];
        if (!file) return;
        await SystemBridge.writeFile(file.path, file.content);
        setOpenFiles(prev => prev.map((f, i) => i === idx ? { ...f, modified: false } : f));
        runLint(idx);
    }, [openFiles, runLint]);

    const saveAll = useCallback(() => {
        openFiles.forEach((f, i) => f.modified && saveFile(i));
    }, [openFiles, saveFile]);

    /** Updates a file's path in-place after an external rename (keeps tab/content). */
    const renameOpenFile = useCallback((oldPath: string, newPath: string) => {
        setOpenFiles(prev => prev.map(f => f.path === oldPath ? { ...f, path: newPath, language: getLang(newPath) } : f));
    }, []);

    const handleEditorChange = useCallback((value?: string) => {
        if (value === undefined) return;
        setOpenFiles(prev => prev.map((f, i) => i === activeIdx ? { ...f, content: value, modified: true } : f));
    }, [activeIdx]);

    const handleEditorMount: OnMount = useCallback((editor, monaco) => {
        editorRef.current = editor;
        monacoRef.current = monaco;

        editor.onDidChangeCursorPosition((e: any) => {
            setCursorPos({ line: e.position.lineNumber, col: e.position.column });
        });

        monaco.editor.defineTheme('blue-dark', {
            base: 'vs-dark', inherit: true, rules: [],
            colors: {
                'editor.background': '#0f172a',
                'editor.lineHighlightBackground': '#1e293b50',
                'editorLineNumber.foreground': '#475569',
                'editorLineNumber.activeForeground': '#94a3b8',
                'editorGutter.background': '#0f172a',
                'editor.selectionBackground': '#2563eb40',
            },
        });
        monaco.editor.defineTheme('blue-light', {
            base: 'vs', inherit: true, rules: [],
            colors: { 'editor.background': '#f8fafc' },
        });
        monaco.editor.setTheme(editorTheme);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    useEffect(() => { monacoRef.current?.editor.setTheme(editorTheme); }, [editorTheme]);

    const activeFile = openFiles[activeIdx];
    const fileDiagnostics = diagnostics.filter(d =>
        activeFile && (d.file === activeFile.path || activeFile.path.endsWith(d.file)));
    const errors   = fileDiagnostics.filter(d => d.severity === 'error').length;
    const warnings = fileDiagnostics.filter(d => d.severity === 'warning').length;

    return {
        openFiles, activeIdx, setActiveIdx, activeFile,
        diagnostics, fileDiagnostics, errors, warnings,
        lspStatus, editorTheme, setEditorTheme, fontSize, setFontSize, cursorPos,
        editorRef, monacoRef,
        openFile, closeFile, saveFile, saveAll, runLint, renameOpenFile,
        handleEditorChange, handleEditorMount,
    };
}

export type EditorFilesState = ReturnType<typeof useEditorFiles>;

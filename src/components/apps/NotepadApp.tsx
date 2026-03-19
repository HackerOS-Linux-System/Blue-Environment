import React, { useState, useEffect } from 'react';
import { Save, FileText, Trash2, Download, Upload } from 'lucide-react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';

const NotepadApp: React.FC<AppProps> = () => {
    const [content, setContent] = useState('');
    const [filename, setFilename] = useState('nowy.txt');
    const [saved, setSaved] = useState(true);
    const [filePath, setFilePath] = useState<string | null>(null);

    useEffect(() => {
        // Automatyczne zapisywanie co 30 sekund, jeśli jest zmiana
        const interval = setInterval(() => {
            if (!saved && content) {
                handleSave();
            }
        }, 30000);
        return () => clearInterval(interval);
    }, [content, saved]);

    const handleSave = async () => {
        if (!content) return;
        try {
            if (!filePath) {
                // W rzeczywistej implementacji otwórz okno dialogowe zapisu
                const path = `/home/user/Dokumenty/${filename}`;
                await SystemBridge.writeFile(path, content);
                setFilePath(path);
            } else {
                await SystemBridge.writeFile(filePath, content);
            }
            setSaved(true);
        } catch (e) {
            console.error('Błąd zapisu:', e);
        }
    };

    const handleOpen = async () => {
        // W rzeczywistej implementacji otwórz okno wyboru pliku
        const path = '/home/user/Dokumenty/notatka.txt';
        try {
            const text = await SystemBridge.readFile(path);
            setContent(text);
            setFilePath(path);
            setFilename(path.split('/').pop() || 'notatka.txt');
            setSaved(true);
        } catch (e) {
            console.error('Błąd odczytu:', e);
        }
    };

    const handleNew = () => {
        if (!saved && content) {
            if (!confirm('Masz niezapisane zmiany. Czy na pewno chcesz utworzyć nowy plik?')) return;
        }
        setContent('');
        setFilename('nowy.txt');
        setFilePath(null);
        setSaved(true);
    };

    const handleContentChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
        setContent(e.target.value);
        setSaved(false);
    };

    return (
        <div className="flex flex-col h-full bg-slate-900 text-white">
        {/* Pasek narzędzi */}
        <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-4 gap-2">
        <button
        onClick={handleNew}
        className="p-2 hover:bg-white/10 rounded-lg transition-colors"
        title="Nowy"
        >
        <FileText size={18} />
        </button>
        <button
        onClick={handleOpen}
        className="p-2 hover:bg-white/10 rounded-lg transition-colors"
        title="Otwórz"
        >
        <Upload size={18} />
        </button>
        <button
        onClick={handleSave}
        className="p-2 hover:bg-white/10 rounded-lg transition-colors"
        title="Zapisz"
        disabled={saved}
        >
        <Save size={18} className={saved ? 'text-slate-500' : 'text-blue-400'} />
        </button>
        <div className="w-px h-6 bg-white/10 mx-2" />
        <input
        type="text"
        value={filename}
        onChange={(e) => setFilename(e.target.value)}
        className="bg-slate-900 border border-white/10 rounded px-3 py-1 text-sm text-white focus:outline-none focus:border-blue-500"
        />
        <div className="flex-1" />
        <span className="text-xs text-slate-500">
        {saved ? 'Zapisano' : 'Niezapisano'}
        </span>
        </div>

        {/* Edytor */}
        <textarea
        value={content}
        onChange={handleContentChange}
        className="flex-1 w-full p-4 bg-slate-900 text-white font-mono text-sm resize-none focus:outline-none"
        placeholder="Wpisz tutaj swoją notatkę..."
        />
        </div>
    );
};

export default NotepadApp;

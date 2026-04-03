import React, { useState, useRef, useEffect, KeyboardEvent } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';

interface TerminalLine {
    type: 'input' | 'output' | 'error';
    content: string;
}

const TerminalApp: React.FC<AppProps> = () => {
    const [lines, setLines] = useState<TerminalLine[]>([
        { type: 'output', content: 'Blue Terminal v1.0' },
        { type: 'output', content: 'Wpisz "help" aby zobaczyć dostępne komendy.' },
    ]);
    const [currentInput, setCurrentInput] = useState('');
    const [history, setHistory] = useState<string[]>([]);
    const [historyIndex, setHistoryIndex] = useState(-1);
    const [cwd, setCwd] = useState('~');
    const inputRef = useRef<HTMLInputElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    // Przewijanie na dół przy nowych liniach
    useEffect(() => {
        if (containerRef.current) {
            containerRef.current.scrollTop = containerRef.current.scrollHeight;
        }
    }, [lines]);

    // Fokus na input przy renderowaniu
    useEffect(() => {
        inputRef.current?.focus();
    }, []);

    const handleCommand = async (cmd: string) => {
        // Dodaj komendę do historii
        setLines(prev => [...prev, { type: 'input', content: `${cwd} $ ${cmd}` }]);

        // Obsługa pustej komendy
        if (cmd.trim() === '') {
            setLines(prev => [...prev, { type: 'output', content: '' }]);
            return;
        }

        // Parsuj komendę i argumenty
        const parts = cmd.trim().split(' ');
        const command = parts[0].toLowerCase();
        const args = parts.slice(1);

        // Wbudowane komendy
        let output = '';
        let isError = false;

        try {
            switch (command) {
                case 'clear':
                case 'cls':
                    setLines([]);
                    return;

                case 'help':
                    output = `Dostępne komendy:
                    help      – wyświetla tę pomoc
                    clear     – czyści ekran
                    echo      – wyświetla tekst
                    pwd       – wyświetla bieżący katalog
                    ls        – lista plików w katalogu
                    cd        – zmiana katalogu
                    date      – wyświetla datę i czas
                    whoami    – wyświetla nazwę użytkownika
                    exit      – zamyka terminal (w tym oknie nie działa)
                    history   – wyświetla historię komend`;
                    break;

                case 'echo':
                    output = args.join(' ');
                    break;

                case 'pwd':
                    output = cwd;
                    break;

                case 'date':
                    output = new Date().toString();
                    break;

                case 'whoami':
                    output = SystemBridge.getUsername();
                    break;

                case 'history':
                    output = history.map((h, i) => `${i + 1}  ${h}`).join('\n');
                    break;

                case 'ls':
                    // Symulacja ls – w rzeczywistości mogłaby korzystać z SystemBridge.listFiles
                    output = 'Desktop  Documents  Downloads  Pictures  Videos  .config';
                    break;

                case 'cd':
                    if (args.length === 0 || args[0] === '~') {
                        setCwd('~');
                    } else {
                        setCwd(cwd + '/' + args[0]); // bardzo uproszczone
                    }
                    output = '';
                    break;

                default:
                    // Spróbuj wykonać przez system (jeśli dostępne)
                    if (await SystemBridge.isTauri()) {
                        try {
                            // Wykonaj komendę w shellu przez Tauri
                            const result = await SystemBridge.executeCommand(cmd);
                            output = result.stdout || result.stderr;
                            isError = !!result.stderr;
                        } catch (e: any) {
                            output = `Błąd: ${e.message}`;
                            isError = true;
                        }
                    } else {
                        output = `Nieznana komenda: ${command}`;
                        isError = true;
                    }
            }
        } catch (e: any) {
            output = `Błąd: ${e.message}`;
            isError = true;
        }

        if (output) {
            setLines(prev => [...prev, { type: isError ? 'error' : 'output', content: output }]);
        }
    };

    const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            const cmd = currentInput.trim();
            if (cmd) {
                setHistory(prev => [...prev, cmd]);
                setHistoryIndex(-1);
                handleCommand(cmd);
                setCurrentInput('');
            } else {
                setLines(prev => [...prev, { type: 'input', content: `${cwd} $ ` }]);
            }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (history.length > 0) {
                const newIndex = historyIndex === -1 ? history.length - 1 : Math.max(0, historyIndex - 1);
                setHistoryIndex(newIndex);
                setCurrentInput(history[newIndex]);
            }
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            if (historyIndex !== -1) {
                const newIndex = Math.min(history.length - 1, historyIndex + 1);
                setHistoryIndex(newIndex);
                setCurrentInput(history[newIndex]);
            } else {
                setCurrentInput('');
            }
        } else if (e.key === 'Tab') {
            e.preventDefault();
            // Proste autouzupełnianie (opcjonalnie)
        }
    };

    return (
        <div className="flex flex-col h-full bg-black text-green-400 font-mono text-sm p-2 overflow-hidden">
        <div
        ref={containerRef}
        className="flex-1 overflow-y-auto whitespace-pre-wrap break-words"
        onClick={() => inputRef.current?.focus()}
        >
        {lines.map((line, idx) => (
            <div key={idx} className={line.type === 'error' ? 'text-red-400' : ''}>
            {line.content}
            </div>
        ))}
        <div className="flex items-center">
        <span className="mr-1">{cwd} $</span>
        <input
        ref={inputRef}
        type="text"
        className="flex-1 bg-transparent border-none outline-none text-green-400"
        value={currentInput}
        onChange={(e) => setCurrentInput(e.target.value)}
        onKeyDown={handleKeyDown}
        />
        </div>
        </div>
        </div>
    );
};

export default TerminalApp;

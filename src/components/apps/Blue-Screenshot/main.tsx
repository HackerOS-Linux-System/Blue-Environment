import React, { useState, useRef, useCallback, useEffect } from 'react';
import {
    Camera, Download, Copy, Trash2, Timer, Monitor,
    Crop, RefreshCw, Check, X, Maximize2, Square
} from 'lucide-react';
import { AppProps } from '../../../types';
import { SystemBridge } from '../../../utils/systemBridge';

type CaptureMode = 'fullscreen' | 'window' | 'region';
type DelayOption = 0 | 3 | 5 | 10;

interface Screenshot {
    id: string;
    dataUrl: string;
    savedPath: string | null;
    timestamp: Date;
    mode: CaptureMode;
    width: number;
    height: number;
}

const BlueScreenshot: React.FC<AppProps> = () => {
    const [mode, setMode]     = useState<CaptureMode>('fullscreen');
    const [delay, setDelay]   = useState<DelayOption>(0);
    const [screenshots, setScreenshots] = useState<Screenshot[]>([]);
    const [capturing, setCapturing]     = useState(false);
    const [countdown, setCountdown]     = useState(0);
    const [selected, setSelected]       = useState<Screenshot | null>(null);
    const [copied, setCopied]           = useState(false);
    const [savePath, setSavePath]       = useState('~/Pictures/Screenshots');
    const canvasRef = useRef<HTMLCanvasElement>(null);

    const capture = useCallback(async () => {
        setCapturing(true);

        if (delay > 0) {
            setCountdown(delay);
            for (let i = delay; i > 0; i--) {
                await new Promise(r => setTimeout(r, 1000));
                setCountdown(i - 1);
            }
        }

        try {
            let dataUrl = '';
            let savedPath = '';

            if (SystemBridge.isTauri()) {
                if (mode === 'region') {
                    // Region selection via slurp (Wayland geometry picker)
                    const geomResult = await SystemBridge.executeCommand('slurp 2>/dev/null');
                    const geomRaw = typeof geomResult === 'string' ? geomResult : (geomResult as any)?.stdout ?? '';
                    const geom = (geomRaw as string).trim();
                    if (!geom) throw new Error('Selection cancelled');

                    const ts = new Date().toISOString().replace(/[:.]/g, '-');
                    const outPath = `${(window as any).__TAURI_HOME__ || `${await SystemBridge.getHomePath()}`}/Pictures/Screenshots/screenshot-${ts}.png`;
                    await SystemBridge.executeCommand(`mkdir -p "$(dirname '${outPath}')" && grim -g "${geom}" "${outPath}" 2>/dev/null || import -geometry "${geom}" "${outPath}" 2>/dev/null`);
                    dataUrl = await SystemBridge.readFileAsDataURL(outPath);
                    savedPath = outPath;
                } else if (mode === 'window') {
                    const ts = new Date().toISOString().replace(/[:.]/g, '-');
                    const outPath = `${(window as any).__TAURI_HOME__ || `${await SystemBridge.getHomePath()}`}/Pictures/Screenshots/screenshot-${ts}.png`;
                    await SystemBridge.executeCommand(`mkdir -p "$(dirname '${outPath}')" && grim -g "$(swaymsg -t get_tree 2>/dev/null | python3 -c "import json,sys; t=json.load(sys.stdin); [print(f\\"{n['rect']['x']},{n['rect']['y']} {n['rect']['width']}x{n['rect']['height']}\\") for n in [t] if n.get('focused')]" 2>/dev/null || slurp)" "${outPath}" 2>/dev/null || import -window root "${outPath}" 2>/dev/null`);
                    dataUrl = await SystemBridge.readFileAsDataURL(outPath);
                    savedPath = outPath;
                } else {
                    // Fullscreen - use the backend take_screenshot command which
                    // auto-detects grim/scrot/spectacle and saves to ~/Pictures/Screenshots
                    savedPath = await SystemBridge.takeScreenshot() || '';
                    if (!savedPath) throw new Error('No screenshot tool found (grim, scrot, or spectacle required)');
                    dataUrl = await SystemBridge.readFileAsDataURL(savedPath);
                }
            } else {
                // Browser fallback using getDisplayMedia
                const stream = await (navigator.mediaDevices as any).getDisplayMedia({ video: true });
                const video  = document.createElement('video');
                video.srcObject = stream;
                await new Promise<void>(r => { video.onloadedmetadata = () => { video.play(); r(); }; });
                const canvas = canvasRef.current!;
                canvas.width  = video.videoWidth;
                canvas.height = video.videoHeight;
                canvas.getContext('2d')!.drawImage(video, 0, 0);
                dataUrl = canvas.toDataURL('image/png');
                stream.getTracks().forEach((t: MediaStreamTrack) => t.stop());
            }

            if (!dataUrl) throw new Error('Empty screenshot data');

            const img = new Image();
            img.src = dataUrl;
            await new Promise(r => { img.onload = r; });

            const ss: Screenshot = {
                id:        Date.now().toString(),
                dataUrl,
                savedPath: savedPath || null,
                timestamp: new Date(),
                mode,
                width:  img.naturalWidth,
                height: img.naturalHeight,
            };

            setScreenshots(prev => [ss, ...prev]);
            setSelected(ss);
        } catch (e) {
            console.error('Screenshot error:', e);
        } finally {
            setCapturing(false);
            setCountdown(0);
        }
    }, [mode, delay]);

    const saveToFile = async (ss: Screenshot) => {
        const name = `screenshot-${ss.timestamp.toISOString().slice(0, 19).replace(/:/g, '-')}.png`;
        if (SystemBridge.isTauri()) {
            await SystemBridge.saveFile(`${savePath.replace('~', '')}/${name}`, ss.dataUrl);
        } else {
            const a = document.createElement('a');
            a.href = ss.dataUrl;
            a.download = name;
            a.click();
        }
    };

    const copyToClipboard = async (ss: Screenshot) => {
        await SystemBridge.writeClipboardImage(ss.dataUrl);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    const deleteScreenshot = (id: string) => {
        setScreenshots(prev => prev.filter(s => s.id !== id));
        if (selected?.id === id) setSelected(null);
    };

    const MODES: { id: CaptureMode; label: string; icon: React.ReactNode }[] = [
        { id: 'fullscreen', label: 'Full Screen', icon: <Monitor size={16} /> },
        { id: 'window',     label: 'Window',      icon: <Square size={16} /> },
        { id: 'region',     label: 'Region',      icon: <Crop size={16} /> },
    ];

    const DELAYS: DelayOption[] = [0, 3, 5, 10];

    return (
        <div className="flex h-full bg-slate-900 text-white overflow-hidden">
            <canvas ref={canvasRef} className="hidden" />

            {/* Sidebar */}
            <div className="w-64 bg-slate-800/50 border-r border-white/5 flex flex-col">
                {/* Mode */}
                <div className="p-4 border-b border-white/5">
                    <div className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3">Capture Mode</div>
                    <div className="space-y-1">
                        {MODES.map(m => (
                            <button
                                key={m.id}
                                onClick={() => setMode(m.id)}
                                className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${mode === m.id ? 'bg-blue-600 text-white' : 'text-slate-300 hover:bg-white/5'}`}
                            >
                                {m.icon} {m.label}
                            </button>
                        ))}
                    </div>
                </div>

                {/* Delay */}
                <div className="p-4 border-b border-white/5">
                    <div className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-3 flex items-center gap-1.5">
                        <Timer size={12} /> Delay
                    </div>
                    <div className="flex gap-2">
                        {DELAYS.map(d => (
                            <button
                                key={d}
                                onClick={() => setDelay(d)}
                                className={`flex-1 py-1.5 rounded-lg text-xs font-medium transition-colors ${delay === d ? 'bg-blue-600 text-white' : 'bg-slate-700 text-slate-300 hover:bg-slate-600'}`}
                            >
                                {d === 0 ? 'Now' : `${d}s`}
                            </button>
                        ))}
                    </div>
                </div>

                {/* Save path */}
                <div className="p-4 border-b border-white/5">
                    <div className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Save to</div>
                    <div className="flex gap-2">
                        <input
                            type="text"
                            value={savePath}
                            onChange={e => setSavePath(e.target.value)}
                            className="flex-1 bg-slate-700 border border-white/10 rounded-lg px-3 py-1.5 text-xs text-white focus:outline-none min-w-0"
                        />
                        <button
                            onClick={async () => { const p = await SystemBridge.pickDirectory(); if (p) setSavePath(p); }}
                            className="p-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-slate-300"
                        >
                            <RefreshCw size={13} />
                        </button>
                    </div>
                </div>

                {/* Capture button */}
                <div className="p-4">
                    <button
                        onClick={capture}
                        disabled={capturing}
                        className="w-full py-3 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl font-semibold text-sm flex items-center justify-center gap-2 transition-colors"
                    >
                        {capturing ? (
                            countdown > 0 ? <><Timer size={16} /> {countdown}s</> : <><RefreshCw size={16} className="animate-spin" /> Capturing…</>
                        ) : (
                            <><Camera size={16} /> Take Screenshot</>
                        )}
                    </button>
                </div>

                {/* Gallery list */}
                <div className="flex-1 overflow-y-auto p-2">
                    {screenshots.length === 0 ? (
                        <div className="text-center text-slate-600 text-xs py-8">No screenshots yet</div>
                    ) : (
                        <div className="space-y-2">
                            {screenshots.map(ss => (
                                <div
                                    key={ss.id}
                                    onClick={() => setSelected(ss)}
                                    className={`rounded-xl overflow-hidden cursor-pointer border-2 transition-colors ${selected?.id === ss.id ? 'border-blue-500' : 'border-transparent hover:border-white/20'}`}
                                >
                                    <img src={ss.dataUrl} alt="" className="w-full h-24 object-cover" />
                                    <div className="px-2 py-1 bg-slate-800 flex items-center justify-between">
                                        <span className="text-[10px] text-slate-400">
                                            {ss.width}×{ss.height} · {ss.timestamp.toLocaleTimeString()}
                                        </span>
                                        <button
                                            onClick={e => { e.stopPropagation(); deleteScreenshot(ss.id); }}
                                            className="p-0.5 hover:text-red-400 text-slate-500"
                                        >
                                            <Trash2 size={11} />
                                        </button>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>

            {/* Preview */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {selected ? (
                    <>
                        <div className="shrink-0 flex items-center justify-between px-4 py-3 border-b border-white/5 bg-slate-800/30">
                            <div className="text-sm text-slate-300">
                                {selected.width}×{selected.height} · {selected.mode} · {selected.timestamp.toLocaleString()}
                            </div>
                            <div className="flex gap-2">
                                <button
                                    onClick={() => copyToClipboard(selected)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm"
                                >
                                    {copied ? <><Check size={14} className="text-green-400" /> Copied!</> : <><Copy size={14} /> Copy</>}
                                </button>
                                <button
                                    onClick={() => saveToFile(selected)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm"
                                >
                                    <Download size={14} /> Save
                                </button>
                                <button
                                    onClick={() => deleteScreenshot(selected.id)}
                                    className="p-1.5 hover:bg-red-500/20 rounded-lg text-red-400"
                                >
                                    <Trash2 size={16} />
                                </button>
                            </div>
                        </div>
                        <div className="flex-1 overflow-auto flex items-center justify-center p-6 bg-slate-950/50">
                            <img
                                src={selected.dataUrl}
                                alt="Screenshot"
                                className="max-w-full max-h-full object-contain rounded-lg shadow-2xl"
                            />
                        </div>
                    </>
                ) : (
                    <div className="flex-1 flex flex-col items-center justify-center text-slate-600 gap-4">
                        <Camera size={56} className="opacity-20" />
                        <div className="text-center">
                            <div className="text-lg font-medium text-slate-400 mb-1">Blue Screenshot</div>
                            <div className="text-sm">Choose a mode and click "Take Screenshot"</div>
                        </div>
                        <button
                            onClick={capture}
                            disabled={capturing}
                            className="mt-2 px-6 py-2.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl text-sm text-white font-medium flex items-center gap-2"
                        >
                            <Camera size={16} /> Take Screenshot
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
};

export default BlueScreenshot;

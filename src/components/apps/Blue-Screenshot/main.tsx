import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
    Camera, Settings, History, Check, Loader2,
    Copy, Download, ZoomIn, ZoomOut, RotateCcw,
    Monitor, Crop, AppWindow, Timer, X,
} from 'lucide-react';
import type { AppProps } from '../../../types';
import { SystemBridge } from '../../../utils/systemBridge';
import type { CaptureMode, Screenshot, ScreenshotSettings } from './types';
import { DEFAULT_SETTINGS } from './types';
import CaptureButton from './CaptureButton';
import HistoryPanel from './HistoryPanel';
import SettingsPanel from './SettingsPanel';

type PanelView = 'history' | 'settings' | null;

const BlueScreenshot: React.FC<AppProps> = () => {
    const [mode, setMode]           = useState<CaptureMode>('fullscreen');
    const [capturing, setCapturing] = useState(false);
    const [countdown, setCountdown] = useState(0);
    const [panel, setPanel]         = useState<PanelView>('history');
    const [screenshots, setScreenshots] = useState<Screenshot[]>([]);
    const [selected, setSelected]   = useState<string | null>(null);
    const [settings, setSettings]   = useState<ScreenshotSettings>(DEFAULT_SETTINGS);
    const [zoom, setZoom]           = useState(1);
    const [status, setStatus]       = useState<{ type: 'success'|'error'; msg: string } | null>(null);

    const previewRef = useRef<HTMLDivElement>(null);

    // Load saved settings & history from localStorage
    useEffect(() => {
        try {
            const s = localStorage.getItem('blue-screenshot-settings');
            if (s) setSettings({ ...DEFAULT_SETTINGS, ...JSON.parse(s) });
            const h = localStorage.getItem('blue-screenshot-history');
            if (h) setScreenshots(JSON.parse(h));
        } catch {}
    }, []);

    const saveSettings = useCallback((patch: Partial<ScreenshotSettings>) => {
        const next = { ...settings, ...patch };
        setSettings(next);
        localStorage.setItem('blue-screenshot-settings', JSON.stringify(next));
    }, [settings]);

    const selectedShot = screenshots.find(s => s.id === selected);

    // ── Capture logic ─────────────────────────────────────────────────────

    const buildGrimCmd = (m: CaptureMode, outPath: string, s: ScreenshotSettings): string => {
        const cursor = s.showCursor ? '--include-cursor' : '';
        switch (m) {
            case 'fullscreen': return `grim ${cursor} "${outPath}"`;
            case 'region':     return `grim ${cursor} -g "$(slurp)" "${outPath}"`;
            case 'window':     return `grim ${cursor} -g "$(swaymsg -t get_tree | jq -r '.. | select(.focused?) | .rect | "\\(.x),\\(.y) \\(.width)x\\(.height)"' 2>/dev/null || slurp)" "${outPath}"`;
            default:           return `grim ${cursor} "${outPath}"`;
        }
    };

    const buildX11Cmd = (m: CaptureMode, outPath: string, s: ScreenshotSettings): string => {
        switch (m) {
            case 'fullscreen': return `scrot "${outPath}" 2>/dev/null || gnome-screenshot -f "${outPath}"`;
            case 'region':     return `scrot -s "${outPath}" 2>/dev/null || gnome-screenshot -a -f "${outPath}"`;
            case 'window':     return `scrot -u "${outPath}" 2>/dev/null || gnome-screenshot -w -f "${outPath}"`;
            default:           return `scrot "${outPath}"`;
        }
    };

    const doCapture = useCallback(async () => {
        if (capturing) return;
        setCapturing(true);
        setStatus(null);

        try {
            // Delay countdown
            if (settings.delay > 0) {
                for (let i = settings.delay; i > 0; i--) {
                    setCountdown(i);
                    await new Promise(r => setTimeout(r, 1000));
                }
                setCountdown(0);
            }

            // Resolve save path
            const timestamp = Date.now();
            const filename  = `screenshot-${new Date(timestamp).toISOString().replace(/[:.]/g, '-')}.${settings.format}`;
            const saveDir   = settings.savePath.replace('HOME', await SystemBridge.getHomePath());
            const outPath   = `${saveDir}/${filename}`;

            // Ensure save dir exists
            await SystemBridge.executeCommand(`mkdir -p "${saveDir}"`);

            // Detect session type
            const session = await SystemBridge.getSessionType();
            const isWayland = session.startsWith('wayland');
            const cmd = isWayland
                ? buildGrimCmd(mode, outPath, settings)
                : buildX11Cmd(mode, outPath, settings);

            const result = await SystemBridge.executeCommand(cmd);
            if (result.stderr?.includes('error') || result.returnCode !== 0) {
                throw new Error(result.stderr || 'Capture failed');
            }

            // Read back as data URL for preview
            const dataUrl = await SystemBridge.readFileAsDataURL(outPath) ?? '';

            // Get dimensions from dataUrl
            let width = 1920, height = 1080;
            if (dataUrl) {
                await new Promise<void>(resolve => {
                    const img = new window.Image();
                    img.onload = () => { width = img.width; height = img.height; resolve(); };
                    img.onerror = () => resolve();
                    img.src = dataUrl;
                });
            }

            const shot: Screenshot = {
                id: `ss-${timestamp}`, path: outPath, dataUrl,
                width, height, timestamp, mode,
            };

            const next = [...screenshots, shot];
            setScreenshots(next);
            localStorage.setItem('blue-screenshot-history', JSON.stringify(next.slice(-50)));
            setSelected(shot.id);
            setPanel('history');

            // Copy to clipboard
            if (settings.copyToClipboard && dataUrl) {
                await SystemBridge.writeClipboardImage(dataUrl);
            }

            // Sound effect
            if (settings.playSoundEffect) {
                await SystemBridge.executeCommand(
                    `paplay /usr/share/sounds/freedesktop/stereo/camera-shutter.oga 2>/dev/null || true`
                );
            }

            setStatus({ type: 'success', msg: `Saved: ${filename}` });
        } catch (e: any) {
            setStatus({ type: 'error', msg: e.message || 'Capture failed' });
        } finally {
            setCapturing(false);
            setCountdown(0);
        }
    }, [mode, settings, screenshots, capturing]);

    const handleDelete = (id: string) => {
        const next = screenshots.filter(s => s.id !== id);
        setScreenshots(next);
        localStorage.setItem('blue-screenshot-history', JSON.stringify(next.slice(-50)));
        if (selected === id) setSelected(next.length > 0 ? next[next.length - 1].id : null);
    };

    // Keyboard shortcut: Enter to capture
    useEffect(() => {
        const h = (e: KeyboardEvent) => {
            if (e.key === 'Enter' && !capturing) doCapture();
            if (e.key === 'Escape' && capturing) setCapturing(false);
            if (e.key === '+' || e.key === '=') setZoom(z => Math.min(3, z + 0.25));
            if (e.key === '-') setZoom(z => Math.max(0.25, z - 0.25));
            if (e.key === '0') setZoom(1);
        };
        window.addEventListener('keydown', h);
        return () => window.removeEventListener('keydown', h);
    }, [capturing, doCapture]);

    const MODES: { mode: CaptureMode; label: string; desc: string }[] = [
        { mode: 'fullscreen', label: 'Full Screen',  desc: 'Entire display' },
        { mode: 'region',     label: 'Region',       desc: 'Select area' },
        { mode: 'window',     label: 'Window',       desc: 'Active window' },
        { mode: 'timer',      label: 'Timer',        desc: `${settings.delay}s delay` },
    ];

    return (
        <div className="flex h-full bg-slate-900 text-white overflow-hidden">
            {/* Main area */}
            <div className="flex-1 flex flex-col overflow-hidden">
                {/* Toolbar */}
                <div className="h-12 bg-slate-800 border-b border-white/5 flex items-center px-4 gap-2 shrink-0">
                    <Camera size={18} className="text-blue-400" />
                    <span className="font-semibold text-sm mr-auto">Blue Screenshot</span>

                    {status && (
                        <div className={`flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs
                            ${status.type === 'success' ? 'bg-green-600/20 text-green-400' : 'bg-red-600/20 text-red-400'}`}>
                            {status.type === 'success' ? <Check size={12}/> : <X size={12}/>}
                            {status.msg}
                        </div>
                    )}

                    <button onClick={() => setZoom(z => Math.max(0.25, z - 0.25))} className="p-1.5 hover:bg-white/10 rounded" title="Zoom out (-)"><ZoomOut size={15}/></button>
                    <span className="text-xs text-slate-500 w-10 text-center">{Math.round(zoom*100)}%</span>
                    <button onClick={() => setZoom(z => Math.min(3, z + 0.25))} className="p-1.5 hover:bg-white/10 rounded" title="Zoom in (+)"><ZoomIn size={15}/></button>
                    <button onClick={() => setZoom(1)} className="p-1.5 hover:bg-white/10 rounded" title="Reset zoom (0)"><RotateCcw size={15}/></button>

                    <div className="w-px h-5 bg-white/10 mx-1"/>

                    <button onClick={() => setPanel(p => p === 'history' ? null : 'history')}
                        className={`p-1.5 rounded ${panel==='history' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}>
                        <History size={15}/>
                    </button>
                    <button onClick={() => setPanel(p => p === 'settings' ? null : 'settings')}
                        className={`p-1.5 rounded ${panel==='settings' ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10'}`}>
                        <Settings size={15}/>
                    </button>
                </div>

                {/* Capture mode selector */}
                <div className="flex gap-3 p-4 border-b border-white/5 shrink-0">
                    {MODES.map(m => (
                        <CaptureButton key={m.mode}
                            mode={m.mode === 'timer' ? mode : m.mode}
                            label={m.label}
                            description={m.mode === 'timer' ? `${settings.delay}s delay` : m.desc}
                            isActive={mode === m.mode}
                            onClick={() => setMode(m.mode)}
                        />
                    ))}
                </div>

                {/* Preview area */}
                <div ref={previewRef} className="flex-1 overflow-auto flex items-center justify-center bg-[#0a0f1e] p-4">
                    {capturing && countdown > 0 ? (
                        <div className="flex flex-col items-center gap-4">
                            <div className="text-8xl font-light tabular-nums"
                                style={{ fontFamily: '"Oxanium", monospace', color: '#3b82f6',
                                    textShadow: '0 0 60px rgba(59,130,246,0.5)' }}>
                                {countdown}
                            </div>
                            <p className="text-slate-400 text-sm">Capturing in…</p>
                        </div>
                    ) : capturing ? (
                        <div className="flex flex-col items-center gap-4">
                            <Loader2 size={40} className="animate-spin text-blue-400"/>
                            <p className="text-slate-400 text-sm">Capturing…</p>
                        </div>
                    ) : selectedShot?.dataUrl ? (
                        <div style={{ transform: `scale(${zoom})`, transformOrigin: 'center', transition: 'transform 0.15s ease' }}>
                            <img
                                src={selectedShot.dataUrl}
                                alt="screenshot preview"
                                className="max-w-full rounded-lg shadow-2xl"
                                style={{ imageRendering: zoom > 1.5 ? 'pixelated' : 'auto' }}
                            />
                        </div>
                    ) : (
                        <div className="flex flex-col items-center gap-4 text-slate-700">
                            <div className="w-24 h-24 bg-slate-800 rounded-3xl flex items-center justify-center">
                                <Camera size={40} className="text-slate-600"/>
                            </div>
                            <p className="text-sm">Press <kbd className="px-2 py-0.5 bg-slate-800 rounded text-slate-400 text-xs">Enter</kbd> or click Capture</p>
                        </div>
                    )}
                </div>

                {/* Bottom bar */}
                <div className="h-14 bg-slate-800 border-t border-white/5 flex items-center justify-between px-4 shrink-0">
                    <div className="flex items-center gap-2 text-xs text-slate-500">
                        {selectedShot && (
                            <>
                                <span>{selectedShot.width}×{selectedShot.height}</span>
                                <span>·</span>
                                <span>{selectedShot.mode}</span>
                                <span>·</span>
                                <span>{new Date(selectedShot.timestamp).toLocaleString()}</span>
                            </>
                        )}
                    </div>

                    <div className="flex items-center gap-2">
                        {selectedShot && (
                            <>
                                <button
                                    onClick={() => SystemBridge.writeClipboardImage(selectedShot.dataUrl)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs transition-colors">
                                    <Copy size={13}/> Copy
                                </button>
                                <button
                                    onClick={() => SystemBridge.saveFile(selectedShot.path, selectedShot.dataUrl)}
                                    className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-xs transition-colors">
                                    <Download size={13}/> Save As
                                </button>
                            </>
                        )}
                        <button
                            onClick={doCapture}
                            disabled={capturing}
                            className="flex items-center gap-2 px-5 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl text-sm font-medium transition-all shadow-lg shadow-blue-500/20 disabled:cursor-not-allowed">
                            {capturing
                                ? <Loader2 size={16} className="animate-spin"/>
                                : <Camera size={16}/>
                            }
                            {capturing ? 'Capturing…' : 'Capture'}
                        </button>
                    </div>
                </div>
            </div>

            {/* Side panel */}
            {panel && (
                <div className="w-64 border-l border-white/5 bg-slate-800/50 flex flex-col overflow-hidden shrink-0">
                    <div className="h-10 flex items-center justify-between px-3 border-b border-white/5">
                        <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider">
                            {panel === 'history' ? 'History' : 'Settings'}
                        </span>
                        <button onClick={() => setPanel(null)} className="p-1 hover:bg-white/10 rounded text-slate-500 hover:text-white">
                            <X size={12}/>
                        </button>
                    </div>
                    <div className="flex-1 overflow-hidden">
                        {panel === 'history'
                            ? <HistoryPanel
                                screenshots={screenshots}
                                selected={selected}
                                onSelect={setSelected}
                                onDelete={handleDelete}
                              />
                            : <SettingsPanel settings={settings} onChange={saveSettings} />
                        }
                    </div>
                </div>
            )}
        </div>
    );
};

export default BlueScreenshot;

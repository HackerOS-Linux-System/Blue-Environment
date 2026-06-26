import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import { SystemBridge } from '../../utils/systemBridge';
import {
    Camera, Video, Image, FlipHorizontal, Settings,
    Circle, Square, RefreshCw, AlertTriangle,
    SwitchCamera, Grid3x3, X, Info
} from 'lucide-react';

type Mode = 'photo' | 'video';
type TimerSecs = 0 | 3 | 5 | 10;

interface NativeDevice { path: string; name: string; }

const CameraApp: React.FC<AppProps> = () => {
    const videoRef = useRef<HTMLVideoElement>(null);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const mediaRecorderRef = useRef<MediaRecorder | null>(null);
    const chunksRef = useRef<Blob[]>([]);
    const streamRef = useRef<MediaStream | null>(null);
    const timerRef = useRef<ReturnType<typeof setInterval>>();
    const pollRef = useRef<ReturnType<typeof setInterval>>();

    const [mode, setMode] = useState<Mode>('photo');
    const [recording, setRecording] = useState(false);
    const [recordTime, setRecordTime] = useState(0);
    const [captures, setCaptures] = useState<{ url: string; type: 'photo' | 'video'; name: string }[]>([]);
    const [mirrored, setMirrored] = useState(false);
    const [showGrid, setShowGrid] = useState(false);
    const [timer, setTimer] = useState<TimerSecs>(0);
    const [countdown, setCountdown] = useState(0);
    const [error, setError] = useState<string | null>(null);
    const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
    const [deviceIdx, setDeviceIdx] = useState(0);
    const [resolution, setResolution] = useState({ w: 1280, h: 720 });
    const [showSettings, setShowSettings] = useState(false);
    const [flashAnim, setFlashAnim] = useState(false);

    // ── Native fallback state ───────────────────────────────────────────
    // navigator.mediaDevices.getUserMedia() does not work inside Tauri's
    // Linux webview (WebKitGTK ships with media-stream support disabled —
    // see CameraApp/mod.rs for the full explanation). When it throws, we
    // fall back to grabbing frames straight from /dev/video* via ffmpeg.
    const [nativeMode, setNativeMode] = useState(false);
    const [nativeDevices, setNativeDevices] = useState<NativeDevice[]>([]);
    const [nativeFrame, setNativeFrame] = useState<string | null>(null);
    const [ffmpegMissing, setFfmpegMissing] = useState(false);
    const [busy, setBusy] = useState(false);

    const stopNativePolling = () => { clearInterval(pollRef.current); };

    const startNativeCamera = useCallback(async (devIdx = deviceIdx) => {
        setError(null);
        setNativeMode(true);
        stopNativePolling();

        const available = await SystemBridge.cameraCheckAvailable();
        if (!available) {
            setFfmpegMissing(true);
            return;
        }
        setFfmpegMissing(false);

        const list = await SystemBridge.cameraListDevices();
        setNativeDevices(list);
        if (list.length === 0) {
            setError('No camera device found under /dev/video*.');
            return;
        }
        const device = list[devIdx % list.length].path;

        const poll = async () => {
            try {
                const frame = await SystemBridge.cameraCaptureFrame(device, resolution.w, resolution.h);
                setNativeFrame(frame);
            } catch {
                // A transient grab failure (device busy on this tick) — keep
                // showing the last good frame rather than flashing an error.
            }
        };
        poll();
        pollRef.current = setInterval(poll, 600);
    }, [deviceIdx, resolution]);

    const startCamera = useCallback(async (devIdx = deviceIdx) => {
        streamRef.current?.getTracks().forEach(t => t.stop());
        setError(null);
        try {
            const allDevices = await navigator.mediaDevices.enumerateDevices();
            const cams = allDevices.filter(d => d.kind === 'videoinput');
            setDevices(cams);
            const constraints: MediaStreamConstraints = {
                video: {
                    deviceId: cams[devIdx]?.deviceId ? { ideal: cams[devIdx].deviceId } : undefined,
                    width: { ideal: resolution.w },
                    height: { ideal: resolution.h },
                },
                audio: mode === 'video',
            };
            const stream = await navigator.mediaDevices.getUserMedia(constraints);
            streamRef.current = stream;
            setNativeMode(false);
            stopNativePolling();
            if (videoRef.current) {
                videoRef.current.srcObject = stream;
                await videoRef.current.play();
            }
        } catch {
            // Browser camera API unavailable (the normal case in this
            // webview on Linux) — fall back to native ffmpeg/v4l2 capture.
            await startNativeCamera(devIdx);
        }
    }, [deviceIdx, resolution, mode, startNativeCamera]);

    useEffect(() => {
        startCamera();
        return () => {
            streamRef.current?.getTracks().forEach(t => t.stop());
            clearInterval(timerRef.current);
            stopNativePolling();
        };
    }, []);

    const doCapture = useCallback(async () => {
        setFlashAnim(true);
        setTimeout(() => setFlashAnim(false), 300);

        if (nativeMode) {
            const device = nativeDevices[deviceIdx % Math.max(nativeDevices.length, 1)]?.path;
            if (!device) return;
            setBusy(true);
            try {
                const path = await SystemBridge.cameraCapturePhoto(device, resolution.w, resolution.h);
                const name = path.split('/').pop() || 'photo.jpg';
                setCaptures(prev => [{ url: `file://${path}`, type: 'photo', name }, ...prev]);
            } catch (e: any) {
                setError(`Capture failed: ${e?.message || e}`);
            } finally {
                setBusy(false);
            }
            return;
        }

        const canvas = canvasRef.current;
        const video = videoRef.current;
        if (!canvas || !video) return;
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        const ctx = canvas.getContext('2d')!;
        if (mirrored) {
            ctx.translate(canvas.width, 0);
            ctx.scale(-1, 1);
        }
        ctx.drawImage(video, 0, 0);

        canvas.toBlob(blob => {
            if (!blob) return;
            const url = URL.createObjectURL(blob);
            const name = `photo-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.jpg`;
            setCaptures(prev => [{ url, type: 'photo', name }, ...prev]);
            const a = document.createElement('a');
            a.href = url; a.download = name; a.click();
        }, 'image/jpeg', 0.92);
    }, [mirrored, nativeMode, nativeDevices, deviceIdx, resolution]);

    const takePhoto = () => {
        if (timer === 0) { doCapture(); return; }
        let count = timer;
        setCountdown(count);
        const id = setInterval(() => {
            count--;
            setCountdown(count);
            if (count <= 0) { clearInterval(id); setCountdown(0); doCapture(); }
        }, 1000);
    };

    const startNativeRecording = async () => {
        const device = nativeDevices[deviceIdx % Math.max(nativeDevices.length, 1)]?.path;
        if (!device) return;
        const duration = timer || 5;
        setRecording(true);
        setRecordTime(0);
        timerRef.current = setInterval(() => setRecordTime(t => t + 1), 1000);
        try {
            const path = await SystemBridge.cameraRecordVideo(device, resolution.w, resolution.h, duration);
            const name = path.split('/').pop() || 'video.mp4';
            setCaptures(prev => [{ url: `file://${path}`, type: 'video', name }, ...prev]);
        } catch (e: any) {
            setError(`Recording failed: ${e?.message || e}`);
        } finally {
            clearInterval(timerRef.current);
            setRecording(false);
            setRecordTime(0);
        }
    };

    const startRecording = async () => {
        if (nativeMode) { startNativeRecording(); return; }
        if (!streamRef.current) return;
        if (!streamRef.current.getAudioTracks().length) {
            await startCamera(deviceIdx);
            await new Promise(r => setTimeout(r, 500));
        }
        const stream = streamRef.current;
        if (!stream) return;
        const mime = MediaRecorder.isTypeSupported('video/webm;codecs=vp9') ? 'video/webm;codecs=vp9' : 'video/webm';
        const mr = new MediaRecorder(stream, { mimeType: mime });
        chunksRef.current = [];
        mr.ondataavailable = e => { if (e.data.size > 0) chunksRef.current.push(e.data); };
        mr.onstop = () => {
            const blob = new Blob(chunksRef.current, { type: mime });
            const url = URL.createObjectURL(blob);
            const name = `video-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.webm`;
            setCaptures(prev => [{ url, type: 'video', name }, ...prev]);
            const a = document.createElement('a');
            a.href = url; a.download = name; a.click();
        };
        mr.start(250);
        mediaRecorderRef.current = mr;
        setRecording(true);
        setRecordTime(0);
        timerRef.current = setInterval(() => setRecordTime(t => t + 1), 1000);
    };

    const stopRecording = () => {
        mediaRecorderRef.current?.stop();
        clearInterval(timerRef.current);
        setRecording(false);
        setRecordTime(0);
    };

    const switchCamera = () => {
        const count = nativeMode ? nativeDevices.length : devices.length;
        const next = (deviceIdx + 1) % Math.max(count, 1);
        setDeviceIdx(next);
        if (nativeMode) startNativeCamera(next); else startCamera(next);
    };

    const fmtTime = (s: number) => `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
    const deviceCount = nativeMode ? nativeDevices.length : devices.length;

    return (
        <div className="flex flex-col h-full bg-black text-white overflow-hidden">
            {/* Viewfinder */}
            <div className="flex-1 relative overflow-hidden bg-black">
                {ffmpegMissing ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center text-center px-8">
                        <AlertTriangle size={40} className="text-amber-500 mb-4" />
                        <p className="text-slate-200 text-sm mb-1 font-medium">ffmpeg not found</p>
                        <p className="text-slate-400 text-xs max-w-xs">
                            The browser camera API isn't available in this webview, so Blue Camera
                            captures frames via ffmpeg + V4L2 instead. Install ffmpeg to use the camera
                            (e.g. <code className="bg-slate-800 px-1 rounded">sudo dnf install ffmpeg</code>).
                        </p>
                        <button onClick={() => startNativeCamera()} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm mt-4">
                            Retry
                        </button>
                    </div>
                ) : error ? (
                    <div className="absolute inset-0 flex flex-col items-center justify-center text-center px-8">
                        <Camera size={48} className="text-slate-600 mb-4" />
                        <p className="text-slate-300 text-sm mb-2">{error}</p>
                        <button onClick={() => startCamera()} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm mt-2">
                            Retry
                        </button>
                    </div>
                ) : (
                    <>
                        {nativeMode ? (
                            nativeFrame ? (
                                <img
                                    src={nativeFrame}
                                    className="w-full h-full object-contain"
                                    style={{ transform: mirrored ? 'scaleX(-1)' : 'none' }}
                                    alt="Camera preview"
                                />
                            ) : (
                                <div className="absolute inset-0 flex items-center justify-center">
                                    <RefreshCw size={28} className="animate-spin text-slate-500" />
                                </div>
                            )
                        ) : (
                            <video
                                ref={videoRef}
                                className="w-full h-full object-contain"
                                style={{ transform: mirrored ? 'scaleX(-1)' : 'none' }}
                                playsInline muted autoPlay
                            />
                        )}

                        {nativeMode && (
                            <div className="absolute bottom-3 left-3 flex items-center gap-1.5 bg-black/60 backdrop-blur-sm rounded-full px-3 py-1 text-[11px] text-slate-300">
                                <Info size={11} /> Native capture mode (ffmpeg / V4L2)
                            </div>
                        )}

                        {/* Grid overlay */}
                        {showGrid && (
                            <div className="absolute inset-0 pointer-events-none">
                                {[1, 2].map(i => (
                                    <div key={`v${i}`} className="absolute top-0 bottom-0 border-l border-white/20" style={{ left: `${i * 33.33}%` }} />
                                ))}
                                {[1, 2].map(i => (
                                    <div key={`h${i}`} className="absolute left-0 right-0 border-t border-white/20" style={{ top: `${i * 33.33}%` }} />
                                ))}
                            </div>
                        )}

                        {flashAnim && <div className="absolute inset-0 bg-white animate-ping pointer-events-none" style={{ animation: 'flash 0.3s ease-out' }} />}

                        {countdown > 0 && (
                            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
                                <span className="text-8xl font-bold text-white drop-shadow-2xl" style={{ textShadow: '0 0 30px rgba(0,0,0,0.8)' }}>
                                    {countdown}
                                </span>
                            </div>
                        )}

                        {recording && (
                            <div className="absolute top-4 left-4 flex items-center gap-2 bg-black/60 backdrop-blur-sm rounded-full px-3 py-1.5">
                                <div className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                                <span className="text-sm font-mono text-white">
                                    {fmtTime(recordTime)}{nativeMode && ` / ${fmtTime(timer || 5)}`}
                                </span>
                            </div>
                        )}

                        <div className="absolute top-3 right-3 flex gap-2">
                            <button onClick={() => setMirrored(m => !m)} className={`p-2 rounded-full backdrop-blur-sm ${mirrored ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}`}>
                                <FlipHorizontal size={16} />
                            </button>
                            <button onClick={() => setShowGrid(g => !g)} className={`p-2 rounded-full backdrop-blur-sm ${showGrid ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}`}>
                                <Grid3x3 size={16} />
                            </button>
                            {deviceCount > 1 && (
                                <button onClick={switchCamera} className="p-2 rounded-full bg-black/40 hover:bg-black/60 backdrop-blur-sm">
                                    <SwitchCamera size={16} />
                                </button>
                            )}
                            <button onClick={() => setShowSettings(s => !s)} className={`p-2 rounded-full backdrop-blur-sm ${showSettings ? 'bg-blue-600/80' : 'bg-black/40 hover:bg-black/60'}`}>
                                <Settings size={16} />
                            </button>
                        </div>

                        {showSettings && (
                            <div className="absolute top-14 right-3 bg-slate-900/95 backdrop-blur-sm rounded-xl border border-white/10 p-4 w-56 space-y-3">
                                <div>
                                    <label className="text-xs text-slate-400 block mb-1">Camera</label>
                                    <select
                                        value={deviceIdx}
                                        onChange={e => { const v = +e.target.value; setDeviceIdx(v); nativeMode ? startNativeCamera(v) : startCamera(v); }}
                                        className="w-full bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white focus:outline-none"
                                    >
                                        {nativeMode
                                            ? nativeDevices.map((d, i) => <option key={d.path} value={i}>{d.name}</option>)
                                            : devices.map((d, i) => <option key={d.deviceId} value={i}>{d.label || `Camera ${i + 1}`}</option>)}
                                        {deviceCount === 0 && <option value={0}>Default Camera</option>}
                                    </select>
                                </div>
                                <div>
                                    <label className="text-xs text-slate-400 block mb-1">Resolution</label>
                                    <select value={`${resolution.w}x${resolution.h}`} onChange={e => {
                                        const [w, h] = e.target.value.split('x').map(Number);
                                        setResolution({ w, h });
                                        nativeMode ? startNativeCamera() : startCamera();
                                    }} className="w-full bg-slate-800 border border-white/10 rounded-lg px-2 py-1.5 text-sm text-white focus:outline-none">
                                        <option value="1920x1080">FHD (1920x1080)</option>
                                        <option value="1280x720">HD (1280x720)</option>
                                        <option value="640x480">SD (640x480)</option>
                                    </select>
                                </div>
                                <button onClick={() => setShowSettings(false)} className="w-full text-center text-xs text-slate-500 hover:text-white pt-1">Close</button>
                            </div>
                        )}
                    </>
                )}
            </div>

            {/* Controls bar */}
            <div className="shrink-0 bg-slate-900 border-t border-white/5">
                <div className="flex items-center justify-between px-4 pt-3 pb-1">
                    <div className="flex bg-slate-800 rounded-xl p-0.5">
                        <button onClick={() => setMode('photo')} className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${mode === 'photo' ? 'bg-white text-black' : 'text-slate-400'}`}>
                            <Camera size={13} /> Photo
                        </button>
                        <button onClick={() => setMode('video')} className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${mode === 'video' ? 'bg-white text-black' : 'text-slate-400'}`}>
                            <Video size={13} /> Video
                        </button>
                    </div>
                    {(mode === 'photo' || nativeMode) && (
                        <div className="flex items-center gap-1">
                            {(nativeMode && mode === 'video' ? [3, 5, 10] as TimerSecs[] : [0, 3, 5, 10] as TimerSecs[]).map(t => (
                                <button key={t} onClick={() => setTimer(t)} className={`w-8 h-8 rounded-lg text-xs font-medium transition-colors ${timer === t ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'}`}>
                                    {t === 0 ? <X size={12} className="mx-auto" /> : `${t}s`}
                                </button>
                            ))}
                        </div>
                    )}
                </div>

                <div className="flex items-center justify-center gap-8 px-4 py-4 relative">
                    <div className="w-14 h-14 rounded-xl overflow-hidden bg-slate-800 border border-white/10 shrink-0">
                        {captures.length > 0 ? (
                            captures[0].type === 'photo'
                                ? <img src={captures[0].url} className="w-full h-full object-cover" alt="" />
                                : <video src={captures[0].url} className="w-full h-full object-cover" />
                        ) : (
                            <div className="w-full h-full flex items-center justify-center"><Image size={20} className="text-slate-600" /></div>
                        )}
                    </div>

                    <div className="relative">
                        {mode === 'photo' ? (
                            <button onClick={takePhoto} disabled={countdown > 0 || busy}
                                className="w-16 h-16 rounded-full bg-white hover:bg-slate-200 active:scale-95 transition-all disabled:opacity-50 flex items-center justify-center shadow-lg shadow-white/20">
                                <div className="w-12 h-12 rounded-full border-2 border-slate-300" />
                            </button>
                        ) : (
                            <button onClick={recording ? stopRecording : startRecording} disabled={nativeMode && recording}
                                className={`w-16 h-16 rounded-full flex items-center justify-center transition-all active:scale-95 shadow-lg disabled:opacity-60 ${recording ? 'bg-red-500 hover:bg-red-400 shadow-red-500/30' : 'bg-red-600 hover:bg-red-500 shadow-red-600/30'}`}>
                                {recording ? <Square size={20} fill="white" /> : <Circle size={20} fill="white" />}
                            </button>
                        )}
                    </div>

                    <div className="w-14 h-14 flex items-center justify-center text-slate-500 text-xs text-center shrink-0">
                        {captures.length > 0 && <span className="font-medium text-slate-300">{captures.length}<br /><span className="text-slate-600 font-normal">saved</span></span>}
                    </div>
                </div>
            </div>

            <canvas ref={canvasRef} className="hidden" />

            <style>{`
                @keyframes flash { 0% { opacity: 0.8; } 100% { opacity: 0; } }
            `}</style>
        </div>
    );
};
export default CameraApp;

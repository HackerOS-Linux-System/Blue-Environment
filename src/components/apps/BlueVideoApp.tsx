import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import {
    Play, Pause, SkipBack, SkipForward, Volume2, VolumeX,
    Maximize2, Minimize2, FolderOpen, ListVideo, Shuffle,
    Repeat, Repeat1, Settings, Film, Plus, X
} from 'lucide-react';

interface VideoItem { name: string; url: string; }
type RepeatMode = 'none' | 'all' | 'one';

const BlueVideoApp: React.FC<AppProps> = () => {
    const videoRef = useRef<HTMLVideoElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const hideTimerRef = useRef<ReturnType<typeof setTimeout>>();
    const fileInputRef = useRef<HTMLInputElement>(null);

    const [playlist, setPlaylist] = useState<VideoItem[]>([]);
    const [currentIdx, setCurrentIdx] = useState(0);
    const [playing, setPlaying] = useState(false);
    const [currentTime, setCurrentTime] = useState(0);
    const [duration, setDuration] = useState(0);
    const [volume, setVolume] = useState(1);
    const [muted, setMuted] = useState(false);
    const [fullscreen, setFullscreen] = useState(false);
    const [showControls, setShowControls] = useState(true);
    const [showPlaylist, setShowPlaylist] = useState(false);
    const [shuffle, setShuffle] = useState(false);
    const [repeat, setRepeat] = useState<RepeatMode>('none');
    const [speed, setSpeed] = useState(1);
    const [showSettings, setShowSettings] = useState(false);
    const [buffered, setBuffered] = useState(0);

    const current = playlist[currentIdx] ?? null;
    const fmtTime = (s: number) => { if (!isFinite(s)) return '0:00'; const h=Math.floor(s/3600),m=Math.floor((s%3600)/60),sec=Math.floor(s%60); return h>0?`${h}:${m.toString().padStart(2,'0')}:${sec.toString().padStart(2,'0')}`:`${m}:${sec.toString().padStart(2,'0')}`; };

    const resetHideTimer = useCallback(() => {
        setShowControls(true); clearTimeout(hideTimerRef.current);
        if (playing) hideTimerRef.current = setTimeout(() => setShowControls(false), 3000);
    }, [playing]);

    useEffect(() => { return () => clearTimeout(hideTimerRef.current); }, []);

    useEffect(() => {
        const v = videoRef.current; if (!v) return;
        const handlers = {
            timeupdate: () => { setCurrentTime(v.currentTime); if (v.buffered.length>0) setBuffered(v.buffered.end(v.buffered.length-1)); },
            durationchange: () => setDuration(v.duration),
            play: () => setPlaying(true),
            pause: () => setPlaying(false),
            ended: () => {
                if (repeat==='one'){v.play();return;}
                if (shuffle){setCurrentIdx(Math.floor(Math.random()*playlist.length));}
                else if (currentIdx<playlist.length-1){setCurrentIdx(i=>i+1);}
                else if (repeat==='all'){setCurrentIdx(0);}
                else{setPlaying(false);}
            },
        };
        Object.entries(handlers).forEach(([e,h])=>v.addEventListener(e,h));
        return ()=>Object.entries(handlers).forEach(([e,h])=>v.removeEventListener(e,h));
    }, [currentIdx, playlist.length, repeat, shuffle]);

    const addFiles = (e: React.ChangeEvent<HTMLInputElement>) => {
        const files = Array.from(e.target.files||[]);
        const items: VideoItem[] = files.map(f=>({name:f.name,url:URL.createObjectURL(f)}));
        setPlaylist(prev=>{const n=[...prev,...items];return n;});
        if (playlist.length===0&&items.length>0) setTimeout(()=>videoRef.current?.play(),100);
        e.target.value='';
    };

    const togglePlay=()=>{ const v=videoRef.current; if(!v) return; playing?v.pause():v.play(); };
    const seek=(e: React.ChangeEvent<HTMLInputElement>)=>{ const v=videoRef.current; if(v) v.currentTime=parseFloat(e.target.value); };
    const changeVolume=(e: React.ChangeEvent<HTMLInputElement>)=>{ const v=videoRef.current; if(v){v.volume=parseFloat(e.target.value);v.muted=false;} };
    const toggleMute=()=>{ const v=videoRef.current; if(v) v.muted=!v.muted; };
    const changeSpeed=(s:number)=>{ const v=videoRef.current; if(v) v.playbackRate=s; setSpeed(s); setShowSettings(false); };
    const toggleFullscreen=async()=>{ if(!document.fullscreenElement){await containerRef.current?.requestFullscreen();setFullscreen(true);}else{await document.exitFullscreen();setFullscreen(false);} };

    const pct = duration>0?(currentTime/duration)*100:0;
    const bufPct = duration>0?(buffered/duration)*100:0;
    const RepeatIcon = repeat==='one'?Repeat1:Repeat;

    return (
        <div ref={containerRef} className="flex flex-col h-full bg-black text-white overflow-hidden" onMouseMove={resetHideTimer} onClick={()=>!showPlaylist&&togglePlay()}>
            <input ref={fileInputRef} type="file" multiple accept="video/*" className="hidden" onChange={addFiles}/>
            <div className="flex-1 relative overflow-hidden flex items-center justify-center bg-black">
                {current ? (
                    <video ref={videoRef} src={current.url} className="max-w-full max-h-full" onClick={e=>e.stopPropagation()} onDoubleClick={toggleFullscreen} style={{cursor:showControls?'default':'none'}}/>
                ) : (
                    <div className="flex flex-col items-center gap-4 text-slate-500 cursor-default" onClick={e=>e.stopPropagation()}>
                        <Film size={64} className="opacity-30"/><p className="text-lg">No video loaded</p>
                        <button onClick={()=>fileInputRef.current?.click()} className="px-5 py-2.5 bg-blue-600 hover:bg-blue-500 rounded-xl text-white text-sm">Open video files</button>
                    </div>
                )}
                {showPlaylist&&(
                    <div className="absolute right-0 top-0 bottom-0 w-64 bg-slate-900/97 border-l border-white/10 flex flex-col z-10" onClick={e=>e.stopPropagation()}>
                        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
                            <span className="font-semibold text-sm">Playlist ({playlist.length})</span>
                            <div className="flex gap-1">
                                <button onClick={()=>fileInputRef.current?.click()} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><Plus size={14}/></button>
                                <button onClick={()=>setShowPlaylist(false)} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><X size={14}/></button>
                            </div>
                        </div>
                        <div className="flex-1 overflow-y-auto">
                            {playlist.map((item,i)=>(
                                <div key={i} onClick={()=>{setCurrentIdx(i);setTimeout(()=>videoRef.current?.play(),50);}}
                                    className={`flex items-center gap-3 px-4 py-2.5 cursor-pointer hover:bg-white/10 group ${i===currentIdx?'bg-blue-600/20 border-l-2 border-blue-500':''}`}>
                                    <Film size={14} className={i===currentIdx?'text-blue-400':'text-slate-500'}/>
                                    <span className="text-xs truncate flex-1">{item.name}</span>
                                    <button onClick={e=>{e.stopPropagation();setPlaylist(p=>p.filter((_,j)=>j!==i));}} className="opacity-0 group-hover:opacity-100 hover:text-red-400"><X size={11}/></button>
                                </div>
                            ))}
                        </div>
                    </div>
                )}
            </div>
            <div className={`shrink-0 bg-gradient-to-t from-black via-black/95 to-transparent transition-opacity duration-300 ${showControls||!playing?'opacity-100':'opacity-0'}`} onClick={e=>e.stopPropagation()}>
                {current&&<div className="px-4 pt-2 pb-0.5"><p className="text-xs text-slate-300 truncate">{current.name}</p></div>}
                <div className="relative px-4 py-2">
                    <div className="relative h-1 bg-white/20 rounded-full overflow-hidden">
                        <div className="absolute h-full bg-white/30 rounded-full" style={{width:`${bufPct}%`}}/>
                        <div className="absolute h-full bg-blue-500 rounded-full" style={{width:`${pct}%`}}/>
                    </div>
                    <input type="range" min={0} max={duration||100} step={0.1} value={currentTime} onChange={seek} className="absolute inset-0 w-full opacity-0 cursor-pointer h-5" style={{top:'50%',transform:'translateY(-50%)'}}/>
                </div>
                <div className="flex items-center justify-between px-4 pb-3">
                    <div className="flex items-center gap-1">
                        <button onClick={()=>setCurrentIdx(i=>Math.max(0,i-1))} className="p-2 hover:bg-white/10 rounded-lg"><SkipBack size={16}/></button>
                        <button onClick={togglePlay} className="p-2.5 bg-white/10 hover:bg-white/20 rounded-xl">{playing?<Pause size={18}/>:<Play size={18}/>}</button>
                        <button onClick={()=>setCurrentIdx(i=>Math.min(playlist.length-1,i+1))} className="p-2 hover:bg-white/10 rounded-lg"><SkipForward size={16}/></button>
                        <div className="flex items-center gap-1 ml-2">
                            <button onClick={toggleMute} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-300">{muted||volume===0?<VolumeX size={15}/>:<Volume2 size={15}/>}</button>
                            <input type="range" min={0} max={1} step={0.05} value={muted?0:volume} onChange={changeVolume} className="w-20 accent-blue-400"/>
                        </div>
                        <span className="text-xs text-slate-400 ml-2 tabular-nums">{fmtTime(currentTime)} / {fmtTime(duration)}</span>
                    </div>
                    <div className="flex items-center gap-1">
                        <button onClick={()=>setShuffle(s=>!s)} className={`p-1.5 rounded-lg ${shuffle?'text-blue-400':'text-slate-500 hover:text-white'}`}><Shuffle size={14}/></button>
                        <button onClick={()=>setRepeat(r=>r==='none'?'all':r==='all'?'one':'none')} className={`p-1.5 rounded-lg ${repeat!=='none'?'text-blue-400':'text-slate-500 hover:text-white'}`}><RepeatIcon size={14}/></button>
                        <div className="relative">
                            <button onClick={()=>setShowSettings(s=>!s)} className={`p-1.5 rounded-lg ${showSettings?'text-blue-400':'text-slate-500 hover:text-white'}`}><Settings size={14}/></button>
                            {showSettings&&(
                                <div className="absolute bottom-8 right-0 bg-slate-800 border border-white/10 rounded-xl p-2 z-50 min-w-28">
                                    <div className="text-[10px] text-slate-500 px-2 mb-1">Speed</div>
                                    {[0.25,0.5,0.75,1,1.25,1.5,2].map(s=>(
                                        <button key={s} onClick={()=>changeSpeed(s)} className={`w-full text-left px-3 py-1 rounded-lg text-xs ${speed===s?'bg-blue-600 text-white':'text-slate-300 hover:bg-white/10'}`}>{s===1?'Normal':`${s}×`}</button>
                                    ))}
                                </div>
                            )}
                        </div>
                        <button onClick={()=>setShowPlaylist(s=>!s)} className={`p-1.5 rounded-lg ${showPlaylist?'text-blue-400':'text-slate-500 hover:text-white'}`}><ListVideo size={14}/></button>
                        <button onClick={()=>fileInputRef.current?.click()} className="p-1.5 rounded-lg text-slate-500 hover:text-white"><FolderOpen size={14}/></button>
                        <button onClick={toggleFullscreen} className="p-1.5 rounded-lg text-slate-500 hover:text-white">{fullscreen?<Minimize2 size={14}/>:<Maximize2 size={14}/>}</button>
                    </div>
                </div>
            </div>
        </div>
    );
};
export default BlueVideoApp;

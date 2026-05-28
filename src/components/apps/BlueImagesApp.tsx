import React, { useState, useEffect, useRef, useCallback } from 'react';
import { AppProps } from '../../types';
import {
    ChevronLeft, ChevronRight, ZoomIn, ZoomOut, RotateCcw, RotateCw,
    Maximize2, Minimize2, FolderOpen, Grid, Image, Info,
    Download, Trash2, FlipHorizontal, FlipVertical, Star, X
} from 'lucide-react';

interface ImageItem { name: string; url: string; file?: File; }

const BlueImagesApp: React.FC<AppProps> = () => {
    const [images, setImages] = useState<ImageItem[]>([]);
    const [idx, setIdx] = useState(0);
    const [zoom, setZoom] = useState(1);
    const [rotation, setRotation] = useState(0);
    const [flipH, setFlipH] = useState(false);
    const [flipV, setFlipV] = useState(false);
    const [view, setView] = useState<'single'|'grid'>('single');
    const [fullscreen, setFullscreen] = useState(false);
    const [showInfo, setShowInfo] = useState(false);
    const [pan, setPan] = useState({x:0,y:0});
    const [dragging, setDragging] = useState(false);
    const [dragStart, setDragStart] = useState({x:0,y:0,px:0,py:0});
    const [imgSize, setImgSize] = useState({w:0,h:0});
    const [favorites, setFavorites] = useState<Set<string>>(new Set());
    const fileInputRef = useRef<HTMLInputElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const current = images[idx] ?? null;
    const resetView = useCallback(()=>{setZoom(1);setRotation(0);setFlipH(false);setFlipV(false);setPan({x:0,y:0});}, []);
    useEffect(()=>{resetView();},[idx]);

    const addFiles = (e: React.ChangeEvent<HTMLInputElement>) => {
        const files = Array.from(e.target.files||[]).filter(f=>f.type.startsWith('image/'));
        const items = files.map(f=>({name:f.name,url:URL.createObjectURL(f),file:f}));
        setImages(prev=>{if(prev.length===0)setIdx(0);return[...prev,...items];});
        e.target.value='';
    };

    const go = (dir: -1|1) => setIdx(i=>Math.max(0,Math.min(images.length-1,i+dir)));

    useEffect(()=>{
        const handleKey = (e: KeyboardEvent) => {
            if(e.key==='ArrowLeft')go(-1);
            else if(e.key==='ArrowRight')go(1);
            else if(e.key==='+'||e.key==='=')setZoom(z=>Math.min(8,z*1.2));
            else if(e.key==='-')setZoom(z=>Math.max(0.1,z/1.2));
            else if(e.key==='0')resetView();
            else if(e.key==='r')setRotation(r=>(r+90)%360);
            else if(e.key==='Delete')setImages(prev=>{const n=prev.filter((_,i)=>i!==idx);setIdx(i=>Math.min(i,n.length-1));return n;});
        };
        window.addEventListener('keydown',handleKey);
        return()=>window.removeEventListener('keydown',handleKey);
    },[idx,images.length,resetView]);

    const handleWheel = (e: React.WheelEvent) => {
        e.preventDefault();
        setZoom(z=>Math.max(0.1,Math.min(8,z*(e.deltaY<0?1.1:0.9))));
    };

    const startDrag = (e: React.MouseEvent) => { if(zoom<=1)return; setDragging(true); setDragStart({x:e.clientX,y:e.clientY,px:pan.x,py:pan.y}); };
    const doDrag = (e: React.MouseEvent) => { if(!dragging)return; setPan({x:dragStart.px+e.clientX-dragStart.x,y:dragStart.py+e.clientY-dragStart.y}); };
    const endDrag = () => setDragging(false);

    const download=()=>{if(!current)return;const a=document.createElement('a');a.href=current.url;a.download=current.name;a.click();};
    const toggleFullscreen=async()=>{if(!document.fullscreenElement){await containerRef.current?.requestFullscreen();setFullscreen(true);}else{await document.exitFullscreen();setFullscreen(false);}};
    const toggleFav=(name:string)=>setFavorites(prev=>{const n=new Set(prev);n.has(name)?n.delete(name):n.add(name);return n;});

    const transform=`rotate(${rotation}deg) scaleX(${flipH?-1:1}) scaleY(${flipV?-1:1}) scale(${zoom}) translate(${pan.x/zoom}px,${pan.y/zoom}px)`;

    return (
        <div ref={containerRef} className="flex flex-col h-full bg-slate-950 text-white overflow-hidden" onWheel={handleWheel}>
            <input ref={fileInputRef} type="file" multiple accept="image/*" className="hidden" onChange={addFiles}/>
            <div className="shrink-0 flex items-center gap-1 px-3 py-2 bg-slate-900 border-b border-white/5">
                <button onClick={()=>fileInputRef.current?.click()} className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 rounded-lg text-sm"><FolderOpen size={14}/> Open</button>
                <div className="w-px h-5 bg-white/10 mx-1"/>
                <button onClick={()=>setView(v=>v==='single'?'grid':'single')} className={`p-1.5 rounded-lg ${view==='grid'?'text-blue-400 bg-blue-500/10':'text-slate-400 hover:bg-white/10'}`}><Grid size={16}/></button>
                {view==='single'&&current&&(<>
                    <div className="w-px h-5 bg-white/10 mx-1"/>
                    <button onClick={()=>setZoom(z=>Math.min(8,z*1.25))} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><ZoomIn size={15}/></button>
                    <button onClick={()=>setZoom(z=>Math.max(0.1,z/1.25))} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><ZoomOut size={15}/></button>
                    <button onClick={resetView} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 text-xs px-2">1:1</button>
                    <div className="w-px h-5 bg-white/10 mx-1"/>
                    <button onClick={()=>setRotation(r=>(r-90+360)%360)} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><RotateCcw size={15}/></button>
                    <button onClick={()=>setRotation(r=>(r+90)%360)} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><RotateCw size={15}/></button>
                    <button onClick={()=>setFlipH(h=>!h)} className={`p-1.5 rounded-lg ${flipH?'text-blue-400 bg-blue-500/10':'text-slate-400 hover:bg-white/10'}`}><FlipHorizontal size={15}/></button>
                    <button onClick={()=>setFlipV(v=>!v)} className={`p-1.5 rounded-lg ${flipV?'text-blue-400 bg-blue-500/10':'text-slate-400 hover:bg-white/10'}`}><FlipVertical size={15}/></button>
                    <div className="w-px h-5 bg-white/10 mx-1"/>
                    <button onClick={()=>toggleFav(current.name)} className={`p-1.5 rounded-lg ${favorites.has(current.name)?'text-yellow-400':'text-slate-400 hover:bg-white/10'}`}><Star size={15}/></button>
                    <button onClick={download} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400"><Download size={15}/></button>
                    <button onClick={()=>setImages(prev=>{const n=prev.filter((_,i)=>i!==idx);setIdx(i=>Math.min(i,n.length-1));return n;})} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400 hover:text-red-400"><Trash2 size={15}/></button>
                    <div className="w-px h-5 bg-white/10 mx-1"/>
                    <button onClick={()=>setShowInfo(s=>!s)} className={`p-1.5 rounded-lg ${showInfo?'text-blue-400 bg-blue-500/10':'text-slate-400 hover:bg-white/10'}`}><Info size={15}/></button>
                    <button onClick={toggleFullscreen} className="p-1.5 hover:bg-white/10 rounded-lg text-slate-400">{fullscreen?<Minimize2 size={15}/>:<Maximize2 size={15}/>}</button>
                </>)}
                <div className="flex-1"/>
                {images.length>0&&<span className="text-xs text-slate-500">{idx+1} / {images.length}</span>}
            </div>

            <div className="flex-1 flex overflow-hidden">
                {view==='grid'?(
                    <div className="flex-1 overflow-y-auto p-3">
                        {images.length===0?(
                            <div className="flex flex-col items-center justify-center h-full gap-4">
                                <Image size={48} className="text-slate-600"/><p className="text-slate-500">No images loaded</p>
                                <button onClick={()=>fileInputRef.current?.click()} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">Open Images</button>
                            </div>
                        ):(
                            <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
                                {images.map((img,i)=>(
                                    <div key={i} onClick={()=>{setIdx(i);setView('single');}} className={`relative aspect-square rounded-xl overflow-hidden cursor-pointer border-2 ${i===idx?'border-blue-500':'border-transparent hover:border-white/20'}`}>
                                        <img src={img.url} className="w-full h-full object-cover" alt={img.name}/>
                                        {favorites.has(img.name)&&<div className="absolute top-1 right-1"><Star size={12} className="text-yellow-400 fill-yellow-400"/></div>}
                                        <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/70 to-transparent px-2 py-1"><p className="text-[10px] text-white truncate">{img.name}</p></div>
                                    </div>
                                ))}
                            </div>
                        )}
                    </div>
                ):(
                    <div className="flex-1 flex overflow-hidden">
                        <div className="flex items-center px-2"><button onClick={()=>go(-1)} disabled={idx===0} className="p-2 rounded-xl bg-black/30 hover:bg-black/50 disabled:opacity-20"><ChevronLeft size={20}/></button></div>
                        <div className="flex-1 relative overflow-hidden flex items-center justify-center bg-slate-950"
                            onMouseDown={startDrag} onMouseMove={doDrag} onMouseUp={endDrag} onMouseLeave={endDrag}
                            style={{cursor:zoom>1?(dragging?'grabbing':'grab'):'default'}}>
                            {current?(
                                <img src={current.url} alt={current.name} draggable={false}
                                    onLoad={e=>{const i=e.target as HTMLImageElement;setImgSize({w:i.naturalWidth,h:i.naturalHeight});}}
                                    style={{transform,maxWidth:'100%',maxHeight:'100%',objectFit:'contain',transition:dragging?'none':'transform 0.1s',userSelect:'none'}}/>
                            ):(
                                <div className="flex flex-col items-center gap-4">
                                    <Image size={64} className="text-slate-700"/><p className="text-slate-500">No image</p>
                                    <button onClick={()=>fileInputRef.current?.click()} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">Open Images</button>
                                </div>
                            )}
                            {zoom!==1&&<div className="absolute top-3 left-3 bg-black/60 backdrop-blur-sm rounded-lg px-2 py-1 text-xs text-white">{Math.round(zoom*100)}%</div>}
                            {showInfo&&current&&(
                                <div className="absolute top-3 right-3 bg-slate-900/95 border border-white/10 rounded-xl p-4 w-48 text-xs space-y-1.5">
                                    <div className="font-semibold text-white truncate">{current.name}</div>
                                    <div className="flex justify-between text-slate-400"><span>Size</span><span className="text-white">{imgSize.w}×{imgSize.h}</span></div>
                                    <div className="flex justify-between text-slate-400"><span>Zoom</span><span className="text-white">{Math.round(zoom*100)}%</span></div>
                                    <div className="flex justify-between text-slate-400"><span>Rotation</span><span className="text-white">{rotation}°</span></div>
                                </div>
                            )}
                            {images.length>1&&(
                                <div className="absolute bottom-3 left-1/2 -translate-x-1/2 flex gap-1.5 bg-black/60 backdrop-blur-sm rounded-xl p-1.5 max-w-xs overflow-x-auto">
                                    {images.slice(Math.max(0,idx-3),idx+4).map((img,i)=>{const realIdx=Math.max(0,idx-3)+i;return(
                                        <div key={realIdx} onClick={()=>setIdx(realIdx)} className={`w-10 h-10 rounded-lg overflow-hidden cursor-pointer border-2 shrink-0 ${realIdx===idx?'border-blue-400 scale-110':'border-transparent opacity-60 hover:opacity-100'}`}>
                                            <img src={img.url} className="w-full h-full object-cover" alt=""/>
                                        </div>
                                    );})}
                                </div>
                            )}
                        </div>
                        <div className="flex items-center px-2"><button onClick={()=>go(1)} disabled={idx===images.length-1} className="p-2 rounded-xl bg-black/30 hover:bg-black/50 disabled:opacity-20"><ChevronRight size={20}/></button></div>
                    </div>
                )}
            </div>

            <div className="shrink-0 flex items-center px-4 py-1.5 bg-slate-900 border-t border-white/5 text-xs text-slate-500 gap-4">
                {current?(<><span className="truncate">{current.name}</span>{imgSize.w>0&&<span>{imgSize.w}×{imgSize.h}</span>}<span>{Math.round(zoom*100)}%</span><span className="ml-auto">← → Navigate · Scroll Zoom · 0 Reset</span></>):<span>Blue Images — Open image files to view</span>}
            </div>
        </div>
    );
};
export default BlueImagesApp;

import React, { useState, useCallback, useRef, useEffect } from 'react';
import { AppProps } from '../../../types';
import { Image } from 'lucide-react';
import { useImages } from './src/useImages';
import Toolbar from './src/Toolbar';

const BlueImagesApp: React.FC<AppProps> = () => {
    const { images, idx, setIdx, openFiles, remove } = useImages();
    const [zoom,       setZoom]       = useState(1);
    const [rotation,   setRotation]   = useState(0);
    const [flipH,      setFlipH]      = useState(false);
    const [flipV,      setFlipV]      = useState(false);
    const [view,       setView]       = useState<'single' | 'grid'>('single');
    const [fullscreen, setFullscreen] = useState(false);
    const [showInfo,   setShowInfo]   = useState(false);
    const [pan,        setPan]        = useState({ x: 0, y: 0 });
    const [dragging,   setDragging]   = useState(false);
    const [dragStart,  setDragStart]  = useState({ x: 0, y: 0, px: 0, py: 0 });
    const [imgSize,    setImgSize]    = useState({ w: 0, h: 0 });
    const [favorites,  setFavorites]  = useState<Set<string>>(new Set());
    const containerRef = useRef<HTMLDivElement>(null);

    const current = images[idx] ?? null;
    const resetView = useCallback(() => {
        setZoom(1); setRotation(0); setFlipH(false); setFlipV(false); setPan({ x: 0, y: 0 });
    }, []);

    useEffect(() => { resetView(); }, [idx]);

    const toggleFav = () => {
        if (!current) return;
        setFavorites(prev => {
            const next = new Set(prev);
            next.has(current.url) ? next.delete(current.url) : next.add(current.url);
            return next;
        });
    };

    const downloadCurrent = () => {
        if (!current) return;
        const a = document.createElement('a');
        a.href = current.url; a.download = current.name; a.click();
    };

    const toggleFullscreen = async () => {
        if (!document.fullscreenElement) {
            await containerRef.current?.requestFullscreen();
            setFullscreen(true);
        } else {
            await document.exitFullscreen();
            setFullscreen(false);
        }
    };

    const transform = `rotate(${rotation}deg) scaleX(${flipH ? -1 : 1}) scaleY(${flipV ? -1 : 1})`;

    return (
        <div ref={containerRef} className="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
            <Toolbar
                hasImage={!!current}
                idx={idx} total={images.length}
                zoom={zoom} rotation={rotation} flipH={flipH} flipV={flipV}
                fullscreen={fullscreen} showInfo={showInfo}
                isFav={current ? favorites.has(current.url) : false}
                view={view}
                onPrev={() => { if (idx > 0) setIdx(idx - 1); }}
                onNext={() => { if (idx < images.length - 1) setIdx(idx + 1); }}
                onZoomIn={() => setZoom(z => Math.min(z + 0.25, 5))}
                onZoomOut={() => setZoom(z => Math.max(z - 0.25, 0.1))}
                onRotateL={() => setRotation(r => r - 90)}
                onRotateR={() => setRotation(r => r + 90)}
                onFlipH={() => setFlipH(f => !f)}
                onFlipV={() => setFlipV(f => !f)}
                onFullscreen={toggleFullscreen}
                onToggleInfo={() => setShowInfo(s => !s)}
                onToggleFav={toggleFav}
                onOpenFiles={openFiles}
                onToggleView={() => setView(v => v === 'single' ? 'grid' : 'single')}
                onDownload={downloadCurrent}
                onRemove={() => remove(idx)}
                onReset={resetView}
            />

            <div className="flex-1 overflow-hidden relative">
                {images.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full gap-4 text-slate-600">
                        <Image size={48} />
                        <p className="text-sm text-slate-400">No images open</p>
                        <button onClick={openFiles} className="px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm text-white">Open Images</button>
                    </div>
                ) : view === 'grid' ? (
                    <div className="grid grid-cols-4 gap-2 p-3 overflow-y-auto h-full">
                        {images.map((img, i) => (
                            <div key={i} onClick={() => { setIdx(i); setView('single'); }}
                                className={`aspect-square rounded-xl overflow-hidden cursor-pointer border-2 transition-colors ${i === idx ? 'border-blue-500' : 'border-transparent hover:border-white/20'}`}>
                                <img src={img.url} alt={img.name} className="w-full h-full object-cover" />
                            </div>
                        ))}
                    </div>
                ) : (
                    <div className="flex h-full overflow-hidden">
                        <div className="flex-1 flex items-center justify-center overflow-hidden"
                            onMouseDown={e => { if (zoom > 1) { setDragging(true); setDragStart({ x: e.clientX, y: e.clientY, px: pan.x, py: pan.y }); } }}
                            onMouseMove={e => { if (dragging) setPan({ x: dragStart.px + e.clientX - dragStart.x, y: dragStart.py + e.clientY - dragStart.y }); }}
                            onMouseUp={() => setDragging(false)}
                            onWheel={e => { e.preventDefault(); setZoom(z => Math.max(0.1, Math.min(5, z - e.deltaY * 0.001))); }}
                        >
                            <img
                                src={current!.url} alt={current!.name}
                                className="max-w-full max-h-full object-contain select-none"
                                style={{ transform: `translate(${pan.x}px,${pan.y}px) scale(${zoom}) ${transform}`, cursor: zoom > 1 ? 'grab' : 'default', transition: dragging ? 'none' : 'transform 0.1s' }}
                                onLoad={e => { const img = e.currentTarget; setImgSize({ w: img.naturalWidth, h: img.naturalHeight }); }}
                                draggable={false}
                            />
                        </div>

                        {showInfo && current && (
                            <div className="w-52 bg-slate-800 border-l border-white/5 p-4 shrink-0 text-xs text-slate-400 space-y-2">
                                <div className="font-medium text-white text-sm truncate">{current.name}</div>
                                <div>Size: {imgSize.w} × {imgSize.h}px</div>
                                <div>Zoom: {Math.round(zoom * 100)}%</div>
                                <div>Rotation: {rotation}°</div>
                                {current.path && <div className="text-slate-600 break-all">{current.path}</div>}
                            </div>
                        )}
                    </div>
                )}
            </div>

            {images.length > 1 && view === 'single' && (
                <div className="flex gap-1.5 px-3 py-2 bg-slate-800/70 border-t border-white/5 overflow-x-auto shrink-0">
                    {images.map((img, i) => (
                        <div key={i} onClick={() => setIdx(i)}
                            className={`w-10 h-10 rounded shrink-0 overflow-hidden cursor-pointer border-2 ${i === idx ? 'border-blue-500' : 'border-transparent'}`}>
                            <img src={img.url} alt="" className="w-full h-full object-cover" />
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
};

export default BlueImagesApp;

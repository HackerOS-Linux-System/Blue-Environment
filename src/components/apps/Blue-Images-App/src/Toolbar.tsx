import React from 'react';
import {
    ChevronLeft, ChevronRight, ZoomIn, ZoomOut, RotateCcw, RotateCw,
    Maximize2, Minimize2, FolderOpen, Grid, FlipHorizontal, FlipVertical,
    Star, StarOff, Download, Trash2, Info,
} from 'lucide-react';

interface Props {
    hasImage: boolean;
    idx: number;
    total: number;
    zoom: number;
    rotation: number;
    flipH: boolean;
    flipV: boolean;
    fullscreen: boolean;
    showInfo: boolean;
    isFav: boolean;
    view: 'single' | 'grid';
    onPrev: () => void;
    onNext: () => void;
    onZoomIn: () => void;
    onZoomOut: () => void;
    onRotateL: () => void;
    onRotateR: () => void;
    onFlipH: () => void;
    onFlipV: () => void;
    onFullscreen: () => void;
    onToggleInfo: () => void;
    onToggleFav: () => void;
    onOpenFiles: () => void;
    onToggleView: () => void;
    onDownload: () => void;
    onRemove: () => void;
    onReset: () => void;
}

const Toolbar: React.FC<Props> = ({
    hasImage, idx, total, zoom, rotation, flipH, flipV, fullscreen,
    showInfo, isFav, view,
    onPrev, onNext, onZoomIn, onZoomOut, onRotateL, onRotateR,
    onFlipH, onFlipV, onFullscreen, onToggleInfo, onToggleFav,
    onOpenFiles, onToggleView, onDownload, onRemove, onReset,
}) => (
    <div className="flex items-center gap-1 px-2 py-1.5 bg-slate-800 border-b border-white/5 shrink-0 flex-wrap">
        <button onClick={onOpenFiles} className="flex items-center gap-1 px-2.5 py-1.5 bg-slate-700 hover:bg-slate-600 rounded text-xs">
            <FolderOpen size={13}/> Open
        </button>
        <div className="w-px h-5 bg-white/10 mx-0.5" />
        {hasImage && <>
            <button onClick={onPrev}  className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30" disabled={idx === 0}><ChevronLeft size={16}/></button>
            <span className="text-xs text-slate-400 min-w-[40px] text-center">{idx+1}/{total}</span>
            <button onClick={onNext}  className="p-1.5 hover:bg-white/10 rounded disabled:opacity-30" disabled={idx === total-1}><ChevronRight size={16}/></button>
            <div className="w-px h-5 bg-white/10 mx-0.5" />
            <button onClick={onZoomOut}   className="p-1.5 hover:bg-white/10 rounded"><ZoomOut size={15}/></button>
            <span className="text-xs text-slate-400 w-12 text-center">{Math.round(zoom*100)}%</span>
            <button onClick={onZoomIn}    className="p-1.5 hover:bg-white/10 rounded"><ZoomIn size={15}/></button>
            <button onClick={onReset}     className="text-xs text-slate-400 hover:text-white px-1">1:1</button>
            <div className="w-px h-5 bg-white/10 mx-0.5" />
            <button onClick={onRotateL}   className="p-1.5 hover:bg-white/10 rounded"><RotateCcw size={14}/></button>
            <button onClick={onRotateR}   className="p-1.5 hover:bg-white/10 rounded"><RotateCw size={14}/></button>
            <button onClick={onFlipH}     className={`p-1.5 rounded ${flipH ? 'bg-blue-600/30 text-blue-400' : 'hover:bg-white/10'}`}><FlipHorizontal size={14}/></button>
            <button onClick={onFlipV}     className={`p-1.5 rounded ${flipV ? 'bg-blue-600/30 text-blue-400' : 'hover:bg-white/10'}`}><FlipVertical size={14}/></button>
            <div className="w-px h-5 bg-white/10 mx-0.5" />
            <button onClick={onToggleFav} className="p-1.5 hover:bg-white/10 rounded">{isFav ? <Star size={14} className="text-yellow-400 fill-yellow-400"/> : <StarOff size={14}/>}</button>
            <button onClick={onDownload}  className="p-1.5 hover:bg-white/10 rounded"><Download size={14}/></button>
            <button onClick={onRemove}    className="p-1.5 hover:bg-red-500/20 text-slate-400 hover:text-red-400 rounded"><Trash2 size={14}/></button>
            <div className="w-px h-5 bg-white/10 mx-0.5" />
            <button onClick={onToggleInfo}   className={`p-1.5 rounded ${showInfo ? 'text-blue-400 bg-blue-600/20' : 'hover:bg-white/10'}`}><Info size={14}/></button>
            <button onClick={onToggleView}   className={`p-1.5 rounded ${view==='grid' ? 'text-blue-400 bg-blue-600/20' : 'hover:bg-white/10'}`}><Grid size={14}/></button>
            <button onClick={onFullscreen}   className="p-1.5 hover:bg-white/10 rounded">{fullscreen ? <Minimize2 size={14}/> : <Maximize2 size={14}/>}</button>
        </>}
    </div>
);

export default Toolbar;

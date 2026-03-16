import React, { useState, useRef, useEffect, memo, useCallback } from 'react';
import { X, Minus, Maximize2, Square } from 'lucide-react';
import { WindowState } from '../types';

interface WindowProps {
    window: WindowState;
    isActive: boolean;
    onClose: (id: string) => void;
    onMinimize: (id: string) => void;
    onMaximize: (id: string) => void;
    onFocus: (id: string) => void;
    onMove: (id: string, x: number, y: number) => void;
    onResize: (id: string, width: number, height: number) => void;
    children: React.ReactNode;
}

const TOPBAR_HEIGHT = 48;
const MIN_WIDTH = 280;
const MIN_HEIGHT = 200;

const Window: React.FC<WindowProps> = ({
    window: win,
    isActive,
    onClose,
    onMinimize,
    onMaximize,
    onFocus,
    onMove,
    onResize,
    children,
}) => {
    const [isDragging, setIsDragging] = useState(false);
    const [isResizing, setIsResizing] = useState(false);
    const dragOffset = useRef({ x: 0, y: 0 });
    const resizeStart = useRef({ x: 0, y: 0, w: 0, h: 0 });

    // ── Drag ──────────────────────────────────────────────
    const handleTitleMouseDown = useCallback((e: React.MouseEvent) => {
        if (win.isMaximized) return;
        if ((e.target as HTMLElement).closest('button')) return;
        e.preventDefault();
        onFocus(win.id);
        setIsDragging(true);
        dragOffset.current = { x: e.clientX - win.x, y: e.clientY - win.y };
    }, [win.isMaximized, win.id, win.x, win.y, onFocus]);

    // ── Resize ────────────────────────────────────────────
    const handleResizeMouseDown = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        onFocus(win.id);
        setIsResizing(true);
        resizeStart.current = {
            x: e.clientX,
            y: e.clientY,
            w: win.width,
            h: win.height,
        };
    }, [win.id, win.width, win.height, onFocus]);

    useEffect(() => {
        if (!isDragging && !isResizing) return;

        const handleMouseMove = (e: MouseEvent) => {
            if (isDragging) {
                const newX = e.clientX - dragOffset.current.x;
                const newY = Math.max(TOPBAR_HEIGHT, e.clientY - dragOffset.current.y);
                onMove(win.id, newX, newY);
            }
            if (isResizing) {
                const dx = e.clientX - resizeStart.current.x;
                const dy = e.clientY - resizeStart.current.y;
                const newW = Math.max(MIN_WIDTH, resizeStart.current.w + dx);
                const newH = Math.max(MIN_HEIGHT, resizeStart.current.h + dy);
                onResize(win.id, newW, newH);
            }
        };

        const handleMouseUp = () => {
            setIsDragging(false);
            setIsResizing(false);
        };

        document.addEventListener('mousemove', handleMouseMove);
        document.addEventListener('mouseup', handleMouseUp);
        return () => {
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('mouseup', handleMouseUp);
        };
    }, [isDragging, isResizing, win.id, onMove, onResize]);

    if (win.isMinimized) return null;

    const style: React.CSSProperties = win.isMaximized
        ? {
            left: 0,
            top: TOPBAR_HEIGHT,
            width: '100vw',
            height: `calc(100vh - ${TOPBAR_HEIGHT}px)`,
            borderRadius: 0,
            border: 'none',
            zIndex: win.zIndex,
        }
        : {
            left: win.x,
            top: win.y,
            width: win.width,
            height: win.height,
            zIndex: win.zIndex,
        };

    return (
        <div
            className={`absolute flex flex-col overflow-hidden shadow-2xl border transition-shadow duration-150
                theme-bg-primary
                ${isActive
                    ? 'border-blue-500/60 shadow-blue-500/20'
                    : 'theme-border shadow-black/60'
                }
                ${isDragging ? 'cursor-grabbing select-none' : ''}
                ${win.isMaximized ? '' : 'rounded-xl'}
            `}
            style={style}
            onMouseDown={() => onFocus(win.id)}
        >
            {/* Title Bar */}
            <div
                className={`h-9 flex items-center justify-between px-3 select-none shrink-0
                    theme-bg-secondary theme-border border-b
                    ${win.isMaximized ? '' : 'cursor-default'}
                `}
                onMouseDown={handleTitleMouseDown}
                onDoubleClick={() => onMaximize(win.id)}
            >
                {/* Left: indicator + title */}
                <div className="flex items-center gap-2 text-sm font-medium theme-text-primary min-w-0">
                    <div className={`w-2 h-2 rounded-full shrink-0 transition-colors ${isActive ? 'bg-blue-400 shadow-[0_0_8px_rgba(96,165,250,0.8)]' : 'bg-slate-600'}`} />
                    <span className="truncate">{win.title}</span>
                </div>

                {/* Right: window controls */}
                <div className="flex items-center gap-0.5 shrink-0" onMouseDown={e => e.stopPropagation()}>
                    <button
                        onClick={() => onMinimize(win.id)}
                        className="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-yellow-400 transition-colors"
                        title="Minimize (Super+↓)"
                    >
                        <Minus size={13} />
                    </button>
                    <button
                        onClick={() => onMaximize(win.id)}
                        className="w-7 h-7 flex items-center justify-center hover:bg-white/10 rounded-md theme-text-secondary hover:text-green-400 transition-colors"
                        title="Maximize (Super+↑)"
                    >
                        {win.isMaximized ? <Square size={11} /> : <Maximize2 size={11} />}
                    </button>
                    <button
                        onClick={() => onClose(win.id)}
                        className="w-7 h-7 flex items-center justify-center hover:bg-red-500/80 rounded-md theme-text-secondary hover:text-white transition-colors"
                        title="Close (Alt+F4)"
                    >
                        <X size={13} />
                    </button>
                </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-auto relative theme-bg-primary theme-text-primary select-text cursor-auto">
                {children}
            </div>

            {/* Resize handle (SE corner) */}
            {!win.isMaximized && (
                <div
                    className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize z-10 group"
                    onMouseDown={handleResizeMouseDown}
                >
                    <svg width="12" height="12" viewBox="0 0 12 12" className="absolute bottom-1 right-1 opacity-30 group-hover:opacity-70 transition-opacity">
                        <path d="M0 12 L12 0 M4 12 L12 4 M8 12 L12 8" stroke="currentColor" strokeWidth="1.5" />
                    </svg>
                </div>
            )}
        </div>
    );
};

// Deep equality check — only re-render when props actually changed
export default memo(Window, (prev, next) => {
    return (
        prev.window === next.window &&
        prev.isActive === next.isActive
    );
});

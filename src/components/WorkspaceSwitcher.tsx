import React, { memo, useEffect, useState } from 'react';

interface WorkspaceSwitcherProps {
    currentWorkspace: number;
    workspaceCount: number;
    windowCounts: number[]; // how many windows per workspace
}

const WorkspaceSwitcher: React.FC<WorkspaceSwitcherProps> = ({
    currentWorkspace,
    workspaceCount,
    windowCounts,
}) => {
    const [visible, setVisible] = useState(false);
    const [prevWorkspace, setPrevWorkspace] = useState(currentWorkspace);

    useEffect(() => {
        if (currentWorkspace !== prevWorkspace) {
            setPrevWorkspace(currentWorkspace);
            setVisible(true);
            const t = setTimeout(() => setVisible(false), 1200);
            return () => clearTimeout(t);
        }
    }, [currentWorkspace, prevWorkspace]);

    if (!visible) return null;

    return (
        <div className="absolute bottom-16 left-1/2 -translate-x-1/2 z-[300] pointer-events-none animate-in fade-in slide-in-from-bottom-4 duration-200">
            <div className="bg-slate-900/95 border border-white/10 rounded-2xl px-6 py-4 shadow-2xl backdrop-blur-xl flex flex-col items-center gap-3">
                <span className="text-xs text-slate-400 font-medium uppercase tracking-widest">
                    Workspace
                </span>

                <div className="flex gap-3 items-center">
                    {Array.from({ length: workspaceCount }, (_, i) => (
                        <div
                            key={i}
                            className={`relative flex flex-col items-center gap-1.5`}
                        >
                            {/* Mini workspace preview */}
                            <div className={`w-14 h-10 rounded-lg border-2 transition-all flex items-end justify-center pb-1 gap-0.5
                                ${i === currentWorkspace
                                    ? 'border-blue-500 bg-blue-600/20 shadow-[0_0_12px_rgba(59,130,246,0.4)]'
                                    : 'border-white/10 bg-white/5'
                                }
                            `}>
                                {/* Mini window indicators */}
                                {Array.from({ length: Math.min(windowCounts[i] ?? 0, 3) }, (_, wi) => (
                                    <div
                                        key={wi}
                                        className={`h-3 w-3 rounded-sm ${i === currentWorkspace ? 'bg-blue-400/60' : 'bg-slate-600'}`}
                                    />
                                ))}
                            </div>

                            {/* Label */}
                            <span className={`text-[11px] font-bold ${i === currentWorkspace ? 'text-blue-400' : 'text-slate-500'}`}>
                                {i + 1}
                            </span>
                        </div>
                    ))}
                </div>

                <div className="flex items-center gap-2 text-xs text-slate-500">
                    <kbd className="px-1.5 py-0.5 bg-slate-800 rounded border border-white/10 text-slate-400">Alt+⊞</kbd>
                    <span>+</span>
                    <kbd className="px-1.5 py-0.5 bg-slate-800 rounded border border-white/10 text-slate-400">1–4</kbd>
                </div>
            </div>
        </div>
    );
};

export default memo(WorkspaceSwitcher);

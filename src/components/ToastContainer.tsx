import React, { useState, useEffect } from 'react';
import { X, Bell } from 'lucide-react';

interface Toast {
    id: string;
    title: string;
    message: string;
}

const ToastItem: React.FC<{ toast: Toast; onClose: () => void }> = ({ toast, onClose }) => {
    useEffect(() => {
        const timer = setTimeout(onClose, 5000);
        return () => clearTimeout(timer);
    }, [onClose]);

    return (
        <div className="w-80 bg-slate-900/95 backdrop-blur border border-white/10 rounded-xl shadow-2xl p-4 animate-in slide-in-from-right-5 duration-200">
            <div className="flex items-start justify-between gap-2">
                <div className="flex items-start gap-2 flex-1 min-w-0">
                    <Bell size={14} className="text-blue-400 mt-0.5 shrink-0" />
                    <div className="flex-1 min-w-0">
                        <div className="font-semibold text-white text-sm truncate">{toast.title}</div>
                        <div className="text-xs text-slate-300 mt-0.5 line-clamp-2">{toast.message}</div>
                    </div>
                </div>
                <button
                    onClick={onClose}
                    className="text-slate-400 hover:text-white shrink-0 p-0.5 rounded hover:bg-white/10 transition-colors"
                >
                    <X size={14} />
                </button>
            </div>
            {/* Progress bar */}
            <div className="mt-2 h-0.5 bg-slate-700 rounded-full overflow-hidden">
                <div
                    className="h-full bg-blue-500 rounded-full"
                    style={{ animation: 'toast-progress 5s linear forwards' }}
                />
            </div>
            <style>{`
                @keyframes toast-progress {
                    from { width: 100%; }
                    to { width: 0%; }
                }
            `}</style>
        </div>
    );
};

const ToastContainer: React.FC = () => {
    const [toasts, setToasts] = useState<Toast[]>([]);

    useEffect(() => {
        const handler = (e: CustomEvent) => {
            const notif = e.detail;
            setToasts(prev => [
                ...prev,
                { id: notif.id || Date.now().toString(), title: notif.title, message: notif.message },
            ]);
        };
        window.addEventListener('blue:show-toast', handler as EventListener);
        return () => window.removeEventListener('blue:show-toast', handler as EventListener);
    }, []);

    const removeToast = (id: string) => {
        setToasts(prev => prev.filter(t => t.id !== id));
    };

    if (toasts.length === 0) return null;

    return (
        <div className="fixed bottom-4 right-4 z-[60] flex flex-col gap-2">
            {toasts.map(toast => (
                <ToastItem key={toast.id} toast={toast} onClose={() => removeToast(toast.id)} />
            ))}
        </div>
    );
};

export default ToastContainer;

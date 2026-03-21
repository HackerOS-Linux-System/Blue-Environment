import React, { useState, useEffect } from 'react';
import { X } from 'lucide-react';

interface Toast {
    id: string;
    title: string;
    message: string;
}

const Toast: React.FC<{ toast: Toast; onClose: () => void }> = ({ toast, onClose }) => {
    useEffect(() => {
        const timer = setTimeout(onClose, 5000);
        return () => clearTimeout(timer);
    }, [onClose]);

    return (
        <div className="fixed bottom-4 right-4 w-80 bg-slate-900/95 backdrop-blur border border-white/10 rounded-xl shadow-2xl p-4 animate-in slide-in-from-right-5 z-[60]">
        <div className="flex items-start justify-between">
        <div className="flex-1">
        <div className="font-semibold text-white">{toast.title}</div>
        <div className="text-sm text-slate-300 mt-1">{toast.message}</div>
        </div>
        <button onClick={onClose} className="text-slate-400 hover:text-white">
        <X size={14} />
        </button>
        </div>
        </div>
    );
};

const ToastContainer: React.FC = () => {
    const [toasts, setToasts] = useState<Toast[]>([]);

    useEffect(() => {
        const handler = (e: CustomEvent) => {
            const notif = e.detail;
            setToasts(prev => [...prev, { id: notif.id, title: notif.title, message: notif.message }]);
        };
        window.addEventListener('blue:show-toast', handler as EventListener);
        return () => window.removeEventListener('blue:show-toast', handler as EventListener);
    }, []);

    const removeToast = (id: string) => {
        setToasts(prev => prev.filter(t => t.id !== id));
    };

    return (
        <div className="fixed bottom-4 right-4 z-[60] space-y-2">
        {toasts.map(toast => (
            <Toast key={toast.id} toast={toast} onClose={() => removeToast(toast.id)} />
        ))}
        </div>
    );
};

export default ToastContainer;

import React, { useState, useEffect } from 'react';
import { X, Bell, Trash2, Check } from 'lucide-react';
import { notificationManager, Notification } from '../utils/notificationManager';

const NotificationCenter: React.FC<{ isOpen: boolean; onClose: () => void }> = ({ isOpen, onClose }) => {
    const [notifications, setNotifications] = useState<Notification[]>([]);

    useEffect(() => {
        if (!isOpen) return;
        return notificationManager.subscribe(setNotifications);
    }, [isOpen]);

    const formatTime = (timestamp: number) => {
        const date = new Date(timestamp);
        return date.toLocaleTimeString();
    };

    if (!isOpen) return null;

    return (
        <div className="absolute top-14 right-4 bottom-4 w-96 bg-slate-900 border border-white/10 rounded-3xl shadow-2xl z-40 animate-in slide-in-from-right-10 duration-300 flex flex-col overflow-hidden">
        <div className="p-5 border-b border-white/5 flex items-center justify-between">
        <h2 className="font-semibold text-lg text-white flex items-center gap-2">
        <Bell size={18} /> Powiadomienia
        </h2>
        <div className="flex gap-2">
        <button
        onClick={() => notificationManager.clearAll()}
        className="text-slate-400 hover:text-white bg-white/5 p-1 rounded-full"
        >
        <Trash2 size={16} />
        </button>
        <button onClick={onClose} className="text-slate-400 hover:text-white bg-white/5 p-1 rounded-full">
        <X size={16} />
        </button>
        </div>
        </div>
        <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {notifications.length === 0 && (
            <div className="text-center text-slate-500 py-12">Brak powiadomień</div>
        )}
        {notifications.map(notif => (
            <div
            key={notif.id}
            className={`bg-slate-800 border-l-4 ${notif.read ? 'border-slate-600' : 'border-blue-500'} rounded-xl p-3`}
            >
            <div className="flex items-start justify-between">
            <div>
            <div className="font-semibold text-white">{notif.title}</div>
            <div className="text-sm text-slate-300 mt-1">{notif.message}</div>
            <div className="text-xs text-slate-500 mt-2">{formatTime(notif.timestamp)}</div>
            </div>
            {!notif.read && (
                <button
                onClick={() => notificationManager.markAsRead(notif.id)}
                className="p-1 hover:bg-white/10 rounded"
                >
                <Check size={14} />
                </button>
            )}
            </div>
            </div>
        ))}
        </div>
        </div>
    );
};

export default NotificationCenter;

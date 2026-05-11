import React, { useState, useEffect } from 'react';
import { X, Bell, Trash2, Check } from 'lucide-react';
import { notificationManager } from '../utils/notificationManager';
import { Notification } from '../types';

const NotificationCenter: React.FC<{ isOpen: boolean; onClose: () => void }> = ({
    isOpen,
    onClose,
}) => {
    const [notifications, setNotifications] = useState<Notification[]>([]);

    useEffect(() => {
        if (!isOpen) return;
        return notificationManager.subscribe(setNotifications);
    }, [isOpen]);

    if (!isOpen) return null;

    const formatTime = (ts: number) => new Date(ts).toLocaleTimeString();

    return (
        <div className="absolute top-14 right-4 bottom-4 w-96 bg-slate-900 border border-white/10 rounded-3xl shadow-2xl z-40 animate-in slide-in-from-right-10 duration-300 flex flex-col overflow-hidden">
            <div className="p-5 border-b border-white/5 flex items-center justify-between">
                <h2 className="font-semibold text-lg text-white flex items-center gap-2">
                    <Bell size={18} /> Notifications
                </h2>
                <div className="flex gap-2">
                    <button
                        onClick={() => notificationManager.clearAll()}
                        className="text-slate-400 hover:text-white bg-white/5 p-1.5 rounded-full transition-colors"
                        title="Clear all"
                    >
                        <Trash2 size={15} />
                    </button>
                    <button
                        onClick={onClose}
                        className="text-slate-400 hover:text-white bg-white/5 p-1.5 rounded-full transition-colors"
                    >
                        <X size={15} />
                    </button>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-4 space-y-3">
                {notifications.length === 0 && (
                    <div className="text-center text-slate-500 py-12 flex flex-col items-center gap-3">
                        <Bell size={32} className="opacity-30" />
                        <p>No notifications</p>
                    </div>
                )}
                {notifications.map(notif => (
                    <div
                        key={notif.id}
                        className={`bg-slate-800 border-l-4 ${
                            notif.read ? 'border-slate-600' : 'border-blue-500'
                        } rounded-xl p-3`}
                    >
                        <div className="flex items-start justify-between">
                            <div>
                                <div className="font-semibold text-white text-sm">{notif.title}</div>
                                <div className="text-xs text-slate-300 mt-1">{notif.message}</div>
                                <div className="text-[10px] text-slate-500 mt-2">
                                    {formatTime(notif.timestamp)}
                                </div>
                            </div>
                            {!notif.read && (
                                <button
                                    onClick={() => notificationManager.markAsRead(notif.id)}
                                    className="p-1 hover:bg-white/10 rounded transition-colors"
                                    title="Mark as read"
                                >
                                    <Check size={14} className="text-blue-400" />
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

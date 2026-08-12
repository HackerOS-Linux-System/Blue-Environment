import type { Notification } from '../types';

type Listener = (notifications: Notification[]) => void;

class NotificationManager {
    private notifications: Notification[] = [];
    private listeners: Set<Listener> = new Set();

    subscribe(listener: Listener): () => void {
        this.listeners.add(listener);
        listener([...this.notifications]);
        return () => {
            this.listeners.delete(listener);
        };
    }

    private notify(): void {
        const copy = [...this.notifications];
        this.listeners.forEach(l => l(copy));
    }

    add(notification: Omit<Notification, 'id' | 'timestamp' | 'read'>): void {
        const n: Notification = {
            ...notification,
            id: Date.now().toString() + Math.random().toString(36).slice(2),
            timestamp: Date.now(),
            read: false,
        };
        this.notifications = [n, ...this.notifications];
        this.notify();
    }

    async markRead(id: string): Promise<void> {
        this.notifications = this.notifications.map(n =>
        n.id === id ? { ...n, read: true } : n
        );
        this.notify();
    }

    async markAsRead(id: string): Promise<void> {
        return this.markRead(id);
    }

    clearAll(): void {
        this.notifications = [];
        this.notify();
    }

    remove(id: string): void {
        this.notifications = this.notifications.filter(n => n.id !== id);
        this.notify();
    }
}

export const notificationManager = new NotificationManager();

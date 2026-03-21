import { SystemBridge } from './systemBridge';

export interface Notification {
    id: string;
    title: string;
    message: string;
    appId?: string;
    timestamp: number;
    read: boolean;
    icon?: string;
    actions?: { label: string; action: string }[];
}

class NotificationManager {
    private listeners: ((notifications: Notification[]) => void)[] = [];
    private notifications: Notification[] = [];

    constructor() {
        this.loadHistory();
        if (SystemBridge.isTauri()) {
            window.addEventListener('blue:notification', this.handleNotification as EventListener);
        }
    }

    private handleNotification = (e: CustomEvent) => {
        this.add(e.detail);
    };

    async loadHistory() {
        const history = await SystemBridge.getNotificationHistory();
        this.notifications = history;
        this.notifyListeners();
    }

    add(notification: Omit<Notification, 'id' | 'timestamp' | 'read'>) {
        const newNotif: Notification = {
            ...notification,
            id: Date.now().toString(),
            timestamp: Date.now(),
            read: false,
        };
        this.notifications.unshift(newNotif);
        this.notifications = this.notifications.slice(0, 100);
        SystemBridge.saveNotificationHistory(this.notifications);
        this.notifyListeners();
        this.showToast(newNotif);
    }

    showToast(notification: Notification) {
        window.dispatchEvent(new CustomEvent('blue:show-toast', { detail: notification }));
    }

    markAsRead(id: string) {
        const notif = this.notifications.find(n => n.id === id);
        if (notif) notif.read = true;
        SystemBridge.saveNotificationHistory(this.notifications);
        this.notifyListeners();
    }

    clearAll() {
        this.notifications = [];
        SystemBridge.saveNotificationHistory(this.notifications);
        this.notifyListeners();
    }

    subscribe(listener: (notifications: Notification[]) => void) {
        this.listeners.push(listener);
        listener(this.notifications);
        return () => {
            this.listeners = this.listeners.filter(l => l !== listener);
        };
    }

    private notifyListeners() {
        this.listeners.forEach(l => l(this.notifications));
    }
}

export const notificationManager = new NotificationManager();

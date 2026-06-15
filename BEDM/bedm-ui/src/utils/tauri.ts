declare global {
  interface Window {
    __TAURI__?: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
  }
}

const isTauri = typeof window !== 'undefined' && !!window.__TAURI__;

async function invoke<T>(cmd: string, args?: unknown): Promise<T> {
  if (isTauri && window.__TAURI__) {
    return window.__TAURI__.invoke(cmd, args) as Promise<T>;
  }
  // Dev mock
  return mockInvoke<T>(cmd, args);
}

// ── Real Tauri commands ────────────────────────────────────────────────────

import type {
  AuthResult,
  DaemonInfo,
  SessionInfo,
  UserInfo,
} from '../types';

export const BedmBridge = {
  connectDaemon: () => invoke<DaemonInfo>('connect_daemon'),

  getSessions: () => invoke<SessionInfo[]>('get_sessions'),

  getUsers: () => invoke<UserInfo[]>('get_users'),

  authenticate: (username: string, password: string) =>
    invoke<AuthResult>('authenticate', { username, password }),

  startSession: (username: string, session: string) =>
    invoke<string>('start_session', { username, session }),

  powerAction: (action: string) =>
    invoke<boolean>('power_action', { action }),

  getWallpaper: () => invoke<string | null>('get_wallpaper'),

  getHostname: () => invoke<string>('get_hostname'),

  getCurrentTime: () => invoke<string>('get_current_time'),

  getCurrentDate: () => invoke<string>('get_current_date'),

  readUserAvatar: (path: string) => invoke<string | null>('read_user_avatar', { path }),
};

// ── Dev mocks (used when running in browser without Tauri) ─────────────────

function mockInvoke<T>(cmd: string, args?: unknown): Promise<T> {
  const MOCK_USERS: UserInfo[] = [
    {
      username: 'michal',
      realname: 'Michał Kowalski',
      uid: 1000,
      home: '/home/michal',
      shell: '/bin/bash',
      icon_path: undefined,
      last_session: 'blue-environment',
    },
    {
      username: 'admin',
      realname: 'System Administrator',
      uid: 1001,
      home: '/home/admin',
      shell: '/bin/bash',
      icon_path: undefined,
      last_session: undefined,
    },
  ];

  const MOCK_SESSIONS: SessionInfo[] = [
    {
      id: 'blue-environment',
      name: 'Blue Environment',
      exec: '/usr/share/Blue-Environment/lib/blue-compositor',
      session_type: 'wayland',
      desktop_names: ['Blue'],
      comment: 'Blue Environment Wayland Desktop',
    },
    {
      id: 'gnome',
      name: 'GNOME',
      exec: 'gnome-session',
      session_type: 'wayland',
      desktop_names: ['GNOME'],
      comment: 'GNOME Desktop Environment',
    },
    {
      id: 'plasma',
      name: 'KDE Plasma',
      exec: 'startplasma-wayland',
      session_type: 'wayland',
      desktop_names: ['KDE'],
      comment: 'KDE Plasma Desktop',
    },
    {
      id: 'openbox',
      name: 'Openbox',
      exec: 'openbox-session',
      session_type: 'x11',
      desktop_names: ['Openbox'],
      comment: 'Openbox Window Manager',
    },
  ];

  const delay = (ms: number) => new Promise(r => setTimeout(r, ms));

  switch (cmd) {
    case 'connect_daemon':
      return delay(600).then(
        () =>
          ({
            version: '1.0.0',
            hostname: 'hackeros',
            uptime: 3600,
            os_name: 'HackerOS Linux',
            os_version: '0.2.0-alpha',
            connected: true,
          } as unknown as T)
      );

    case 'get_users':
      return delay(200).then(() => MOCK_USERS as unknown as T);

    case 'get_sessions':
      return delay(200).then(() => MOCK_SESSIONS as unknown as T);

    case 'authenticate': {
      const { username, password } = args as { username: string; password: string };
      return delay(800).then(() => {
        if (password === 'demo' || password === '') {
          return { success: true, username, error: undefined, attempts_left: 5 } as unknown as T;
        }
        return {
          success: false,
          username: undefined,
          error: 'Incorrect password',
          attempts_left: 4,
        } as unknown as T;
      });
    }

    case 'start_session':
      return delay(1200).then(() => 'mock-session-id' as unknown as T);

    case 'power_action':
      return delay(300).then(() => true as unknown as T);

    case 'get_wallpaper':
      return Promise.resolve(null as unknown as T);

    case 'get_hostname':
      return Promise.resolve('hackeros' as unknown as T);

    case 'get_current_time': {
      const now = new Date();
      const t = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}`;
      return Promise.resolve(t as unknown as T);
    }

    case 'get_current_date': {
      const now = new Date();
      const opts: Intl.DateTimeFormatOptions = {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric',
      };
      return Promise.resolve(now.toLocaleDateString('en-US', opts) as unknown as T);
    }

    case 'read_user_avatar':
      return Promise.resolve(null as unknown as T);

    default:
      console.warn('[MockBridge] Unknown command:', cmd);
      return Promise.resolve(null as unknown as T);
  }
}

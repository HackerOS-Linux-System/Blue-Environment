import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  Power, ChevronLeft, Eye, EyeOff, AlertCircle,
  Loader2, ChevronDown, Wifi, Volume2,
  Shield, Monitor,
} from 'lucide-react';

import { Background } from './components/Background';
import { Clock } from './components/Clock';
import { UserCard } from './components/UserCard';
import { SessionPicker } from './components/SessionPicker';
import { PowerMenu } from './components/PowerMenu';
import { BedmBridge } from './utils/tauri';
import type {
  DaemonInfo, UserInfo, SessionInfo,
  Screen, PowerAction,
} from './types';

export default function App() {
  // ── State ────────────────────────────────────────────────────────────────
  const [screen, setScreen] = useState<Screen>('connecting');
  const [daemon, setDaemon] = useState<DaemonInfo | null>(null);
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [wallpaper, setWallpaper] = useState<string | null>(null);
  const [avatars, setAvatars] = useState<Record<string, string>>({});

  const [selectedUser, setSelectedUser] = useState<UserInfo | null>(null);
  const [selectedSession, setSelectedSession] = useState<string>('');
  const [showSessionPicker, setShowSessionPicker] = useState(false);

  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [attemptsLeft, setAttemptsLeft] = useState(5);
  const [shake, setShake] = useState(false);
  const [isAuthenticating, setIsAuthenticating] = useState(false);

  const [showPowerMenu, setShowPowerMenu] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);

  const passwordRef = useRef<HTMLInputElement>(null);

  // ── Init ─────────────────────────────────────────────────────────────────
  useEffect(() => {
    init();
  }, []);

  const init = async () => {
    try {
      // Connect to daemon
      const info = await BedmBridge.connectDaemon();
      setDaemon(info);

      // Parallel loads
      const [usersData, sessionsData, wp] = await Promise.all([
        BedmBridge.getUsers(),
        BedmBridge.getSessions(),
        BedmBridge.getWallpaper(),
      ]);

      setUsers(usersData);
      setSessions(sessionsData);
      setWallpaper(wp);

      // Pre-select default session
      const defaultSession = sessionsData.find(s => s.id === 'blue-environment')
        ?? sessionsData[0];
      if (defaultSession) setSelectedSession(defaultSession.id);

      // Load avatars in background
      loadAvatars(usersData);

      setScreen('user-select');
    } catch (e: any) {
      setConnectError(e?.message ?? 'Cannot connect to BEDM daemon');
      setScreen('error');
    }
  };

  const loadAvatars = async (usersData: UserInfo[]) => {
    for (const user of usersData) {
      if (user.icon_path) {
        try {
          const data = await BedmBridge.readUserAvatar(user.icon_path);
          if (data) {
            setAvatars(prev => ({ ...prev, [user.username]: data }));
          }
        } catch {}
      }
    }
  };

  // ── Navigation ────────────────────────────────────────────────────────────
  const selectUser = useCallback((user: UserInfo) => {
    setSelectedUser(user);
    setPassword('');
    setAuthError(null);
    setAttemptsLeft(5);

    // Use user's last session if available
    if (user.last_session) {
      const found = sessions.find(s => s.id === user.last_session);
      if (found) setSelectedSession(user.last_session);
    }

    setScreen('password');
    setTimeout(() => passwordRef.current?.focus(), 100);
  }, [sessions]);

  const goBack = () => {
    setScreen('user-select');
    setSelectedUser(null);
    setPassword('');
    setAuthError(null);
  };

  // ── Authentication ────────────────────────────────────────────────────────
  const authenticate = async () => {
    if (!selectedUser || !password || isAuthenticating) return;

    setIsAuthenticating(true);
    setAuthError(null);

    try {
      const result = await BedmBridge.authenticate(selectedUser.username, password);

      if (result.success) {
        setPassword('');
        await launchSession(selectedUser.username, selectedSession);
      } else {
        setAttemptsLeft(result.attempts_left);
        setAuthError(result.error ?? 'Incorrect password');
        setPassword('');
        setShake(true);
        setTimeout(() => setShake(false), 500);
        setTimeout(() => passwordRef.current?.focus(), 100);
      }
    } catch (e: any) {
      setAuthError(e?.message ?? 'Authentication error');
      setShake(true);
      setTimeout(() => setShake(false), 500);
    } finally {
      setIsAuthenticating(false);
    }
  };

  const launchSession = async (username: string, sessionId: string) => {
    setScreen('logging-in');
    try {
      await BedmBridge.startSession(username, sessionId);
      // Session started — greeter will be replaced
    } catch (e: any) {
      setAuthError(e?.message ?? 'Session failed to start');
      setScreen('password');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') authenticate();
    if (e.key === 'Escape') goBack();
  };

  // ── Power ─────────────────────────────────────────────────────────────────
  const handlePower = async (action: PowerAction) => {
    setShowPowerMenu(false);
    await BedmBridge.powerAction(action);
  };

  // ── UI helpers ─────────────────────────────────────────────────────────────
  const currentSession = sessions.find(s => s.id === selectedSession);

  const formatUptime = (secs: number) => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  };

  // ── Render ────────────────────────────────────────────────────────────────
  return (
    <div className="relative w-screen h-screen overflow-hidden">
      <Background wallpaper={wallpaper} />

      {/* System info bar — top right */}
      <div className="absolute top-5 right-6 flex items-center gap-4 z-10">
        {daemon && (
          <span
            className="text-xs text-slate-500"
            style={{ fontFamily: '"JetBrains Mono", monospace' }}
          >
            {daemon.hostname} · {daemon.os_name}
          </span>
        )}
        <Wifi size={14} className="text-slate-600" />
        <Volume2 size={14} className="text-slate-600" />
      </div>

      {/* BEDM brand — top left */}
      <div className="absolute top-5 left-6 flex items-center gap-2 z-10">
        <div
          className="w-7 h-7 rounded-lg flex items-center justify-center"
          style={{
            background: 'linear-gradient(135deg, #1d4ed8, #7c3aed)',
            boxShadow: '0 0 12px rgba(59,130,246,0.4)',
          }}
        >
          <Shield size={14} className="text-white" />
        </div>
        <span
          className="text-slate-500 text-xs tracking-widest uppercase"
          style={{ fontFamily: '"Oxanium", monospace', letterSpacing: '0.15em' }}
        >
          BEDM
        </span>
        {daemon && (
          <span className="text-slate-700 text-xs" style={{ fontFamily: '"JetBrains Mono", monospace' }}>
            v{daemon.version}
          </span>
        )}
      </div>

      {/* Power button — bottom right */}
      <button
        onClick={() => setShowPowerMenu(true)}
        className="absolute bottom-6 right-6 z-10 p-3 rounded-xl bedm-btn-ghost group"
        title="Power Options"
      >
        <Power size={18} className="text-slate-500 group-hover:text-red-400 transition-colors" />
      </button>

      {/* Uptime — bottom left */}
      {daemon && (
        <div className="absolute bottom-6 left-6 z-10">
          <span
            className="text-slate-700 text-xs"
            style={{ fontFamily: '"JetBrains Mono", monospace' }}
          >
            up {formatUptime(daemon.uptime)}
          </span>
        </div>
      )}

      {/* ── Main content ── */}
      <div className="absolute inset-0 flex flex-col items-center justify-center z-10 gap-12">

        {/* Clock — always visible except logging-in */}
        {screen !== 'logging-in' && screen !== 'connecting' && screen !== 'error' && (
          <Clock className="animate-fade-in" />
        )}

        {/* ── Connecting ── */}
        {screen === 'connecting' && (
          <div className="flex flex-col items-center gap-6 animate-fade-in">
            <div
              className="w-20 h-20 rounded-2xl flex items-center justify-center"
              style={{
                background: 'linear-gradient(135deg, #1d4ed8, #7c3aed)',
                boxShadow: '0 0 40px rgba(59,130,246,0.4)',
              }}
            >
              <Shield size={36} className="text-white" />
            </div>
            <div className="text-center">
              <div
                className="text-white text-2xl mb-1"
                style={{ fontFamily: '"Oxanium", monospace', fontWeight: 400 }}
              >
                Blue Environment
              </div>
              <div className="text-slate-500 text-sm flex items-center gap-2 justify-center">
                <Loader2 size={14} className="animate-spin" />
                Connecting to display manager…
              </div>
            </div>
          </div>
        )}

        {/* ── Error ── */}
        {screen === 'error' && (
          <div className="flex flex-col items-center gap-6 animate-slide-up">
            <div
              className="w-16 h-16 rounded-2xl flex items-center justify-center"
              style={{ background: 'rgba(239,68,68,0.15)', border: '1px solid rgba(239,68,68,0.3)' }}
            >
              <AlertCircle size={28} className="text-red-400" />
            </div>
            <div className="text-center">
              <div className="text-white text-lg mb-2">Connection Failed</div>
              <div className="text-slate-400 text-sm max-w-xs text-center">
                {connectError}
              </div>
            </div>
            <button onClick={init} className="bedm-btn-primary px-6 py-2.5 rounded-xl text-sm">
              Retry
            </button>
          </div>
        )}

        {/* ── Logging in ── */}
        {screen === 'logging-in' && (
          <div className="flex flex-col items-center gap-8 animate-fade-in">
            <div className="relative">
              <div
                className="w-24 h-24 rounded-full flex items-center justify-center text-3xl font-medium text-white"
                style={{
                  background: 'linear-gradient(135deg, #1d4ed8, #7c3aed)',
                  boxShadow: '0 0 60px rgba(59,130,246,0.5)',
                  fontFamily: '"DM Sans", sans-serif',
                }}
              >
                {avatars[selectedUser?.username ?? '']
                  ? <img src={avatars[selectedUser!.username]} className="w-full h-full rounded-full object-cover" alt="" />
                  : (selectedUser?.realname.charAt(0) ?? '?').toUpperCase()
                }
              </div>
              <div
                className="absolute inset-0 rounded-full animate-pulse-glow"
                style={{ border: '2px solid rgba(59,130,246,0.5)' }}
              />
            </div>
            <div className="text-center">
              <div className="text-white text-xl mb-1"
                style={{ fontFamily: '"Oxanium", monospace' }}>
                Welcome, {selectedUser?.realname}
              </div>
              <div className="text-slate-400 text-sm flex items-center gap-2 justify-center">
                <Loader2 size={14} className="animate-spin" />
                Starting {currentSession?.name ?? 'session'}…
              </div>
            </div>
          </div>
        )}

        {/* ── User select ── */}
        {screen === 'user-select' && (
          <div className="flex flex-col items-center gap-8 animate-slide-up">
            <div
              className="glass-card rounded-3xl px-10 py-8 flex flex-col items-center gap-6"
              style={{ minWidth: 480 }}
            >
              <div className="text-slate-400 text-sm tracking-widest uppercase"
                style={{ fontFamily: '"Oxanium", monospace', letterSpacing: '0.2em' }}>
                Select User
              </div>

              {users.length === 0 ? (
                <div className="text-slate-500 text-sm py-4">
                  No users found
                </div>
              ) : (
                <div className="flex gap-4 flex-wrap justify-center">
                  {users.map(user => (
                    <UserCard
                      key={user.username}
                      user={user}
                      isSelected={false}
                      avatarData={avatars[user.username]}
                      onClick={() => selectUser(user)}
                    />
                  ))}
                </div>
              )}

              {/* Session selector row */}
              <div className="w-full pt-2 border-t border-white/5">
                <button
                  onClick={() => setShowSessionPicker(!showSessionPicker)}
                  className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bedm-btn-ghost"
                >
                  <div className="flex items-center gap-2 text-sm text-slate-400">
                    <Monitor size={14} />
                    <span>{currentSession?.name ?? 'Select session…'}</span>
                    <span
                      className="text-[10px] px-1.5 py-0.5 rounded"
                      style={{
                        background: currentSession?.session_type === 'wayland'
                          ? 'rgba(59,130,246,0.15)' : 'rgba(249,115,22,0.15)',
                        color: currentSession?.session_type === 'wayland' ? '#93c5fd' : '#fdba74',
                        fontFamily: '"JetBrains Mono", monospace',
                      }}
                    >
                      {currentSession?.session_type === 'wayland' ? 'WL' : 'X11'}
                    </span>
                  </div>
                  <ChevronDown
                    size={14}
                    className={`text-slate-600 transition-transform ${showSessionPicker ? 'rotate-180' : ''}`}
                  />
                </button>
                {showSessionPicker && (
                  <div className="mt-2 animate-slide-down">
                    <SessionPicker
                      sessions={sessions}
                      selected={selectedSession}
                      onSelect={(id) => { setSelectedSession(id); setShowSessionPicker(false); }}
                    />
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* ── Password ── */}
        {screen === 'password' && selectedUser && (
          <div className="flex flex-col items-center gap-8 animate-slide-up">
            <div
              className="glass-card rounded-3xl px-10 py-8 flex flex-col items-center gap-6 w-96"
            >
              {/* User identity */}
              <div className="flex flex-col items-center gap-3">
                <div
                  className="w-20 h-20 rounded-full flex items-center justify-center text-2xl font-medium text-white"
                  style={{
                    background: avatars[selectedUser.username]
                      ? 'transparent'
                      : 'linear-gradient(135deg, #1d4ed8, #7c3aed)',
                    boxShadow: '0 0 0 3px rgba(59,130,246,0.3), 0 4px 20px rgba(0,0,0,0.4)',
                    fontFamily: '"DM Sans", sans-serif',
                  }}
                >
                  {avatars[selectedUser.username]
                    ? <img src={avatars[selectedUser.username]} className="w-full h-full rounded-full object-cover" alt="" />
                    : selectedUser.realname.charAt(0).toUpperCase()
                  }
                </div>
                <div className="text-center">
                  <div className="text-white font-medium">{selectedUser.realname}</div>
                  <div className="text-slate-500 text-sm">{selectedUser.username}</div>
                </div>
              </div>

              {/* Password input */}
              <div
                className="w-full space-y-3"
                style={{ animation: shake ? 'shake 0.4s cubic-bezier(0.36,0.07,0.19,0.97)' : 'none' }}
              >
                <div className="relative">
                  <input
                    ref={passwordRef}
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={e => setPassword(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Password"
                    className="bedm-input w-full px-4 py-3 rounded-xl pr-12 text-sm"
                    style={{ fontFamily: '"DM Sans", sans-serif' }}
                    autoFocus
                    disabled={isAuthenticating}
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(s => !s)}
                    className="absolute right-3 top-1/2 -translate-y-1/2 p-1.5 rounded-lg hover:bg-white/5 transition-colors"
                  >
                    {showPassword
                      ? <EyeOff size={15} className="text-slate-600" />
                      : <Eye size={15} className="text-slate-600" />
                    }
                  </button>
                </div>

                {/* Error */}
                {authError && (
                  <div className="flex items-center gap-2 text-red-400 text-sm animate-slide-down">
                    <AlertCircle size={14} />
                    <span>{authError}</span>
                    {attemptsLeft < 5 && (
                      <span className="text-slate-600 text-xs ml-auto">
                        {attemptsLeft} left
                      </span>
                    )}
                  </div>
                )}

                {/* Login button */}
                <button
                  onClick={authenticate}
                  disabled={!password || isAuthenticating}
                  className="bedm-btn-primary w-full py-3 rounded-xl font-medium text-sm flex items-center justify-center gap-2"
                >
                  {isAuthenticating ? (
                    <>
                      <Loader2 size={16} className="animate-spin" />
                      Authenticating…
                    </>
                  ) : (
                    'Log In'
                  )}
                </button>
              </div>

              {/* Session inline selector */}
              <div className="w-full border-t border-white/5 pt-3">
                <button
                  onClick={() => setShowSessionPicker(!showSessionPicker)}
                  className="w-full flex items-center justify-between px-3 py-2 rounded-xl bedm-btn-ghost text-sm"
                >
                  <div className="flex items-center gap-2 text-slate-500">
                    <Monitor size={13} />
                    <span className="text-xs">{currentSession?.name}</span>
                    <span
                      className="text-[10px] px-1.5 py-0.5 rounded"
                      style={{
                        background: currentSession?.session_type === 'wayland'
                          ? 'rgba(59,130,246,0.15)' : 'rgba(249,115,22,0.15)',
                        color: currentSession?.session_type === 'wayland' ? '#93c5fd' : '#fdba74',
                        fontFamily: '"JetBrains Mono", monospace',
                      }}
                    >
                      {currentSession?.session_type === 'wayland' ? 'WL' : 'X11'}
                    </span>
                  </div>
                  <ChevronDown
                    size={12}
                    className={`text-slate-700 transition-transform ${showSessionPicker ? 'rotate-180' : ''}`}
                  />
                </button>
                {showSessionPicker && (
                  <div className="mt-1 animate-slide-down">
                    <SessionPicker
                      sessions={sessions}
                      selected={selectedSession}
                      onSelect={(id) => { setSelectedSession(id); setShowSessionPicker(false); }}
                    />
                  </div>
                )}
              </div>
            </div>

            {/* Back button */}
            {users.length > 1 && (
              <button
                onClick={goBack}
                className="flex items-center gap-2 text-slate-500 hover:text-slate-300 text-sm transition-colors"
              >
                <ChevronLeft size={16} />
                Switch User
              </button>
            )}
          </div>
        )}
      </div>

      {/* Power menu overlay */}
      {showPowerMenu && (
        <PowerMenu
          onAction={handlePower}
          onClose={() => setShowPowerMenu(false)}
        />
      )}
    </div>
  );
}

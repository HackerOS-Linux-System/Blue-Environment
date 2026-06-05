import React from 'react';
import { Monitor, Globe } from 'lucide-react';
import type { SessionInfo } from '../types';

interface SessionPickerProps {
  sessions: SessionInfo[];
  selected: string;
  onSelect: (id: string) => void;
}

export const SessionPicker: React.FC<SessionPickerProps> = ({
  sessions,
  selected,
  onSelect,
}) => {
  return (
    <div className="space-y-1.5 max-h-64 overflow-y-auto pr-1">
      {sessions.map(session => {
        const isSelected = session.id === selected;
        const isWayland = session.session_type === 'wayland';

        return (
          <button
            key={session.id}
            onClick={() => onSelect(session.id)}
            className="w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-200 text-left group"
            style={{
              background: isSelected
                ? 'rgba(37,99,235,0.15)'
                : 'rgba(8,20,45,0.5)',
              border: isSelected
                ? '1px solid rgba(59,130,246,0.4)'
                : '1px solid rgba(255,255,255,0.05)',
            }}
          >
            {/* Session type icon */}
            <div
              className="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
              style={{
                background: isSelected
                  ? 'rgba(59,130,246,0.2)'
                  : 'rgba(255,255,255,0.05)',
              }}
            >
              {isWayland ? (
                <Monitor
                  size={16}
                  style={{ color: isSelected ? '#60a5fa' : '#475569' }}
                />
              ) : (
                <Globe
                  size={16}
                  style={{ color: isSelected ? '#60a5fa' : '#475569' }}
                />
              )}
            </div>

            {/* Session info */}
            <div className="flex-1 min-w-0">
              <div
                className="text-sm font-medium truncate"
                style={{ color: isSelected ? '#e2e8f0' : '#94a3b8' }}
              >
                {session.name}
              </div>
              {session.comment && (
                <div className="text-xs truncate mt-0.5" style={{ color: '#475569' }}>
                  {session.comment}
                </div>
              )}
            </div>

            {/* Type badge */}
            <span
              className="text-[10px] px-1.5 py-0.5 rounded shrink-0"
              style={{
                background: isWayland
                  ? 'rgba(59,130,246,0.15)'
                  : 'rgba(249,115,22,0.15)',
                color: isWayland ? '#93c5fd' : '#fdba74',
                border: isWayland
                  ? '1px solid rgba(59,130,246,0.2)'
                  : '1px solid rgba(249,115,22,0.2)',
                fontFamily: '"JetBrains Mono", monospace',
              }}
            >
              {isWayland ? 'WL' : 'X11'}
            </span>

            {/* Check mark */}
            {isSelected && (
              <div
                className="w-5 h-5 rounded-full flex items-center justify-center shrink-0"
                style={{ background: '#3b82f6' }}
              >
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                  <path
                    d="M2 5L4 7L8 3"
                    stroke="white"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </div>
            )}
          </button>
        );
      })}
    </div>
  );
};

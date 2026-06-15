import React, { useState, useEffect } from 'react';

interface ClockProps {
  className?: string;
}

export const Clock: React.FC<ClockProps> = ({ className = '' }) => {
  const [time, setTime] = useState<string>('');
  const [date, setDate] = useState<string>('');

  useEffect(() => {
    const update = () => {
      const now = new Date();
      const h = now.getHours().toString().padStart(2, '0');
      const m = now.getMinutes().toString().padStart(2, '0');
      setTime(`${h}:${m}`);

      const opts: Intl.DateTimeFormatOptions = {
        weekday: 'long',
        month: 'long',
        day: 'numeric',
        year: 'numeric',
      };
      setDate(now.toLocaleDateString('en-US', opts));
    };

    update();
    const id = setInterval(update, 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className={`text-center ${className}`}>
      <div
        className="tabular-nums leading-none text-white"
        style={{
          fontFamily: '"Oxanium", monospace',
          fontSize: 'clamp(72px, 10vw, 120px)',
          fontWeight: 300,
          letterSpacing: '-0.02em',
          textShadow: '0 0 60px rgba(59,130,246,0.3), 0 2px 4px rgba(0,0,0,0.5)',
        }}
      >
        {time}
      </div>
      <div
        className="mt-2 text-slate-400 tracking-widest uppercase"
        style={{
          fontFamily: '"DM Sans", sans-serif',
          fontSize: '0.875rem',
          fontWeight: 300,
          letterSpacing: '0.15em',
        }}
      >
        {date}
      </div>
    </div>
  );
};

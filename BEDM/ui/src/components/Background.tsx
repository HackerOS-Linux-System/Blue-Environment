import React, { useEffect, useRef } from 'react';

interface BackgroundProps {
  wallpaper?: string | null;
}

export const Background: React.FC<BackgroundProps> = ({ wallpaper }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;

    // Particle system
    const particles: Array<{
      x: number; y: number;
      vx: number; vy: number;
      size: number; opacity: number;
      life: number; maxLife: number;
    }> = [];

    const spawnParticle = () => {
      const x = Math.random() * canvas.width;
      const y = Math.random() * canvas.height;
      const maxLife = 120 + Math.random() * 180;
      particles.push({
        x, y,
        vx: (Math.random() - 0.5) * 0.3,
        vy: -Math.random() * 0.4 - 0.1,
        size: Math.random() * 1.5 + 0.5,
        opacity: 0,
        life: 0,
        maxLife,
      });
    };

    // Spawn initial particles
    for (let i = 0; i < 60; i++) spawnParticle();

    let animId: number;
    let frame = 0;

    const draw = () => {
      frame++;
      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // Spawn new particles occasionally
      if (frame % 3 === 0 && particles.length < 80) spawnParticle();

      // Draw particles
      for (let i = particles.length - 1; i >= 0; i--) {
        const p = particles[i];
        p.life++;
        p.x += p.vx;
        p.y += p.vy;

        // Fade in then out
        const progress = p.life / p.maxLife;
        if (progress < 0.2) {
          p.opacity = progress / 0.2;
        } else if (progress > 0.8) {
          p.opacity = (1 - progress) / 0.2;
        } else {
          p.opacity = 1;
        }

        if (p.life >= p.maxLife) {
          particles.splice(i, 1);
          continue;
        }

        ctx.beginPath();
        ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(147, 197, 253, ${p.opacity * 0.5})`;
        ctx.fill();
      }

      // Subtle grid lines
      ctx.strokeStyle = 'rgba(59, 130, 246, 0.03)';
      ctx.lineWidth = 1;
      const gridSize = 80;
      for (let x = 0; x < canvas.width; x += gridSize) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, canvas.height);
        ctx.stroke();
      }
      for (let y = 0; y < canvas.height; y += gridSize) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(canvas.width, y);
        ctx.stroke();
      }

      animId = requestAnimationFrame(draw);
    };

    draw();

    const handleResize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    window.addEventListener('resize', handleResize);

    return () => {
      cancelAnimationFrame(animId);
      window.removeEventListener('resize', handleResize);
    };
  }, []);

  return (
    <div className="fixed inset-0 overflow-hidden">
      {/* Base dark background */}
      <div className="absolute inset-0 bg-[#020812]" />

      {/* Wallpaper if set */}
      {wallpaper && (
        <div
          className="absolute inset-0 bg-cover bg-center bg-no-repeat"
          style={{
            backgroundImage: `url(${wallpaper})`,
            filter: 'brightness(0.25) saturate(0.6)',
          }}
        />
      )}

      {/* Aurora blobs */}
      <div className="absolute inset-0 overflow-hidden">
        <div
          className="aurora-1 absolute rounded-full"
          style={{
            width: '70vw', height: '70vw',
            left: '-20vw', top: '-20vw',
            background: 'radial-gradient(circle, rgba(37,99,235,0.12) 0%, rgba(29,78,216,0.06) 50%, transparent 70%)',
            filter: 'blur(80px)',
          }}
        />
        <div
          className="aurora-2 absolute rounded-full"
          style={{
            width: '60vw', height: '60vw',
            right: '-15vw', bottom: '-10vw',
            background: 'radial-gradient(circle, rgba(124,58,237,0.1) 0%, rgba(109,40,217,0.05) 50%, transparent 70%)',
            filter: 'blur(80px)',
          }}
        />
        <div
          className="aurora-3 absolute rounded-full"
          style={{
            width: '40vw', height: '40vw',
            left: '30vw', top: '20vh',
            background: 'radial-gradient(circle, rgba(6,182,212,0.06) 0%, transparent 70%)',
            filter: 'blur(60px)',
          }}
        />
      </div>

      {/* Particle canvas */}
      <canvas
        ref={canvasRef}
        className="absolute inset-0 pointer-events-none"
        style={{ opacity: 0.8 }}
      />

      {/* Vignette */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background: 'radial-gradient(ellipse at center, transparent 40%, rgba(2,8,18,0.7) 100%)',
        }}
      />
    </div>
  );
};

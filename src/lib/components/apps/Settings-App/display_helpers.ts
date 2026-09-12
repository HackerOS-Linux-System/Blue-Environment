import { SystemBridge, shellQuote } from '../../../utils/systemBridge';

function out(r: any): string {
  return typeof r === 'string' ? r : (r?.stdout ?? '') + (r?.stderr ?? '');
}

export async function applyResolution(res: string): Promise<void> {
  const session = await SystemBridge.getSessionType();
  const [w, h] = res.split('x');
  if (session.startsWith('wayland')) {
    const r1 = await SystemBridge.executeCommand(
      `wlr-randr --output "$(wlr-randr 2>/dev/null | grep -v '^\\s' | head -1 | cut -d' ' -f1)" --mode ${w}x${h} 2>&1`
    );
    if (out(r1).includes('error')) await applyXrandrMode(w, h);
  } else {
    await applyXrandrMode(w, h);
  }
}

async function applyXrandrMode(w: string, h: string): Promise<void> {
  const o = await SystemBridge.executeCommand(`xrandr | grep ' connected' | head -1 | cut -d' ' -f1`);
  const mon = out(o).trim();
  if (mon) await SystemBridge.executeCommand(`xrandr --output ${shellQuote(mon)} --mode ${w}x${h} 2>&1`);
}

export async function applyRefreshRate(rate: number): Promise<void> {
  const session = await SystemBridge.getSessionType();
  if (session.startsWith('wayland')) {
    const r1 = await SystemBridge.executeCommand(
      `wlr-randr --output "$(wlr-randr 2>/dev/null | grep -v '^\\s' | head -1 | cut -d' ' -f1)" --rate ${rate} 2>&1`
    );
    if (out(r1).includes('error')) await applyXrandrRate(rate);
  } else {
    await applyXrandrRate(rate);
  }
}

async function applyXrandrRate(rate: number): Promise<void> {
  const o = await SystemBridge.executeCommand(`xrandr | grep ' connected' | head -1 | cut -d' ' -f1`);
  const mon = out(o).trim();
  if (mon) await SystemBridge.executeCommand(`xrandr --output ${shellQuote(mon)} --rate ${rate} 2>&1`);
}

export async function getAvailableModes(): Promise<{ resolution: string; rates: number[] }[]> {
  const result = await SystemBridge.executeCommand(
    `xrandr | awk '/connected/{found=1;next} found && /^[[:space:]]/{print $1,$2;next} /connected/{found=0}'`
  );
  const modes: { resolution: string; rates: number[] }[] = [];
  const seen = new Set<string>();
  for (const line of out(result).split('\n')) {
    const parts = line.trim().split(/\s+/);
    if (!parts[0]?.includes('x')) continue;
    const res = parts[0];
    if (seen.has(res)) continue;
    seen.add(res);
    const rates = parts.slice(1)
      .map((r: string) => parseFloat(r.replace('*', '').replace('+', '')))
      .filter((r: number) => !isNaN(r))
      .map((r: number) => Math.round(r));
    modes.push({ resolution: res, rates: rates.length > 0 ? rates : [60] });
  }
  return modes.length > 0 ? modes : [
    { resolution: '1920x1080', rates: [60, 120] },
    { resolution: '2560x1440', rates: [60, 144] },
    { resolution: '3840x2160', rates: [30, 60] },
    { resolution: '1366x768', rates: [60] },
  ];
}

export interface GeoCoords { lat: number; lon: number; }

/** Resolve the browser's geolocation, falling back to null if denied/unavailable. */
export function getGeoLocation(): Promise<GeoCoords | null> {
  return new Promise((resolve) => {
    if (!('geolocation' in navigator)) { resolve(null); return; }
    navigator.geolocation.getCurrentPosition(
      (pos) => resolve({ lat: pos.coords.latitude, lon: pos.coords.longitude }),
      () => resolve(null),
      { timeout: 4000, maximumAge: 6 * 60 * 60 * 1000 }
    );
  });
}

/**
 * Approximate sunrise/sunset (local 24h "HH:MM") for a given latitude/longitude and date,
 * using the standard NOAA solar-position approximation. Good enough for UI display; the
 * compositor-side daemon (wlsunset/gammastep -l) recomputes precisely every day on its own.
 */
export function computeSunTimes(lat: number, lon: number, date = new Date()): { sunrise: string; sunset: string } {
  const rad = Math.PI / 180;
  const dayOfYear = Math.floor((Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) - Date.UTC(date.getFullYear(), 0, 0)) / 86400000);
  const declination = 23.44 * rad * Math.sin(rad * (360 / 365) * (dayOfYear - 81));
  const latRad = lat * rad;
  const cosHourAngle = -Math.tan(latRad) * Math.tan(declination);
  const clamped = Math.max(-1, Math.min(1, cosHourAngle));
  const hourAngle = Math.acos(clamped) / rad; // degrees
  const solarNoonUtc = 12 - lon / 15;
  const sunriseUtc = solarNoonUtc - hourAngle / 15;
  const sunsetUtc = solarNoonUtc + hourAngle / 15;
  const fmt = (h: number) => {
    const norm = ((h % 24) + 24) % 24;
    const hh = Math.floor(norm);
    const mm = Math.round((norm - hh) * 60);
    return `${String(hh).padStart(2, '0')}:${String(mm % 60).padStart(2, '0')}`;
  };
  const offsetH = -date.getTimezoneOffset() / 60;
  return { sunrise: fmt(sunriseUtc + offsetH), sunset: fmt(sunsetUtc + offsetH) };
}

export async function applyNightLight(
  enabled: boolean,
  tempK: number,
  schedule: 'manual' | 'sunset' = 'manual',
  geo?: GeoCoords | null
): Promise<void> {
  await SystemBridge.executeCommand(`pkill -f wlsunset 2>/dev/null; pkill -f gammastep 2>/dev/null; pkill -f redshift 2>/dev/null; true`);
  if (!enabled) {
    const o = await SystemBridge.executeCommand(`xrandr | grep ' connected' | head -1 | cut -d' ' -f1`);
    const mon = out(o).trim();
    if (mon) await SystemBridge.executeCommand(`xrandr --output ${shellQuote(mon)} --gamma 1:1:1 2>/dev/null || true`);
    return;
  }
  const session = await SystemBridge.getSessionType();
  const dayTemp = 6500;
  const nightTemp = tempK;

  if (schedule === 'sunset' && geo) {
    // Location-based mode: hand full solar scheduling to the color-temperature daemon itself
    // (wlsunset -l/-L and gammastep -l both recompute sunrise/sunset every day from lat/lon).
    if (session.startsWith('wayland')) {
      await SystemBridge.executeCommand(`wlsunset -l ${geo.lat} -L ${geo.lon} -T ${dayTemp} -t ${nightTemp} &`);
    } else {
      await SystemBridge.executeCommand(
        `which gammastep >/dev/null 2>&1 && gammastep -l ${geo.lat}:${geo.lon} -t ${dayTemp}:${nightTemp} & || which redshift >/dev/null 2>&1 && redshift -l ${geo.lat}:${geo.lon} -t ${dayTemp}:${nightTemp} & || true`
      );
    }
    return;
  }

  if (schedule === 'sunset' && !geo) {
    // No location permission granted: fall back to a fixed civil-twilight approximation
    // (19:00-07:00) instead of silently doing nothing.
    if (session.startsWith('wayland')) {
      await SystemBridge.executeCommand(`wlsunset -t ${nightTemp} -T ${dayTemp} -S 07:00 -s 19:00 &`);
    } else {
      await SystemBridge.executeCommand(
        `which gammastep >/dev/null 2>&1 && gammastep -O ${nightTemp} & || which redshift >/dev/null 2>&1 && redshift -O ${nightTemp} & || true`
      );
    }
    return;
  }

  // Manual: always-on fixed temperature (previous default behaviour).
  if (session.startsWith('wayland')) {
    await SystemBridge.executeCommand(`wlsunset -T ${nightTemp} -t ${Math.max(1000, nightTemp - 2500)} &`);
  } else {
    await SystemBridge.executeCommand(
      `which gammastep >/dev/null 2>&1 && gammastep -O ${nightTemp} & || which redshift >/dev/null 2>&1 && redshift -O ${nightTemp} & || true`
    );
  }
}

export async function getCurrentResolution(): Promise<string> {
  const r = await SystemBridge.executeCommand(`xrandr | grep '\\*' | head -1 | awk '{print $1}'`);
  return out(r).trim() || '1920x1080';
}

export async function getCurrentRefreshRate(): Promise<number> {
  const r = await SystemBridge.executeCommand(`xrandr | grep '\\*' | head -1 | grep -oE '[0-9]+\\.[0-9]+\\*' | head -1`);
  const rate = parseFloat(out(r).trim());
  return isNaN(rate) ? 60 : Math.round(rate);
}

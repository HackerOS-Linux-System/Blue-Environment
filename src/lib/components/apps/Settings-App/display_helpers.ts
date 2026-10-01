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

/**
 * Night Light.
 *
 * Fixes for "brightness/colour changes I never asked for":
 *  - MANUAL mode used to start `wlsunset -T <night> -t <night-2500>` with no
 *    schedule/location, i.e. wlsunset's *automatic* day/night mode — the
 *    screen kept drifting between two temperatures on its own. Manual now
 *    means one fixed temperature: gammastep/redshift `-O`, or wlsunset forced
 *    to its "low temperature" mode with SIGUSR1.
 *  - `pkill -f wlsunset` matched the very `sh -c` running this command line
 *    (its own command line contains "wlsunset"), killing it before the new
 *    daemon started. Now `pkill -x` (exact process name).
 *  - X11 branches chained `cmd & || cmd & || true`, which is a shell syntax
 *    error — nothing ever started on X11. Now proper if/elif chains.
 */
const KILL_DAEMONS = `pkill -x wlsunset 2>/dev/null; pkill -x gammastep 2>/dev/null; pkill -x redshift 2>/dev/null; true`;

function constantTempCmd(tempK: number): string {
  const t = Math.round(tempK);
  return [
    `if command -v gammastep >/dev/null 2>&1; then nohup gammastep -O ${t} >/dev/null 2>&1 &`,
    `elif command -v wlsunset >/dev/null 2>&1 && [ -n "$WAYLAND_DISPLAY" ]; then`,
    `  nohup wlsunset -t ${t} -T 6500 -S 06:00 -s 18:00 >/dev/null 2>&1 &`,
    // wlsunset starts in automatic mode; two SIGUSR1s cycle auto → forced high → forced LOW (= fixed temperature).
    `  sleep 1; pkill -USR1 -x wlsunset; sleep 0.2; pkill -USR1 -x wlsunset`,
    `elif command -v redshift >/dev/null 2>&1; then nohup redshift -O ${t} >/dev/null 2>&1 &`,
    `fi; true`,
  ].join('\n');
}

function scheduledCmd(dayK: number, nightK: number, geo: GeoCoords | null | undefined): string {
  const loc = geo ? `-l ${geo.lat} -L ${geo.lon}` : `-S 07:00 -s 19:00`;
  const gLoc = geo ? `-l ${geo.lat}:${geo.lon}` : `-l 52.0:19.0`;
  return [
    `if command -v wlsunset >/dev/null 2>&1 && [ -n "$WAYLAND_DISPLAY" ]; then nohup wlsunset ${loc} -T ${dayK} -t ${nightK} >/dev/null 2>&1 &`,
    `elif command -v gammastep >/dev/null 2>&1; then nohup gammastep ${gLoc} -t ${dayK}:${nightK} >/dev/null 2>&1 &`,
    `elif command -v redshift >/dev/null 2>&1; then nohup redshift ${gLoc} -t ${dayK}:${nightK} >/dev/null 2>&1 &`,
    `fi; true`,
  ].join('\n');
}

export async function applyNightLight(
  enabled: boolean,
  tempK: number,
  schedule: 'manual' | 'sunset' = 'manual',
  geo?: GeoCoords | null
): Promise<void> {
  await SystemBridge.executeCommand(KILL_DAEMONS);
  if (!enabled) {
    const o = await SystemBridge.executeCommand(`xrandr 2>/dev/null | grep ' connected' | head -1 | cut -d' ' -f1`);
    const mon = out(o).trim();
    if (mon) await SystemBridge.executeCommand(`xrandr --output ${shellQuote(mon)} --gamma 1:1:1 2>/dev/null || true`);
    return;
  }
  const dayTemp = 6500;
  const nightTemp = Math.min(tempK, dayTemp - 100);

  if (schedule === 'sunset') {
    // Location-based (or fixed 19:00-07:00 when no location is available):
    // the ONLY mode that changes by itself over the day — by design.
    await SystemBridge.executeCommand(scheduledCmd(dayTemp, nightTemp, geo));
    return;
  }

  // Manual: one fixed temperature, never changes on its own.
  await SystemBridge.executeCommand(constantTempCmd(tempK));
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

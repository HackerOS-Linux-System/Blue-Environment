import { SystemBridge } from '../../../utils/systemBridge';

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
  if (mon) await SystemBridge.executeCommand(`xrandr --output "${mon}" --mode ${w}x${h} 2>&1`);
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
  if (mon) await SystemBridge.executeCommand(`xrandr --output "${mon}" --rate ${rate} 2>&1`);
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

export async function applyNightLight(enabled: boolean, tempK: number): Promise<void> {
  await SystemBridge.executeCommand(`pkill -f wlsunset 2>/dev/null; pkill -f gammastep 2>/dev/null; pkill -f redshift 2>/dev/null; true`);
  if (!enabled) {
    const o = await SystemBridge.executeCommand(`xrandr | grep ' connected' | head -1 | cut -d' ' -f1`);
    const mon = out(o).trim();
    if (mon) await SystemBridge.executeCommand(`xrandr --output "${mon}" --gamma 1:1:1 2>/dev/null || true`);
    return;
  }
  const session = await SystemBridge.getSessionType();
  if (session.startsWith('wayland')) {
    await SystemBridge.executeCommand(`wlsunset -T ${tempK} -t ${Math.max(1000, tempK - 2500)} &`);
  } else {
    await SystemBridge.executeCommand(
      `which gammastep >/dev/null 2>&1 && gammastep -O ${tempK} & || which redshift >/dev/null 2>&1 && redshift -O ${tempK} & || true`
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

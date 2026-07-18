<script lang="ts">
  import { SystemBridge } from '../../../utils/systemBridge';

  export let videoPath: string | undefined;
  export let currentTime: number;

  interface SubLine { start: number; end: number; text: string; }

  function parseSrt(raw: string): SubLine[] {
    const lines: SubLine[] = [];
    const blocks = raw.replace(/\r\n/g, '\n').split(/\n\n+/);
    for (const block of blocks) {
      const rows = block.trim().split('\n');
      if (rows.length < 2) continue;
      const timeRow = rows.find((r) => r.includes('-->'));
      if (!timeRow) continue;
      const [startStr, endStr] = timeRow.split('-->').map((s) => s.trim());
      const toSec = (ts: string) => {
        const [h, m, rest] = ts.split(':');
        const [s, ms] = rest.replace(',', '.').split('.');
        return parseInt(h) * 3600 + parseInt(m) * 60 + parseInt(s) + parseFloat('0.' + (ms ?? '0'));
      };
      const text = rows.slice(rows.indexOf(timeRow) + 1).join('\n').replace(/<[^>]+>/g, '').trim();
      if (text) lines.push({ start: toSec(startStr), end: toSec(endStr), text });
    }
    return lines;
  }

  let lines: SubLine[] = [];

  $: if (videoPath) loadSubs(videoPath); else lines = [];

  async function loadSubs(path: string) {
    const base = path.replace(/\.[^.]+$/, '');
    const candidates = [base + '.srt', base + '.en.srt', base + '.vtt'];
    for (const p of candidates) {
      try {
        const content = await SystemBridge.readFile(p);
        if (content) { lines = parseSrt(content); return; }
      } catch {}
    }
    lines = [];
  }

  $: active = lines.find((l) => currentTime >= l.start && currentTime <= l.end)?.text ?? '';
</script>

{#if active}
  <div style="position:absolute; bottom:4rem; left:0; right:0; display:flex; justify-content:center; pointer-events:none; z-index:10;">
    <div style="background:rgba(0,0,0,0.75); color:#ffffff; padding:0.3rem 0.75rem; border-radius:0.4rem;
      font-size:clamp(0.85rem,1.5vw,1.2rem); font-family:'DM Sans',sans-serif; max-width:80%; text-align:center;
      text-shadow:0 1px 3px rgba(0,0,0,0.8); white-space:pre-line;">
      {active}
    </div>
  </div>
{/if}

<script lang="ts">
  import { tick } from 'svelte';
  import { createEventDispatcher } from 'svelte';

  interface Props {
    content: string;
  }

  let { content }: Props = $props();

  const dispatch = createEventDispatcher<{ change: string }>();

  const DEFAULT_ROWS = 30;
  const DEFAULT_COLS = 12;

  type CellValue = { raw: string; formula?: string };
  type Grid = CellValue[][];

  function makeGrid(rows = DEFAULT_ROWS, cols = DEFAULT_COLS): Grid {
    return Array.from({ length: rows }, () => Array.from({ length: cols }, () => ({ raw: '' })));
  }
  function colLabel(n: number): string {
    let s = '';
    while (n >= 0) { s = String.fromCharCode(65 + (n % 26)) + s; n = Math.floor(n / 26) - 1; }
    return s;
  }
  function resolveRef(ref: string, grid: Grid): number {
    const m = ref.match(/^([A-Z]+)(\d+)$/);
    if (!m) return NaN;
    const col = m[1].split('').reduce((a, c) => a * 26 + c.charCodeAt(0) - 64, 0) - 1;
    const row = parseInt(m[2]) - 1;
    return parseFloat(grid[row]?.[col]?.raw ?? 'NaN');
  }
  function rangeValues(range: string, grid: Grid): number[] {
    const [start, end] = range.split(':');
    const ms = start.match(/^([A-Z]+)(\d+)$/), me = end.match(/^([A-Z]+)(\d+)$/);
    if (!ms || !me) return [];
    const c1 = ms[1].split('').reduce((a, c) => a * 26 + c.charCodeAt(0) - 64, 0) - 1;
    const r1 = parseInt(ms[2]) - 1;
    const c2 = me[1].split('').reduce((a, c) => a * 26 + c.charCodeAt(0) - 64, 0) - 1;
    const r2 = parseInt(me[2]) - 1;
    const vals: number[] = [];
    for (let r = r1; r <= r2; r++) for (let c = c1; c <= c2; c++) { const v = parseFloat(grid[r]?.[c]?.raw ?? 'NaN'); if (!isNaN(v)) vals.push(v); }
    return vals;
  }
  function evalFormula(formula: string, grid: Grid): string {
    const f = formula.slice(1).trim().toUpperCase();
    try {
      for (const fn of ['SUM', 'AVERAGE', 'MAX', 'MIN', 'COUNT']) {
        const m = f.match(new RegExp(`^${fn}\\(([^)]+)\\)$`));
        if (m) {
          const vals = m[1].includes(':') ? rangeValues(m[1], grid) : m[1].split(',').map((s) => resolveRef(s.trim(), grid));
          if (fn === 'SUM') return String(vals.reduce((a, v) => a + v, 0));
          if (fn === 'AVERAGE') return String(vals.reduce((a, v) => a + v, 0) / vals.length);
          if (fn === 'MAX') return String(Math.max(...vals));
          if (fn === 'MIN') return String(Math.min(...vals));
          if (fn === 'COUNT') return String(vals.filter((v) => !isNaN(v)).length);
        }
      }
      const ifm = f.match(/^IF\((.+),(.+),(.+)\)$/);
      if (ifm) {
        const cond = ifm[1].trim();
        const m2 = cond.match(/^([A-Z]+\d+)\s*([><=!]+)\s*([A-Z]+\d+|-?\d+\.?\d*)$/);
        if (m2) {
          const lhs = resolveRef(m2[1], grid);
          const rhs = /[A-Z]/.test(m2[3]) ? resolveRef(m2[3], grid) : parseFloat(m2[3]);
          const ops: Record<string, boolean> = { '>': lhs > rhs, '<': lhs < rhs, '=': lhs === rhs, '>=': lhs >= rhs, '<=': lhs <= rhs, '<>': lhs !== rhs };
          return ops[m2[2]] ? ifm[2].trim().replace(/"/g, '') : ifm[3].trim().replace(/"/g, '');
        }
      }
      const expr = f.replace(/[A-Z]+\d+/g, (r) => String(resolveRef(r, grid)));
      // eslint-disable-next-line no-new-func
      const result = Function(`"use strict"; return (${expr})`)();
      return isNaN(result) ? '#VALUE' : String(result);
    } catch { return '#ERR'; }
  }
  function cellDisplay(cell: CellValue, grid: Grid): string {
    if (!cell.raw) return '';
    if (cell.raw.startsWith('=')) return evalFormula(cell.raw, grid);
    return cell.raw;
  }

  interface Sel { r: number; c: number; }

  let grid = $state<Grid>((() => { try { return JSON.parse(content) as Grid; } catch { return makeGrid(); } })());
  let sel = $state<Sel>({ r: 0, c: 0 });
  let editing = $state(false);
  let editVal = $state('');
  const colWidths = Array(DEFAULT_COLS).fill(100);
  let inputEl = $state() as HTMLInputElement;

  let rows = $derived(grid.length || DEFAULT_ROWS);
  let cols = $derived(grid[0]?.length || DEFAULT_COLS);

  function commit(r: number, c: number, val: string) {
    const next = grid.map((row) => [...row]);
    while (next.length <= r) next.push(Array(cols).fill({ raw: '' }));
    while (next[r].length <= c) next[r].push({ raw: '' });
    next[r][c] = { raw: val };
    grid = next;
    dispatch('change', JSON.stringify(next));
  }

  async function startEdit(r: number, c: number) {
    sel = { r, c };
    editVal = grid[r]?.[c]?.raw ?? '';
    editing = true;
    await tick();
    inputEl?.focus();
  }
  function finishEdit() { commit(sel.r, sel.c, editVal); editing = false; }

  function handleKey(e: KeyboardEvent) {
    if (editing) {
      if (e.key === 'Enter') { finishEdit(); sel = { ...sel, r: sel.r + 1 }; }
      if (e.key === 'Tab') { e.preventDefault(); finishEdit(); sel = { ...sel, c: sel.c + 1 }; }
      if (e.key === 'Escape') editing = false;
      return;
    }
    switch (e.key) {
      case 'ArrowUp': sel = { ...sel, r: Math.max(0, sel.r - 1) }; break;
      case 'ArrowDown': sel = { ...sel, r: Math.min(rows - 1, sel.r + 1) }; break;
      case 'ArrowLeft': sel = { ...sel, c: Math.max(0, sel.c - 1) }; break;
      case 'ArrowRight': sel = { ...sel, c: Math.min(cols - 1, sel.c + 1) }; break;
      case 'Enter': case 'F2': startEdit(sel.r, sel.c); break;
      case 'Delete': case 'Backspace': commit(sel.r, sel.c, ''); break;
      default:
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) { editVal = e.key; startEdit(sel.r, sel.c); }
    }
  }
</script>

<div class="flex-1 flex flex-col overflow-hidden" onkeydown={handleKey} tabindex="0" style="outline:none;">
  <div class="shrink-0 flex items-center gap-2 px-3 py-1.5 border-b border-white/5 bg-slate-900">
    <span class="text-slate-500 text-xs font-mono w-10 shrink-0">{colLabel(sel.c)}{sel.r + 1}</span>
    <div class="w-px h-4 bg-white/10"></div>
    <span class="text-slate-400 text-sm font-mono flex-1">{editing ? editVal : grid[sel.r]?.[sel.c]?.raw ?? ''}</span>
  </div>

  <div class="flex-1 overflow-auto">
    <table class="border-collapse" style="font-size:13px; font-family:monospace;">
      <thead>
        <tr>
          <th class="shrink-0 sticky top-0 left-0 z-20 w-10 h-7 bg-slate-800 border-b border-r border-white/10"></th>
          {#each Array.from({ length: cols }, (_, c) => c) as c (c)}
            <th style="width:{colWidths[c]}px; min-width:{colWidths[c]}px;" class="sticky top-0 z-10 h-7 bg-slate-800 border-b border-r border-white/10 text-slate-400 text-center text-xs select-none">{colLabel(c)}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each Array.from({ length: rows }, (_, r) => r) as r (r)}
          <tr>
            <td class="sticky left-0 z-10 w-10 h-7 bg-slate-800 border-b border-r border-white/10 text-slate-400 text-center text-xs select-none">{r + 1}</td>
            {#each Array.from({ length: cols }, (_, c) => c) as c (c)}
              {@const isSelected = sel.r === r && sel.c === c}
              {@const cell = grid[r]?.[c] ?? { raw: '' }}
              {@const display = cellDisplay(cell, grid)}
              <td style="width:{colWidths[c]}px; min-width:{colWidths[c]}px;"
                class="h-7 border-b border-r border-white/5 text-xs px-1.5 cursor-cell relative {isSelected ? 'bg-blue-900/40 ring-2 ring-inset ring-blue-500 z-10' : 'hover:bg-white/5'}"
                onclick={() => (sel = { r, c })} ondblclick={() => startEdit(r, c)}>
                {#if isSelected && editing}
                  <input bind:this={inputEl} bind:value={editVal} onblur={finishEdit}
                    onkeydown={(e) => { if (e.key === 'Enter') { e.stopPropagation(); finishEdit(); sel = { ...sel, r: sel.r + 1 }; } if (e.key === 'Escape') { e.stopPropagation(); editing = false; } }}
                    class="absolute inset-0 w-full h-full bg-blue-900 text-white px-1.5 text-xs focus:outline-none z-20 font-mono" />
                {:else}
                  <span class="text-slate-200 select-none">{display}</span>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

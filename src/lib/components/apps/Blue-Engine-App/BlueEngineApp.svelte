<script lang="ts">
  import { run as run_1, stopPropagation } from 'svelte/legacy';

  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Play, Square, Hammer, Plus, Trash2, Save, FolderOpen, Sparkles, Box } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import { pickPaths } from '../../../stores/filePicker';
  import {
    NODE_CATALOG, NODE_W, newProject, addNode, connect, removeNode, pinPos, parseAiBlueprint, AI_SYSTEM_PROMPT,
    type Project, type BpNode, type Entity,
  } from './engineModel';

  let project = $state<Project>(newProject());
  let projectDir = $state('');
  let tab = $state<'viewport' | 'blueprint' | 'code'>('viewport');
  const TABS: { id: 'viewport' | 'blueprint' | 'code'; label: string }[] = [
    { id: 'viewport', label: 'Scena' }, { id: 'blueprint', label: 'Blueprint' }, { id: 'code', label: 'Kod' },
  ];
  let selEntity = $state(0);
  let selNode = $state<number | null>(null);
  let linking = $state<{ from: number; pin: string } | null>(null);
  let dragging: { id: number; dx: number; dy: number } | null = null;
  let svgEl = $state() as SVGSVGElement;
  let playSrc = $state('');
  let log = $state<string[]>([]);
  let target = $state<'web' | 'linux' | 'windows'>('web');
  let building = $state(false);
  let lastOutput = $state('');
  let codeFiles = $state<{ path: string; content: string }[]>([]);
  let codeError = $state('');
  let codeSel = $state(0);
  let aiPrompt = $state('');
  let aiBusy = $state(false);
  let unlisten: (() => void) | undefined;

  const say = (s: string) => (log = [...log.slice(-300), s]);

  const onMsg = (e: MessageEvent) => { if (e.data?.type === 'blue-engine-log') say(`▶ ${e.data.text}`); };
  onMount(async () => {
    window.addEventListener('message', onMsg);
    unlisten = await listen<string>('engine:log', (e) => say(e.payload));
    projectDir = `${await SystemBridge.getHomePath().catch(() => '')}/BlueEngineProjects/${project.name}`;
  });
  onDestroy(() => { window.removeEventListener('message', onMsg); unlisten?.(); });

  let ent = $derived(project.entities[selEntity] as Entity | undefined);
  let node = $derived(project.blueprint.nodes.find((n) => n.id === selNode) ?? null);

  // ───────── scena ─────────
  function addEntity() {
    let i = project.entities.length + 1, name = `obiekt${i}`;
    while (project.entities.some((e) => e.name === name)) name = `obiekt${++i}`;
    project.entities = [...project.entities, { name, x: 50, y: 50, w: 40, h: 40, color: '#f59e0b', shape: 'rect' }];
    selEntity = project.entities.length - 1;
  }
  function delEntity() { project.entities = project.entities.filter((_, i) => i !== selEntity); selEntity = Math.max(0, selEntity - 1); }

  // ───────── podgląd (iframe) ─────────
  async function play() {
    try {
      const files = await invoke<{ path: string; content: string }[]>('engine_generate', { project, target: 'web' });
      const html = files.find((f) => f.path === 'index.html')!.content;
      const js = files.find((f) => f.path === 'game.js')!.content;
      // Uwaga: dosłowny zamykający tag skryptu w <script> Svelte urwałby komponent — składamy go z kawałków.
      const OPEN = '<' + 'script', CLOSE = '</' + 'script>';
      const safeJs = js.split(CLOSE).join('<\\/' + 'script>');
      playSrc = html.replace(OPEN + ' src="game.js">' + CLOSE, () => OPEN + '>' + safeJs + CLOSE);
      tab = 'viewport'; say('▶ Uruchomiono podgląd');
    } catch (e) { say(`✖ ${e}`); }
  }
  const stop = () => { playSrc = ''; say('■ Zatrzymano'); };

  // ───────── blueprint ─────────
  function svgPoint(e: PointerEvent) { const r = svgEl.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; }
  function startDrag(e: PointerEvent, n: BpNode) {
    selNode = n.id; const p = svgPoint(e); dragging = { id: n.id, dx: p.x - n.x, dy: p.y - n.y };
    (e.currentTarget as Element).setPointerCapture?.(e.pointerId);
  }
  function moveDrag(e: PointerEvent) {
    if (!dragging) return; const p = svgPoint(e);
    project.blueprint.nodes = project.blueprint.nodes.map((n) => n.id === dragging!.id ? { ...n, x: Math.max(0, p.x - dragging!.dx), y: Math.max(0, p.y - dragging!.dy) } : n);
  }
  const endDrag = () => (dragging = null);
  function outPin(n: BpNode, pin: string) { linking = { from: n.id, pin }; }
  function inPin(n: BpNode) { if (linking) { project = connect(project, linking.from, linking.pin, n.id); linking = null; } }
  const edgePath = (from: number, pin: string, to: number) => {
    const a = project.blueprint.nodes.find((n) => n.id === from), b = project.blueprint.nodes.find((n) => n.id === to);
    if (!a || !b) return '';
    const s = pinPos(a, pin, 'out'), t = pinPos(b, 'exec', 'in');
    const c = Math.max(40, Math.abs(t.x - s.x) / 2);
    return `M${s.x},${s.y} C${s.x + c},${s.y} ${t.x - c},${t.y} ${t.x},${t.y}`;
  };
  const summary = (n: BpNode) => Object.entries(n.params).map(([k, v]) => `${k}=${v}`).join('  ').slice(0, 30);
  const delNode = () => { if (selNode != null) { project = removeNode(project, selNode); selNode = null; } };

  // ───────── kod / build ─────────
  async function refreshCode() {
    codeError = '';
    try { codeFiles = await invoke('engine_generate', { project, target }); codeSel = 0; }
    catch (e) { codeFiles = []; codeError = String(e); }
  }
  run_1(() => {
    if (tab === 'code') { target; project; refreshCode(); }
  });

  async function build() {
    building = true; log = []; say(`▶ Budowanie (${target})…`);
    try {
      const r = await invoke<{ output: string }>('engine_build', { dir: projectDir, project, target });
      lastOutput = r.output; say(`✔ Gotowe: ${r.output}`);
    } catch (e) { say(`✖ ${e}`); }
    building = false;
  }
  const run = () => invoke('engine_run', { output: lastOutput }).catch((e) => say(`✖ ${e}`));
  async function save() { try { await invoke('engine_save_project', { dir: projectDir, project }); say(`💾 Zapisano w ${projectDir}`); } catch (e) { say(`✖ ${e}`); } }
  async function open() {
    const d = (await pickPaths({ mode: 'directory', title: 'Otwórz projekt Blue Engine', multiple: false }))[0];
    if (!d) return;
    try { project = await invoke<Project>('engine_load_project', { dir: d }); projectDir = d; selEntity = 0; selNode = null; say(`📂 Otwarto ${d}`); }
    catch (e) { say(`✖ ${e}`); }
  }

  // ───────── AI ─────────
  async function askAi() {
    aiBusy = true;
    try {
      const cfg: any = await invoke('get_ai_config');
      if (!cfg) throw new Error('Skonfiguruj Blue AI (Ustawienia/Blue AI), aby używać asystenta.');
      const scene = project.entities.map((e) => e.name).join(', ');
      const reply = await invoke<string>('ai_call', { request: {
        service: cfg.service, model: cfg.model, api_key: cfg.api_key ?? null,
        messages: [{ role: 'system', content: AI_SYSTEM_PROMPT }, { role: 'user', content: `Obiekty w scenie: ${scene}.\nOpis: ${aiPrompt}` }],
      } });
      const bp = parseAiBlueprint(reply);
      if (!bp) throw new Error('AI nie zwróciło poprawnego grafu.');
      project = { ...project, blueprint: bp }; tab = 'blueprint'; say(`✨ AI wygenerowało ${bp.nodes.length} węzłów`);
    } catch (e) { say(`✖ ${e}`); }
    aiBusy = false;
  }
</script>

<div class="h-full flex flex-col bg-slate-900 text-slate-200 text-sm">
  <div class="flex items-center gap-2 px-3 py-2 border-b border-white/5">
    <input class="bg-slate-800 rounded-lg px-2 py-1 w-40" bind:value={project.name} />
    <button class="p-1.5 rounded-lg hover:bg-white/10" title="Zapisz" onclick={save}><Save size={15} /></button>
    <button class="p-1.5 rounded-lg hover:bg-white/10" title="Otwórz" onclick={open}><FolderOpen size={15} /></button>
    <div class="flex gap-1 ml-3">
      {#each TABS as t}
        <button class="px-3 py-1 rounded-lg {tab === t.id ? 'bg-blue-600 text-white' : 'hover:bg-white/5'}" onclick={() => (tab = t.id)}>{t.label}</button>
      {/each}
    </div>
    <div class="flex-1"></div>
    {#if playSrc}<button class="flex items-center gap-1 px-3 py-1 rounded-lg bg-red-600 hover:bg-red-500 text-white" onclick={stop}><Square size={13} /> Stop</button>
    {:else}<button class="flex items-center gap-1 px-3 py-1 rounded-lg bg-green-600 hover:bg-green-500 text-white" onclick={play}><Play size={13} /> Graj</button>{/if}
    <select class="bg-slate-800 rounded-lg px-2 py-1" bind:value={target}>
      <option value="web">Web (JS)</option><option value="linux">Linux (Rust)</option><option value="windows">Windows (Rust)</option>
    </select>
    <button class="flex items-center gap-1 px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-40" disabled={building} onclick={build}><Hammer size={13} /> {building ? 'Buduję…' : 'Zbuduj'}</button>
    {#if lastOutput}<button class="px-3 py-1 rounded-lg hover:bg-white/10" onclick={run}>Uruchom wynik</button>{/if}
  </div>

  <div class="flex flex-1 min-h-0">
    <!-- Hierarchia sceny -->
    <div class="w-48 border-r border-white/5 flex flex-col">
      <div class="flex items-center justify-between px-3 py-2 text-xs text-slate-400">Obiekty
        <span><button class="p-1 hover:text-white" onclick={addEntity}><Plus size={14} /></button><button class="p-1 hover:text-red-400" onclick={delEntity}><Trash2 size={13} /></button></span></div>
      <div class="flex-1 overflow-y-auto px-2 space-y-1">
        {#each project.entities as e, i}
          <button class="w-full text-left px-2 py-1 rounded-lg flex items-center gap-2 {selEntity === i ? 'bg-blue-600/40' : 'hover:bg-white/5'}" onclick={() => (selEntity = i)}>
            <Box size={13} style="color:{e.color}" /> {e.name}</button>
        {/each}
      </div>
      <div class="p-2 border-t border-white/5 space-y-1">
        <div class="text-xs text-slate-400 flex items-center gap-1"><Sparkles size={12} /> Asystent AI</div>
        <textarea class="w-full bg-slate-800 rounded-lg p-1.5 text-xs h-16 resize-none outline-none" placeholder="np. gracz porusza się strzałkami, a Spacja zmienia kolor" bind:value={aiPrompt}></textarea>
        <button class="w-full px-2 py-1 rounded-lg bg-purple-600 hover:bg-purple-500 text-white disabled:opacity-40" disabled={aiBusy || !aiPrompt.trim()} onclick={askAi}>{aiBusy ? 'Myślę…' : 'Wygeneruj graf'}</button>
      </div>
    </div>

    <!-- Środek -->
    <div class="flex-1 min-w-0 flex flex-col">
      {#if tab === 'viewport'}
        <div class="flex-1 bg-black flex items-center justify-center overflow-hidden relative">
          {#if playSrc}
            <iframe title="Gra" sandbox="allow-scripts" srcdoc={playSrc} class="w-full h-full border-0"></iframe>
          {:else}
            <div class="relative" style="width:{project.width / 1.6}px; height:{project.height / 1.6}px; background:{project.background}">
              {#each project.entities as e, i}
                <div class="absolute {selEntity === i ? 'ring-2 ring-white/70' : ''}" style="left:{e.x / 1.6}px; top:{e.y / 1.6}px; width:{e.w / 1.6}px; height:{e.h / 1.6}px; background:{e.color}; border-radius:{e.shape === 'circle' ? '50%' : '2px'}"></div>
              {/each}
            </div>
          {/if}
        </div>
      {:else if tab === 'blueprint'}
        <div class="flex-1 flex min-h-0">
          <div class="w-44 border-r border-white/5 overflow-y-auto p-2 space-y-1">
            <div class="text-xs text-slate-400 mb-1">Dodaj węzeł</div>
            {#each Object.entries(NODE_CATALOG) as [kind, spec]}
              <button class="w-full text-left px-2 py-1 rounded-lg text-xs hover:bg-white/10" style="border-left:3px solid {spec.color}"
                onclick={() => (project = addNode(project, kind, 60 + (project.blueprint.nodes.length % 5) * 30, 200 + (project.blueprint.nodes.length % 6) * 30))}>{spec.title}</button>
            {/each}
          </div>
          <div class="flex-1 relative overflow-auto bg-slate-950/60">
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <svg bind:this={svgEl} class="w-[1600px] h-[900px]" onpointermove={moveDrag} onpointerup={endDrag} onclick={() => (linking = null)}>
              {#each project.blueprint.edges as ed}
                <path d={edgePath(ed.from, ed.fromPin, ed.to)} fill="none" stroke={ed.fromPin === 'false' ? '#f87171' : '#94a3b8'} stroke-width="2"
                  class="cursor-pointer hover:stroke-white" onclick={stopPropagation(() => (project.blueprint.edges = project.blueprint.edges.filter((x) => x !== ed)))} />
              {/each}
              {#each project.blueprint.nodes as n (n.id)}
                {@const spec = NODE_CATALOG[n.kind]}
                <g transform="translate({n.x},{n.y})">
                  <rect width={NODE_W} height={Math.max(56, 24 + spec.outs.length * 18 + 16)} rx="8" fill="#1e293b" stroke={selNode === n.id ? '#60a5fa' : '#334155'} stroke-width="2"
                    onpointerdown={stopPropagation((e: any) => startDrag(e, n))} class="cursor-move" />
                  <rect width={NODE_W} height="22" rx="8" fill={spec.color} pointer-events="none" />
                  <text x="8" y="15" font-size="11" fill="white" pointer-events="none">{spec.title}</text>
                  <text x="8" y={36} font-size="10" fill="#94a3b8" pointer-events="none">{summary(n)}</text>
                  {#if !spec.event}<circle cx="0" cy="20" r="6" fill="#0f172a" stroke="#94a3b8" stroke-width="2" class="cursor-pointer" onclick={stopPropagation(() => inPin(n))} />{/if}
                  {#each spec.outs as pin, i}
                    <circle cx={NODE_W} cy={20 + i * 18} r="6" fill={linking?.from === n.id && linking.pin === pin ? '#60a5fa' : '#0f172a'} stroke={pin === 'false' ? '#f87171' : '#94a3b8'} stroke-width="2" class="cursor-pointer"
                      onclick={stopPropagation(() => outPin(n, pin))} />
                    {#if spec.outs.length > 1}<text x={NODE_W - 12} y={24 + i * 18} font-size="9" fill="#94a3b8" text-anchor="end" pointer-events="none">{pin === 'true' ? 'tak' : 'nie'}</text>{/if}
                  {/each}
                </g>
              {/each}
            </svg>
            {#if linking}<div class="absolute top-2 left-2 text-xs bg-blue-600 text-white rounded px-2 py-1">Kliknij wejście (lewy punkt) węzła docelowego</div>{/if}
          </div>
        </div>
      {:else}
        <div class="flex-1 flex flex-col min-h-0">
          {#if codeError}<div class="p-3 text-red-300 text-xs">{codeError}</div>{:else}
            <div class="flex gap-1 p-2 border-b border-white/5">{#each codeFiles as f, i}<button class="px-2 py-1 rounded text-xs {codeSel === i ? 'bg-blue-600 text-white' : 'hover:bg-white/10'}" onclick={() => (codeSel = i)}>{f.path}</button>{/each}</div>
            <pre class="flex-1 overflow-auto p-3 text-[11px] text-slate-300">{codeFiles[codeSel]?.content ?? ''}</pre>
          {/if}
        </div>
      {/if}
      <div class="h-28 border-t border-white/5 bg-black/30 overflow-y-auto p-2 font-mono text-[11px] text-slate-400">
        {#each log as l}<div>{l}</div>{:else}<div class="text-slate-600">Konsola</div>{/each}
      </div>
    </div>

    <!-- Inspektor -->
    <div class="w-60 border-l border-white/5 p-3 space-y-2 overflow-y-auto">
      {#if tab === 'blueprint' && node}
        {@const spec = NODE_CATALOG[node.kind]}
        <div class="font-semibold text-white">{spec.title}</div>
        {#each spec.params as p}
          <label class="block text-xs text-slate-400">{p.label}
            {#if p.key === 'actor'}
              <select class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={node.params[p.key]} onchange={() => (project = project)}>
                {#each project.entities as e}<option value={e.name}>{e.name}</option>{/each}</select>
            {:else}
              <input class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={node.params[p.key]} oninput={() => (project = project)} />
            {/if}
          </label>
        {/each}
        <button class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-red-500/20 text-red-400" onclick={delNode}><Trash2 size={13} /> Usuń węzeł</button>
      {:else if ent}
        <div class="font-semibold text-white">Obiekt</div>
        <label class="block text-xs text-slate-400">Nazwa<input class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.name} oninput={() => (project = project)} /></label>
        <div class="grid grid-cols-2 gap-2 text-xs text-slate-400">
          <label>X<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.x} /></label>
          <label>Y<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.y} /></label>
          <label>Szer.<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.w} /></label>
          <label>Wys.<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.h} /></label>
        </div>
        <label class="flex items-center justify-between text-xs text-slate-400">Kolor<input type="color" bind:value={ent.color} /></label>
        <label class="flex items-center justify-between text-xs text-slate-400">Kształt
          <select class="bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={ent.shape}><option value="rect">Prostokąt</option><option value="circle">Koło</option></select></label>
        <div class="pt-2 border-t border-white/5 space-y-1 text-xs text-slate-400">
          <div class="font-semibold text-white text-sm">Gra</div>
          <div class="grid grid-cols-2 gap-2">
            <label>Szer.<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={project.width} /></label>
            <label>Wys.<input type="number" class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={project.height} /></label>
          </div>
          <label class="flex items-center justify-between">Tło<input type="color" bind:value={project.background} /></label>
          <label class="block">Katalog projektu<input class="w-full bg-slate-800 rounded px-2 py-1 text-slate-200" bind:value={projectDir} /></label>
        </div>
      {/if}
    </div>
  </div>
</div>

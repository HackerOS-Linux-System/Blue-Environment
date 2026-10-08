export interface Entity { name: string; x: number; y: number; w: number; h: number; color: string; shape: 'rect' | 'circle' }
export interface BpNode { id: number; kind: string; params: Record<string, string>; x: number; y: number }
export interface BpEdge { from: number; fromPin: string; to: number }
export interface Project {
  name: string; width: number; height: number; background: string;
  entities: Entity[]; blueprint: { nodes: BpNode[]; edges: BpEdge[] };
}
export interface NodeSpec { title: string; color: string; event?: boolean; outs: string[]; params: { key: string; label: string; def: string }[] }

export const NODE_CATALOG: Record<string, NodeSpec> = {
  event_begin_play: { title: 'Event: Start gry', color: '#b91c1c', event: true, outs: ['exec'], params: [] },
  event_tick: { title: 'Event: Każda klatka', color: '#b91c1c', event: true, outs: ['exec'], params: [] },
  event_key_down: { title: 'Event: Klawisz wciśnięty', color: '#b91c1c', event: true, outs: ['exec'], params: [{ key: 'key', label: 'Klawisz (np. Space)', def: 'Space' }] },
  event_key_held: { title: 'Event: Klawisz trzymany', color: '#b91c1c', event: true, outs: ['exec'], params: [{ key: 'key', label: 'Klawisz (np. ArrowLeft)', def: 'ArrowRight' }] },
  print: { title: 'Wypisz', color: '#2563eb', outs: ['exec'], params: [{ key: 'text', label: 'Tekst / $zmienna', def: 'Hello' }] },
  move: { title: 'Przesuwaj (na sekundę)', color: '#0d9488', outs: ['exec'], params: [{ key: 'actor', label: 'Obiekt', def: '' }, { key: 'dx', label: 'dx', def: '100' }, { key: 'dy', label: 'dy', def: '0' }] },
  set_position: { title: 'Ustaw pozycję', color: '#0d9488', outs: ['exec'], params: [{ key: 'actor', label: 'Obiekt', def: '' }, { key: 'x', label: 'x', def: '0' }, { key: 'y', label: 'y', def: '0' }] },
  set_color: { title: 'Ustaw kolor', color: '#0d9488', outs: ['exec'], params: [{ key: 'actor', label: 'Obiekt', def: '' }, { key: 'color', label: 'Kolor (#rrggbb)', def: '#22c55e' }] },
  set_var: { title: 'Ustaw zmienną', color: '#7c3aed', outs: ['exec'], params: [{ key: 'name', label: 'Nazwa', def: 'score' }, { key: 'value', label: 'Wartość', def: '0' }] },
  add_var: { title: 'Dodaj do zmiennej', color: '#7c3aed', outs: ['exec'], params: [{ key: 'name', label: 'Nazwa', def: 'score' }, { key: 'value', label: 'Ile', def: '1' }] },
  branch: { title: 'Jeśli', color: '#d97706', outs: ['true', 'false'], params: [{ key: 'left', label: 'Lewa ($zmienna/liczba)', def: '$score' }, { key: 'op', label: 'Operator (== != < <= > >=)', def: '>=' }, { key: 'right', label: 'Prawa', def: '10' }] },
};

export const NODE_W = 190;
export const nodeHeight = (n: BpNode) => 34 + (NODE_CATALOG[n.kind]?.params.length ?? 0) * 0 + 14;
export const pinPos = (n: BpNode, pin: string, side: 'in' | 'out') => {
  const spec = NODE_CATALOG[n.kind];
  if (side === 'in') return { x: n.x, y: n.y + 20 };
  const i = Math.max(0, spec?.outs.indexOf(pin) ?? 0);
  return { x: n.x + NODE_W, y: n.y + 20 + i * 18 };
};

export function newProject(name = 'MojaGra'): Project {
  return {
    name, width: 800, height: 600, background: '#0f172a',
    entities: [{ name: 'player', x: 100, y: 250, w: 48, h: 48, color: '#3b82f6', shape: 'rect' }],
    blueprint: {
      nodes: [
        { id: 1, kind: 'event_key_held', params: { key: 'ArrowRight' }, x: 40, y: 40 },
        { id: 2, kind: 'move', params: { actor: 'player', dx: '250', dy: '0' }, x: 300, y: 40 },
        { id: 3, kind: 'event_key_held', params: { key: 'ArrowLeft' }, x: 40, y: 140 },
        { id: 4, kind: 'move', params: { actor: 'player', dx: '-250', dy: '0' }, x: 300, y: 140 },
      ],
      edges: [{ from: 1, fromPin: 'exec', to: 2 }, { from: 3, fromPin: 'exec', to: 4 }],
    },
  };
}

export const nextNodeId = (nodes: BpNode[]) => nodes.reduce((m, n) => Math.max(m, n.id), 0) + 1;

export function addNode(p: Project, kind: string, x: number, y: number): Project {
  const spec = NODE_CATALOG[kind];
  if (!spec) return p;
  const params: Record<string, string> = {};
  for (const prm of spec.params) params[prm.key] = prm.key === 'actor' ? (p.entities[0]?.name ?? '') : prm.def;
  const node: BpNode = { id: nextNodeId(p.blueprint.nodes), kind, params, x, y };
  return { ...p, blueprint: { ...p.blueprint, nodes: [...p.blueprint.nodes, node] } };
}

/** Jedno połączenie na pin wyjściowy; zabrania pętli własnych i celowania w zdarzenia. */
export function connect(p: Project, from: number, fromPin: string, to: number): Project {
  const nodes = p.blueprint.nodes;
  const target = nodes.find((n) => n.id === to);
  if (from === to || !target || NODE_CATALOG[target.kind]?.event) return p;
  const edges = p.blueprint.edges.filter((e) => !(e.from === from && e.fromPin === fromPin));
  return { ...p, blueprint: { nodes, edges: [...edges, { from, fromPin, to }] } };
}

export function removeNode(p: Project, id: number): Project {
  return { ...p, blueprint: {
    nodes: p.blueprint.nodes.filter((n) => n.id !== id),
    edges: p.blueprint.edges.filter((e) => e.from !== id && e.to !== id),
  } };
}

/** Wyciąga JSON grafu z odpowiedzi AI (obsługuje ```json ... ```). */
export function parseAiBlueprint(text: string): { nodes: BpNode[]; edges: BpEdge[] } | null {
  const m = text.match(/```(?:json)?\s*([\s\S]*?)```/);
  const raw = m ? m[1] : text.slice(text.indexOf('{'), text.lastIndexOf('}') + 1);
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v.nodes) || !Array.isArray(v.edges)) return null;
    const nodes: BpNode[] = v.nodes
      .filter((n: any) => Number.isInteger(n.id) && typeof n.kind === 'string' && NODE_CATALOG[n.kind])
      .map((n: any, i: number) => ({ id: n.id, kind: n.kind, params: { ...(n.params ?? {}) }, x: Number(n.x) || 40 + (i % 3) * 240, y: Number(n.y) || 40 + Math.floor(i / 3) * 110 }));
    const ids = new Set(nodes.map((n) => n.id));
    const edges: BpEdge[] = v.edges
      .filter((e: any) => ids.has(e.from) && ids.has(e.to))
      .map((e: any) => ({ from: e.from, fromPin: e.fromPin ?? 'exec', to: e.to }));
    return nodes.length ? { nodes, edges } : null;
  } catch { return null; }
}

export const AI_SYSTEM_PROMPT = `Jesteś asystentem Blue Engine. Na podstawie opisu gry zwróć WYŁĄCZNIE JSON {"nodes":[...],"edges":[...]} (opcjonalnie w bloku \`\`\`json).
Węzeł: {"id":liczba,"kind":...,"params":{...}}. Dostępne kind i params:
${Object.entries(NODE_CATALOG).map(([k, s]) => `- ${k}: ${s.params.map((p) => p.key).join(', ') || '(brak)'}${s.outs.length > 1 ? ' [wyjścia: true, false]' : ''}`).join('\n')}
Krawędź: {"from":id,"fromPin":"exec"|"true"|"false","to":id}. Zdarzenia (event_*) są początkiem łańcucha i nie mogą być celem.
Klawisze to KeyboardEvent.code (ArrowLeft, Space, KeyA). Nazwy obiektów podaj dokładnie jak w scenie.`;

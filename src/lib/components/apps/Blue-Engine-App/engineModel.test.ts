import { describe, it, expect } from 'vitest';
import { newProject, addNode, connect, removeNode, parseAiBlueprint } from './engineModel';

describe('engine blueprint model', () => {
  it('adds nodes with defaults and the first actor', () => {
    const p = addNode(newProject(), 'move', 10, 10);
    const n = p.blueprint.nodes[p.blueprint.nodes.length - 1];
    expect(n.params.actor).toBe('player');
    expect(n.id).toBe(5);
  });
  it('connect replaces the edge on the same pin and refuses events as targets', () => {
    let p = addNode(newProject(), 'print', 0, 0);       // id 5
    p = connect(p, 1, 'exec', 5);
    expect(p.blueprint.edges.filter((e) => e.from === 1)).toHaveLength(1);
    expect(p.blueprint.edges.find((e) => e.from === 1)!.to).toBe(5);
    const same = connect(p, 2, 'exec', 1);               // 1 to zdarzenie
    expect(same).toBe(p);
  });
  it('removeNode cleans edges', () => {
    const p = removeNode(newProject(), 2);
    expect(p.blueprint.edges.some((e) => e.to === 2)).toBe(false);
  });
  it('parses fenced AI json and drops unknown nodes', () => {
    const r = parseAiBlueprint('Oto graf:\n```json\n{"nodes":[{"id":1,"kind":"event_tick"},{"id":2,"kind":"hax"},{"id":3,"kind":"print","params":{"text":"x"}}],"edges":[{"from":1,"to":3},{"from":1,"to":2}]}\n```');
    expect(r!.nodes.map((n) => n.id)).toEqual([1, 3]);
    expect(r!.edges).toEqual([{ from: 1, fromPin: 'exec', to: 3 }]);
    expect(parseAiBlueprint('brak json')).toBeNull();
  });
});

export type PadAction = 'up' | 'down' | 'left' | 'right' | 'select' | 'back' | 'menu' | 'home' | 'prev' | 'next';

export interface GamepadSnapshot {
  /** `pressed` of each button, standard mapping order. */
  buttons: boolean[];
  /** Left stick X, Y (-1..1). */
  axes: number[];
}

const BUTTON_ACTIONS: Record<number, PadAction> = {
  0: 'select', // A / Cross
  1: 'back',   // B / Circle
  3: 'menu',   // Y / Triangle
  4: 'prev',   // LB
  5: 'next',   // RB
  9: 'home',   // Start
  16: 'home',  // Guide / PS button
  12: 'up', 13: 'down', 14: 'left', 15: 'right',
};

const REPEATING: ReadonlySet<PadAction> = new Set(['up', 'down', 'left', 'right']);

export const STICK_DEADZONE = 0.55;
export const REPEAT_DELAY_MS = 380;
export const REPEAT_INTERVAL_MS = 110;

/** Everything "held" in a snapshot, as the set of actions it represents. */
export function heldActions(s: GamepadSnapshot): Set<PadAction> {
  const held = new Set<PadAction>();
  s.buttons.forEach((down, i) => { if (down && BUTTON_ACTIONS[i]) held.add(BUTTON_ACTIONS[i]); });
  const x = s.axes[0] ?? 0; const y = s.axes[1] ?? 0;
  if (Math.abs(x) >= STICK_DEADZONE && Math.abs(x) >= Math.abs(y)) held.add(x > 0 ? 'right' : 'left');
  else if (Math.abs(y) >= STICK_DEADZONE) held.add(y > 0 ? 'down' : 'up');
  return held;
}

/** Edge detection + key repeat. One instance per game pad. */
export class PadTracker {
  private since = new Map<PadAction, number>();
  private lastRepeat = new Map<PadAction, number>();

  /** Feed one snapshot; returns the actions fired at `now` (ms). */
  step(now: number, snapshot: GamepadSnapshot): PadAction[] {
    const held = heldActions(snapshot);
    const fired: PadAction[] = [];
    for (const a of held) {
      const start = this.since.get(a);
      if (start === undefined) {
        this.since.set(a, now); this.lastRepeat.set(a, now);
        fired.push(a);
      } else if (REPEATING.has(a) && now - start >= REPEAT_DELAY_MS && now - (this.lastRepeat.get(a) ?? 0) >= REPEAT_INTERVAL_MS) {
        this.lastRepeat.set(a, now);
        fired.push(a);
      }
    }
    for (const a of [...this.since.keys()]) if (!held.has(a)) { this.since.delete(a); this.lastRepeat.delete(a); }
    return fired;
  }

  reset() { this.since.clear(); this.lastRepeat.clear(); }
}

import { describe, it, expect, beforeEach } from 'vitest';
import { notificationManager } from './notificationManager';

// `notificationManager` is a module-level singleton (see
// notificationManager.ts), so every test starts from a clean slate via
// `clearAll()` rather than constructing a fresh instance per test.
describe('notificationManager', () => {
  beforeEach(() => {
    notificationManager.clearAll();
  });

  it('starts empty', () => {
    let received: unknown[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });
    expect(received).toEqual([]);
    unsub();
  });

  it('add() prepends a new notification with generated id/timestamp/read=false', () => {
    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    notificationManager.add({ title: 'Hello', message: 'World' });

    expect(received).toHaveLength(1);
    expect(received[0].title).toBe('Hello');
    expect(received[0].message).toBe('World');
    expect(received[0].read).toBe(false);
    expect(typeof received[0].id).toBe('string');
    expect(received[0].id.length).toBeGreaterThan(0);
    expect(typeof received[0].timestamp).toBe('number');
    unsub();
  });

  it('newest notification appears first', () => {
    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    notificationManager.add({ title: 'First' });
    notificationManager.add({ title: 'Second' });

    expect(received.map((n) => n.title)).toEqual(['Second', 'First']);
    unsub();
  });

  it('markRead() flips only the targeted notification', async () => {
    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    notificationManager.add({ title: 'A' });
    notificationManager.add({ title: 'B' });
    const idOfB = received[0].id; // most recent = B, per prepend order above

    await notificationManager.markRead(idOfB);

    const b = received.find((n) => n.id === idOfB);
    const a = received.find((n) => n.title === 'A');
    expect(b.read).toBe(true);
    expect(a.read).toBe(false);
    unsub();
  });

  it('remove() drops exactly the targeted notification', () => {
    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    notificationManager.add({ title: 'Keep me' });
    notificationManager.add({ title: 'Remove me' });
    const idToRemove = received[0].id;

    notificationManager.remove(idToRemove);

    expect(received).toHaveLength(1);
    expect(received[0].title).toBe('Keep me');
    unsub();
  });

  it('subscribe() immediately replays current state to a new listener', () => {
    notificationManager.add({ title: 'Existing' });

    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    expect(received).toHaveLength(1);
    expect(received[0].title).toBe('Existing');
    unsub();
  });

  it('unsubscribe stops further updates from reaching the listener', () => {
    let callCount = 0;
    const unsub = notificationManager.subscribe(() => { callCount += 1; });
    const countAfterInitialReplay = callCount;

    unsub();
    notificationManager.add({ title: 'Should not be observed' });

    expect(callCount).toBe(countAfterInitialReplay);
  });

  it('clearAll() empties the list and notifies subscribers', () => {
    let received: any[] = [];
    const unsub = notificationManager.subscribe((n) => { received = n; });

    notificationManager.add({ title: 'A' });
    notificationManager.add({ title: 'B' });
    notificationManager.clearAll();

    expect(received).toEqual([]);
    unsub();
  });
});

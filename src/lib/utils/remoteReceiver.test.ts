import { describe, it, expect, beforeEach, vi } from 'vitest';
import { keyToAction, insertText, backspace, moveCursor, isTextEditable, editFocused, handleRemoteInput, fallbackNavigate, focusStep, __resetCursorForTests, type Env } from './remoteReceiver';

function env(dispatch = vi.fn()): Env & { dispatch: ReturnType<typeof vi.fn> } {
  return { doc: document, win: window, dispatch } as any;
}

describe('pure helpers', () => {
  it('maps keys to navigation actions', () => {
    expect(keyToAction('up')).toBe('up');
    expect(keyToAction('enter')).toBe('select');
    expect(keyToAction('home')).toBe('home');
    expect(keyToAction('rm -rf')).toBeNull();
  });
  it('inserts text at the caret and over a selection', () => {
    expect(insertText('helo', 2, 2, 'l')).toEqual({ value: 'hello', caret: 3 });
    expect(insertText('hello', 0, 5, 'x')).toEqual({ value: 'x', caret: 1 });
    expect(insertText('ab', 99, 99, 'c')).toEqual({ value: 'abc', caret: 3 });
  });
  it('backspace deletes one char or the selection', () => {
    expect(backspace('abc', 3, 3)).toEqual({ value: 'ab', caret: 2 });
    expect(backspace('abc', 0, 0)).toEqual({ value: 'abc', caret: 0 });
    expect(backspace('abcdef', 1, 4)).toEqual({ value: 'aef', caret: 1 });
  });
  it('keeps the virtual cursor on screen', () => {
    expect(moveCursor({ x: 10, y: 10 }, -50, -50, 1920, 1080)).toEqual({ x: 0, y: 0 });
    expect(moveCursor({ x: 1900, y: 1000 }, 100, 100, 1920, 1080)).toEqual({ x: 1919, y: 1079 });
    expect(moveCursor({ x: 5, y: 5 }, 3, 4, 100, 100)).toEqual({ x: 8, y: 9 });
  });
});

describe('DOM behaviour', () => {
  beforeEach(() => { document.body.innerHTML = ''; __resetCursorForTests(); });

  it('types into the focused text field and fires `input`', () => {
    document.body.innerHTML = '<input id="a" value="hel">';
    const a = document.getElementById('a') as HTMLInputElement;
    a.focus(); a.setSelectionRange(3, 3);
    const onInput = vi.fn(); a.addEventListener('input', onInput);
    handleRemoteInput({ input: { type: 'text', text: 'lo' }, handledBySystem: false }, env());
    expect(a.value).toBe('hello');
    expect(onInput).toHaveBeenCalled();
    handleRemoteInput({ input: { type: 'key', name: 'backspace' }, handledBySystem: false }, env());
    expect(a.value).toBe('hell');
  });

  it('ignores text when nothing editable is focused', () => {
    document.body.innerHTML = '<button id="b">x</button>';
    document.getElementById('b')!.focus();
    expect(editFocused(document, { kind: 'text', text: 'a' })).toBe(false);
    expect(isTextEditable(document.getElementById('b'))).toBe(false);
  });

  it('a TV shell that handles the action stops the fallback', () => {
    document.body.innerHTML = '<button id="b">x</button>';
    const b = document.getElementById('b') as HTMLButtonElement; b.focus();
    const click = vi.fn(); b.addEventListener('click', click);
    const e = env(vi.fn((name, detail: any) => { if (name === 'blue:remote-action') detail.handled = true; }));
    handleRemoteInput({ input: { type: 'key', name: 'select' }, handledBySystem: false }, e);
    expect(e.dispatch).toHaveBeenCalledWith('blue:remote-action', { action: 'select', handled: true });
    expect(click).not.toHaveBeenCalled();
  });

  it('without a handler, OK clicks the focused element and arrows become key events', () => {
    document.body.innerHTML = '<button id="b">x</button>';
    const b = document.getElementById('b') as HTMLButtonElement; b.focus();
    const click = vi.fn(); const key = vi.fn();
    b.addEventListener('click', click); b.addEventListener('keydown', key);
    handleRemoteInput({ input: { type: 'key', name: 'select' }, handledBySystem: false }, env());
    expect(click).toHaveBeenCalledTimes(1);
    handleRemoteInput({ input: { type: 'key', name: 'down' }, handledBySystem: false }, env());
    expect((key.mock.calls[0][0] as KeyboardEvent).key).toBe('ArrowDown');
    fallbackNavigate(document, 'back');
    expect((key.mock.calls[1][0] as KeyboardEvent).key).toBe('Escape');
  });

  it('does nothing twice when ydotool already injected the event', () => {
    const e = env();
    handleRemoteInput({ input: { type: 'key', name: 'up' }, handledBySystem: true }, e);
    handleRemoteInput({ input: { type: 'text', text: 'x' }, handledBySystem: true }, e);
    expect(e.dispatch).not.toHaveBeenCalled();
  });

  it('republishes media keys even when handled by the system, and leaves volume to the backend', () => {
    const e = env();
    handleRemoteInput({ input: { type: 'media', action: 'play_pause' }, handledBySystem: true }, e);
    expect(e.dispatch).toHaveBeenCalledWith('blue:media-key', 'play_pause');
    handleRemoteInput({ input: { type: 'volume', delta: 1 }, handledBySystem: false }, e);
    handleRemoteInput({ input: { type: 'mute' }, handledBySystem: false }, e);
    expect(e.dispatch).toHaveBeenCalledTimes(1);
  });

  it('draws a virtual cursor that moves and clicks the element under it', () => {
    document.body.innerHTML = '<button id="b" style="position:fixed;left:0;top:0;width:2000px;height:2000px">x</button>';
    const b = document.getElementById('b') as HTMLButtonElement;
    // jsdom has no layout: stub hit-testing
    (document as any).elementFromPoint = () => b;
    const click = vi.fn(); b.addEventListener('click', click);
    handleRemoteInput({ input: { type: 'move', dx: 10, dy: 10 }, handledBySystem: false }, env());
    expect(document.querySelector('[aria-hidden="true"]')).not.toBeNull();
    handleRemoteInput({ input: { type: 'click', button: 'left' }, handledBySystem: false }, env());
    expect(click).toHaveBeenCalledTimes(1);
  });

  it('Tab moves focus through focusable elements and wraps', () => {
    document.body.innerHTML = '<button id="a">a</button><button id="b">b</button>';
    (document.getElementById('a') as HTMLElement).focus();
    // jsdom has no layout, so every element reports offsetParent === null: allow the active one and elements in the tree
    Object.defineProperty(HTMLElement.prototype, 'offsetParent', { get() { return document.body; }, configurable: true });
    focusStep(document, 1);
    expect(document.activeElement?.id).toBe('b');
    focusStep(document, 1);
    expect(document.activeElement?.id).toBe('a');
    focusStep(document, -1);
    expect(document.activeElement?.id).toBe('b');
  });
});

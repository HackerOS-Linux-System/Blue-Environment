# `blue:back`

The phone and TV UIs have no window frame, so "Back" (Esc / Backspace / B on a
gamepad, the Back button, the remote's Back key) is dispatched to the foreground
app first. The contract is a **cancelable window event**:

```ts
window.addEventListener('blue:back', (e) => {
  if (dialogOpen) { closeDialog(); e.preventDefault(); }   // handled → shell does nothing more
  // not handled → don't call preventDefault; the shell leaves the app / goes up a level
});
```

The SDK wraps it:

```ts
import { onBack } from '@blue-environment/api';
const off = onBack(() => {
  if (!canGoUp()) return false;   // let the shell handle it
  goUp(); return true;            // handled
});
// call off() in your teardown
```

Rules of thumb: handle Back only when you really consumed it (closing a dialog,
leaving a sub-page) — otherwise the person gets stuck in your app. Several
listeners may be registered; any one cancelling the event counts as handled.
The shell dispatches it from `dispatchBack()` (`src/lib/altShell/shellUtils.ts`).
On the desktop UI the title-bar buttons are used and the event is not sent.

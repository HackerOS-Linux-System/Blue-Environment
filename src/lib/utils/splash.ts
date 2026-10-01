let hidden = false;

export function hideSplash(): void {
  if (hidden) return;
  hidden = true;
  const el = document.getElementById('blue-splash');
  if (!el) return;
  // Two frames: let the freshly mounted shell paint underneath first, so
  // the fade reveals a finished desktop rather than another blank frame.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      el.classList.add('blue-splash-hide');
      setTimeout(() => el.remove(), 500);
    }),
  );
}

/** Safety net: never leave the splash up forever if init hangs. */
export function hideSplashAfter(ms: number): void {
  setTimeout(hideSplash, ms);
}

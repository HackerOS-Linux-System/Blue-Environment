<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { a11y, applyA11y, clampMagnifierOrigin, magnifierTransform, updateA11y, MAGNIFIER_MAX, MAGNIFIER_MIN } from '../stores/accessibility';

  /**
   * Applies the accessibility settings to the document and implements the
   * screen magnifier: the whole shell is scaled up around the pointer, so the
   * area under the pointer stays put while the rest grows away from it.
   * Ctrl+Alt+= / Ctrl+Alt+- change the zoom, Ctrl+Alt+M toggles it.
   */
  let target: HTMLElement | null = null;
  let px = 0;
  let py = 0;
  let raf = 0;

  function paint(enabled: boolean, zoom: number) {
    if (!target) return;
    if (!enabled) { target.style.transform = ''; target.style.transformOrigin = ''; target.style.willChange = ''; return; }
    const o = clampMagnifierOrigin(px, py, window.innerWidth, window.innerHeight);
    const m = magnifierTransform(zoom, o.x, o.y);
    target.style.transform = m.transform;
    target.style.transformOrigin = m.origin;
    target.style.willChange = 'transform';
  }

  const unsub = a11y.subscribe((s) => { applyA11y(s); paint(s.magnifierEnabled, s.magnifierZoom); });

  function onMove(e: PointerEvent) {
    px = e.clientX; py = e.clientY;
    let enabled = false; let zoom = 2;
    a11y.subscribe((s) => { enabled = s.magnifierEnabled; zoom = s.magnifierZoom; })();
    if (!enabled || raf) return;
    raf = requestAnimationFrame(() => { raf = 0; paint(enabled, zoom); });
  }

  function onKey(e: KeyboardEvent) {
    if (!(e.ctrlKey && e.altKey)) return;
    let cur = { magnifierEnabled: false, magnifierZoom: 2 };
    a11y.subscribe((s) => (cur = s))();
    if (e.key === '=' || e.key === '+') { e.preventDefault(); updateA11y({ magnifierEnabled: true, magnifierZoom: Math.min(MAGNIFIER_MAX, cur.magnifierZoom + 0.5) }); }
    else if (e.key === '-') { e.preventDefault(); updateA11y({ magnifierZoom: Math.max(MAGNIFIER_MIN, cur.magnifierZoom - 0.5) }); }
    else if (e.key.toLowerCase() === 'm') { e.preventDefault(); updateA11y({ magnifierEnabled: !cur.magnifierEnabled }); }
  }

  onMount(() => {
    target = document.getElementById('app') ?? document.body;
    px = window.innerWidth / 2; py = window.innerHeight / 2;
    window.addEventListener('pointermove', onMove, { passive: true });
    window.addEventListener('keydown', onKey);
  });
  onDestroy(() => {
    unsub();
    if (typeof window !== 'undefined') { window.removeEventListener('pointermove', onMove); window.removeEventListener('keydown', onKey); }
    paint(false, 1);
  });
</script>

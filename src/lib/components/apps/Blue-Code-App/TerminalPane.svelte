<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge } from '../../../utils/systemBridge';

  interface Props {
    windowId: string;
  }

  let { windowId }: Props = $props();

  let container = $state() as HTMLDivElement;

  onMount(() => {
    (async () => {
      try {
        const { Terminal } = await import('xterm');
        const { FitAddon } = await import('xterm-addon-fit');
        const { WebLinksAddon } = await import('xterm-addon-web-links');

        const term = new Terminal({
          cursorBlink: true, fontSize: 13, fontFamily: 'JetBrains Mono, monospace',
          theme: { background: '#0f172a', foreground: '#e2e8f0', cursor: '#60a5fa' },
        });
        const fit = new FitAddon();
        term.loadAddon(fit);
        term.loadAddon(new WebLinksAddon());
        term.open(container);
        fit.fit();

        const ro = new ResizeObserver(() => fit.fit());
        ro.observe(container);

        const res = await SystemBridge.spawnTerminal(windowId);
        if (!res.success) {
          term.writeln('\x1b[33m[No PTY — limited mode]\x1b[0m');
          term.write('$ ');
          let buf = '';
          term.onKey(({ key, domEvent }: any) => {
            if (domEvent.keyCode === 13) {
              SystemBridge.executeCommand(buf).then((r: any) => {
                term.writeln(''); term.writeln(r.stdout || r.stderr || ''); term.write('$ ');
              });
              buf = '';
            } else if (domEvent.keyCode === 8) {
              if (buf.length) { buf = buf.slice(0, -1); term.write('\b \b'); }
            } else {
              buf += key; term.write(key);
            }
          });
        } else {
          term.writeln('\x1b[32mTerminal ready.\x1b[0m');
          term.onData((d: string) => SystemBridge.writeToTerminal(d));
        }

        window.addEventListener('terminal-output', ((e: CustomEvent) => term.write(e.detail.data)) as EventListener);
      } catch {
        if (container) container.innerHTML = '<div class="p-4 text-slate-500 text-xs">xterm not available</div>';
      }
    })();
  });
</script>

<div bind:this={container} class="w-full h-full"></div>

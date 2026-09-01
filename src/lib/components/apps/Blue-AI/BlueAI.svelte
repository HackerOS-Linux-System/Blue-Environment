<script lang="ts">
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import { Send, Bot, User, Settings, X, Loader2, Trash2, Copy, Check, RefreshCw, ExternalLink, Cloud, HardDrive } from 'lucide-svelte';
  import { SystemBridge } from '../../../utils/systemBridge';
  import type { AIMessage, AIConfig } from '../../../types';
  import { AI_SERVICES } from './aiServices';

  let messages: AIMessage[] = [];
  let input = '';
  let isLoading = false;
  let config: AIConfig = { service: 'chatgpt', model: 'gpt-4o', apiKey: '' };
  let showSettings = false;
  let error: string | null = null;
  let scrollEl: HTMLDivElement;
  let textareaEl: HTMLTextAreaElement;
  let apiKeyInput = '';
  let prevMessageCount = 0;
  let configLoaded = false;

  // ── First-run / re-openable Setup screen ────────────────────────────
  // Previously the app defaulted straight to ChatGPT with no key and no
  // explanation, so it looked broken the moment someone typed a message.
  // Now: shown automatically the first time (no `config.configured` yet),
  // and reopenable any time via the header button. If "remember my
  // choice" is unchecked, it's shown on every open instead of only once.
  let showSetup = false;
  let setupService = 'chatgpt';
  let setupModel = '';
  let setupApiKey = '';
  let setupRemember = true;
  let ollamaChecking = false;
  let ollamaReachable = false;
  let ollamaModels: string[] = [];
  let ollamaError: string | null = null;
  let ollamaInstalling = false;
  let ollamaInstallLog: string[] = [];
  let unlistenInstallProgress: (() => void) | null = null;
  let unlistenInstallDone: (() => void) | null = null;

  $: setupServiceDef = AI_SERVICES.find((s) => s.id === setupService);
  $: setupModelOptions = setupService === 'local' && ollamaModels.length > 0
    ? ollamaModels
    : (setupServiceDef?.models ?? []);

  function openSetup() {
    setupService = config.service || 'chatgpt';
    setupModel = config.model || AI_SERVICES.find((s) => s.id === setupService)?.models[0] || '';
    setupApiKey = config.apiKey || '';
    setupRemember = config.rememberChoice ?? true;
    showSetup = true;
    showSettings = false;
    if (setupService === 'local') checkOllama();
  }

  async function checkOllama() {
    ollamaChecking = true;
    ollamaError = null;
    const status = await SystemBridge.checkOllamaStatus();
    ollamaReachable = status.reachable;
    ollamaModels = status.models;
    ollamaError = status.error ?? null;
    if (status.reachable && status.models.length > 0 && !status.models.includes(setupModel)) {
      setupModel = status.models[0];
    }
    ollamaChecking = false;
  }

  async function startOllamaInstall() {
    ollamaInstalling = true;
    ollamaInstallLog = [];
    unlistenInstallProgress = await SystemBridge.onOllamaInstallProgress((line) => {
      ollamaInstallLog = [...ollamaInstallLog, line];
    });
    unlistenInstallDone = await SystemBridge.onOllamaInstallDone(async (result) => {
      unlistenInstallProgress?.();
      unlistenInstallDone?.();
      ollamaInstalling = false;
      if (!result.success) {
        ollamaInstallLog = [...ollamaInstallLog, `— Install failed: ${result.error ?? 'unknown error'}`];
        return;
      }
      // Installer succeeded — refresh so the picker shows real detected
      // state instead of the user having to manually hit "Check again".
      await checkOllama();
    });
    await SystemBridge.installOllama();
  }

  function selectSetupService(id: string) {
    setupService = id;
    const svc = AI_SERVICES.find((s) => s.id === id);
    setupModel = svc?.models[0] || '';
    if (id === 'local') checkOllama();
  }

  async function completeSetup() {
    const newCfg: AIConfig = {
      service: setupService, model: setupModel, apiKey: setupApiKey,
      configured: true, rememberChoice: setupRemember,
    };
    await saveConfig(newCfg);
    apiKeyInput = setupApiKey;
    showSetup = false;
  }

  onMount(() => {
    SystemBridge.loadConfig().then((cfg: any) => {
      if (cfg.aiConfig) { config = cfg.aiConfig; apiKeyInput = config.apiKey; }
      configLoaded = true;
      if (!config.configured || config.rememberChoice === false) openSetup();
    });
  });
  onDestroy(() => {
    unlistenInstallProgress?.();
    unlistenInstallDone?.();
  });

  afterUpdate(() => {
    if (messages.length !== prevMessageCount || isLoading) {
      scrollEl?.scrollTo({ top: scrollEl.scrollHeight, behavior: 'smooth' });
      prevMessageCount = messages.length;
    }
  });

  async function saveConfig(newCfg: AIConfig) {
    config = newCfg;
    const appCfg = await SystemBridge.loadConfig();
    await SystemBridge.saveConfig({ ...appCfg, aiConfig: newCfg });
  }

  async function sendMessage() {
    const text = input.trim();
    if (!text || isLoading) return;
    error = null;
    input = '';
    if (textareaEl) textareaEl.style.height = '42px';
    const userMsg: AIMessage = { role: 'user', content: text };
    const newMessages = [...messages, userMsg];
    messages = newMessages;
    isLoading = true;
    try {
      const reply = await SystemBridge.aiCall({ service: config.service, apiKey: config.apiKey, model: config.model, messages: newMessages });
      messages = [...messages, { role: 'assistant', content: reply }];
    } catch (e: any) {
      error = e instanceof Error ? e.message : 'AI request failed';
    } finally {
      isLoading = false;
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); sendMessage(); }
  }

  function autoGrow(e: Event) {
    const t = e.target as HTMLTextAreaElement;
    t.style.height = 'auto';
    t.style.height = Math.min(t.scrollHeight, 128) + 'px';
  }

  $: currentService = AI_SERVICES.find((s) => s.id === config.service);

  function renderParts(content: string) {
    return content.split(/(```[\s\S]*?```)/g).map((part) => {
      if (part.startsWith('```')) {
        const lines = part.slice(3).split('\n');
        const lang = lines[0];
        const code = lines.slice(1, -1).join('\n');
        return { type: 'code' as const, lang, code };
      }
      return { type: 'text' as const, text: part };
    });
  }

  function handleServiceChange(e: Event) {
    const val = (e.currentTarget as HTMLSelectElement).value;
    const svc = AI_SERVICES.find((s) => s.id === val);
    saveConfig({ ...config, service: val, model: svc?.models[0] || '' });
  }
  function handleModelChange(e: Event) {
    saveConfig({ ...config, model: (e.currentTarget as HTMLSelectElement).value });
  }
  function saveApiKey() {
    saveConfig({ ...config, apiKey: apiKeyInput });
  }
</script>

<div class="flex flex-col h-full bg-slate-900 text-white overflow-hidden">
  <div class="shrink-0 h-12 flex items-center justify-between px-4 bg-slate-800 border-b border-white/5">
    <div class="flex items-center gap-2">
      <div class="w-7 h-7 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shadow"><Bot size={15} /></div>
      <span class="font-semibold text-sm">Blue AI</span>
      <span class="text-xs text-slate-500">— {currentService?.name} / {config.model}</span>
    </div>
    <div class="flex gap-1">
      {#if messages.length > 0}
        <button on:click={() => (messages = [])} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400" title="Clear"><Trash2 size={14} /></button>
      {/if}
      <button on:click={openSetup} class="p-1.5 hover:bg-white/10 rounded-lg text-slate-400" title="Setup (change provider/model)"><RefreshCw size={14} /></button>
      <button on:click={() => (showSettings = !showSettings)} class="p-1.5 rounded-lg {showSettings ? 'bg-blue-600/20 text-blue-400' : 'hover:bg-white/10 text-slate-400'}">
        <Settings size={16} />
      </button>
    </div>
  </div>

  {#if showSetup}
    <div class="flex-1 overflow-y-auto p-6">
      <div class="max-w-md mx-auto space-y-5">
        <div class="text-center">
          <div class="w-14 h-14 mx-auto rounded-2xl bg-gradient-to-br from-blue-500/20 to-purple-600/20 flex items-center justify-center mb-3 border border-white/5">
            <Bot size={28} class="text-blue-400" />
          </div>
          <h3 class="text-lg font-semibold">Set up Blue AI</h3>
          <p class="text-sm text-slate-400 mt-1">Pick a provider and model to chat with.</p>
        </div>

        <div class="grid grid-cols-2 gap-2">
          {#each AI_SERVICES as s (s.id)}
            <button on:click={() => selectSetupService(s.id)}
              class="flex items-center gap-2 px-3 py-2.5 rounded-xl border text-sm font-medium transition-colors {setupService === s.id ? 'bg-blue-600 border-blue-500 text-white' : 'border-white/10 text-slate-300 hover:bg-white/5'}">
              {#if s.kind === 'local'}<HardDrive size={15} />{:else}<Cloud size={15} />{/if}
              {s.name}
            </button>
          {/each}
        </div>

        {#if setupServiceDef?.kind === 'cloud'}
          <div>
            <span class="text-xs text-slate-400 block mb-1">Model</span>
            <select bind:value={setupModel} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none">
              {#each setupModelOptions as m (m)}<option value={m}>{m}</option>{/each}
            </select>
          </div>
          <div>
            <span class="text-xs text-slate-400 block mb-1">API Key</span>
            <input type="password" bind:value={setupApiKey} placeholder="sk-..."
              class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none" />
            {#if setupServiceDef?.keyHint}
              <p class="text-[11px] text-slate-500 mt-1 flex items-center gap-1">
                <ExternalLink size={10} /> Get one at {setupServiceDef.keyHint}
              </p>
            {/if}
          </div>
        {:else}
          <!-- Local (Ollama) setup -->
          <div class="bg-slate-800/60 border border-white/5 rounded-xl p-4 space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-sm font-medium">Ollama status</span>
              <button on:click={checkOllama} disabled={ollamaChecking}
                class="flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 disabled:opacity-50">
                <RefreshCw size={12} class={ollamaChecking ? 'animate-spin' : ''} /> Check again
              </button>
            </div>
            {#if ollamaChecking}
              <div class="flex items-center gap-2 text-sm text-slate-400"><Loader2 size={14} class="animate-spin" /> Checking localhost:11434…</div>
            {:else if ollamaReachable}
              <div class="flex items-center gap-2 text-sm text-green-400"><Check size={14} /> Ollama is running — {ollamaModels.length} model{ollamaModels.length === 1 ? '' : 's'} available.</div>
              {#if ollamaModels.length > 0}
                <select bind:value={setupModel} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none">
                  {#each ollamaModels as m (m)}<option value={m}>{m}</option>{/each}
                </select>
              {:else}
                <p class="text-xs text-amber-400">No models pulled yet. Run: <code class="bg-slate-950 px-1.5 py-0.5 rounded">ollama pull llama3.2</code></p>
              {/if}
            {:else}
              <div class="flex items-start gap-2 text-sm text-red-400"><X size={14} class="shrink-0 mt-0.5" /> {ollamaError || 'Not reachable'}</div>
              {#if ollamaInstalling}
                <div class="flex items-center gap-2 text-sm text-blue-400"><Loader2 size={14} class="animate-spin" /> Installing Ollama…</div>
                <div class="bg-slate-950 rounded-lg p-2 max-h-32 overflow-y-auto font-mono text-[11px] text-slate-400 space-y-0.5">
                  {#each ollamaInstallLog as line, i (i)}<div>{line}</div>{/each}
                  {#if ollamaInstallLog.length === 0}<div class="text-slate-600">Starting…</div>{/if}
                </div>
              {:else}
                <button on:click={startOllamaInstall}
                  class="w-full py-2 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm font-medium">
                  Setup — auto-install Ollama
                </button>
                <p class="text-[11px] text-slate-500">Runs the official installer (asks for your password once). It detects your GPU itself — you'll see exactly what it found in the log above once it starts.</p>
                <div class="text-xs text-slate-500 space-y-1 pt-1">
                  <p>Or manually:</p>
                  <p>1. Install Ollama — <span class="text-slate-400">ollama.com/download</span></p>
                  <p>2. Pull a model — <code class="bg-slate-950 px-1.5 py-0.5 rounded">ollama pull llama3.2</code></p>
                  <p>3. Make sure it's running, then tap "Check again" above.</p>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <label class="flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
          <input type="checkbox" bind:checked={setupRemember} class="w-4 h-4 accent-blue-500" />
          Remember my choice (skip this screen next time)
        </label>

        <div class="flex gap-2">
          {#if config.configured}
            <button on:click={() => (showSetup = false)} class="flex-1 py-2.5 rounded-xl border border-white/10 text-sm text-slate-300 hover:bg-white/5">Cancel</button>
          {/if}
          <button on:click={completeSetup}
            disabled={setupServiceDef?.kind === 'cloud' ? !setupApiKey.trim() || !setupModel : !ollamaReachable || !setupModel}
            class="flex-1 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-sm font-semibold">
            Start chatting
          </button>
        </div>
      </div>
    </div>
  {:else}

  {#if showSettings}
    <div class="shrink-0 border-b border-white/5 bg-slate-800/80 p-4 space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <span class="text-xs text-slate-400 block mb-1">Service</span>
          <select value={config.service} on:change={handleServiceChange} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
            {#each AI_SERVICES as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
          </select>
        </div>
        <div>
          <span class="text-xs text-slate-400 block mb-1">Model</span>
          <select value={config.model} on:change={handleModelChange} class="w-full bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none">
            {#each currentService?.models ?? [] as m (m)}<option value={m}>{m}</option>{/each}
          </select>
        </div>
      </div>
      {#if config.service !== 'local'}
        <div>
          <span class="text-xs text-slate-400 block mb-1">API Key</span>
          <div class="flex gap-2">
            <input type="password" bind:value={apiKeyInput} placeholder="sk-..." class="flex-1 bg-slate-900 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none" />
            <button on:click={saveApiKey} class="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm">Save</button>
          </div>
        </div>
      {/if}
      <button on:click={() => (showSettings = false)} class="w-full text-center text-xs text-slate-500 hover:text-white pt-1">Close</button>
    </div>
  {/if}

  <div bind:this={scrollEl} class="flex-1 overflow-y-auto p-4 space-y-4">
    {#if messages.length === 0 && !showSettings}
      <div class="flex flex-col items-center justify-center h-full text-center">
        <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-blue-500/20 to-purple-600/20 flex items-center justify-center mb-4 border border-white/5">
          <Bot size={32} class="text-blue-400" />
        </div>
        <h3 class="text-lg font-semibold mb-1">Blue AI</h3>
        <p class="text-slate-400 text-sm max-w-xs">
          {config.apiKey || config.service === 'local' ? 'Ask me anything.' : 'Open Settings to configure your API key.'}
        </p>
        {#if !config.apiKey && config.service !== 'local'}
          <button on:click={() => (showSettings = true)} class="mt-4 px-4 py-2 bg-blue-600 hover:bg-blue-500 rounded-xl text-sm">Open Settings</button>
        {/if}
      </div>
    {/if}

    {#each messages as msg, idx (idx)}
      <div class="flex gap-3 {msg.role === 'user' ? 'justify-end' : 'justify-start'}">
        {#if msg.role === 'assistant'}
          <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shrink-0 mt-0.5"><Bot size={14} /></div>
        {/if}
        <div class="group max-w-[80%] relative">
          <div class="rounded-2xl px-4 py-3 text-sm leading-relaxed {msg.role === 'user' ? 'bg-blue-600 text-white rounded-br-md' : 'bg-slate-800 text-slate-100 rounded-bl-md border border-white/5'}">
            {#each renderParts(msg.content) as part, i (i)}
              {#if part.type === 'code'}
                <pre class="bg-slate-950 rounded-lg p-3 my-2 overflow-x-auto text-xs font-mono text-green-300 border border-white/5">{#if part.lang}<div class="text-slate-500 text-[10px] mb-1">{part.lang}</div>{/if}{part.code}</pre>
              {:else}
                <span class="whitespace-pre-wrap">{part.text}</span>
              {/if}
            {/each}
          </div>
          <button on:click={() => navigator.clipboard.writeText(msg.content)} class="absolute top-1 right-1 opacity-0 group-hover:opacity-100 p-1 bg-black/40 rounded-md">
            <Copy size={10} />
          </button>
        </div>
        {#if msg.role === 'user'}
          <div class="w-7 h-7 rounded-full bg-slate-700 flex items-center justify-center shrink-0 mt-0.5"><User size={14} /></div>
        {/if}
      </div>
    {/each}

    {#if isLoading}
      <div class="flex gap-3">
        <div class="w-7 h-7 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shrink-0"><Bot size={14} /></div>
        <div class="bg-slate-800 rounded-2xl rounded-bl-md px-4 py-3 border border-white/5">
          <div class="flex gap-1 items-center h-5">
            {#each [0, 1, 2] as i (i)}<div class="w-1.5 h-1.5 bg-blue-400 rounded-full animate-bounce" style="animation-delay:{i * 150}ms;" />{/each}
          </div>
        </div>
      </div>
    {/if}

    {#if error}
      <div class="bg-red-500/10 border border-red-500/20 rounded-xl p-3 text-red-300 text-sm flex gap-2">
        <X size={14} class="shrink-0 mt-0.5" />{error}
      </div>
    {/if}
  </div>

  <div class="shrink-0 p-3 border-t border-white/5 bg-slate-800/50">
    <div class="flex gap-2 items-end">
      <textarea
        bind:this={textareaEl}
        bind:value={input}
        on:keydown={handleKeyDown}
        on:input={autoGrow}
        placeholder="Ask me anything... (Enter to send)"
        rows="1"
        class="flex-1 bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white resize-none focus:outline-none focus:border-blue-500/50 max-h-32 placeholder-slate-500"
        style="min-height:42px;"
      />
      <button on:click={sendMessage} disabled={!input.trim() || isLoading} class="w-10 h-10 bg-blue-600 hover:bg-blue-500 rounded-xl flex items-center justify-center shrink-0 disabled:opacity-40">
        {#if isLoading}<Loader2 size={16} class="animate-spin" />{:else}<Send size={16} />{/if}
      </button>
    </div>
    <div class="flex justify-between mt-1.5 px-1">
      <span class="text-[10px] text-slate-600">Shift+Enter = newline</span>
      <span class="text-[10px] text-slate-600">{messages.length} messages</span>
    </div>
  </div>
  {/if}
</div>

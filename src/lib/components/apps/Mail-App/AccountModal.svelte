<script lang="ts">
  import { X, Settings, Lock, Mail, Server, Eye, EyeOff } from 'lucide-svelte';
  import type { MailAccount } from './accounts';
  import { createEventDispatcher } from 'svelte';

  export let account: MailAccount | null = null;

  const dispatch = createEventDispatcher<{ save: MailAccount; close: void }>();

  const PRESETS: Record<string, Partial<MailAccount>> = {
    gmail: { imapHost: 'imap.gmail.com', imapPort: 993, smtpHost: 'smtp.gmail.com', smtpPort: 587, useSsl: true },
    outlook: { imapHost: 'outlook.office365.com', imapPort: 993, smtpHost: 'smtp.office365.com', smtpPort: 587, useSsl: true },
    yahoo: { imapHost: 'imap.mail.yahoo.com', imapPort: 993, smtpHost: 'smtp.mail.yahoo.com', smtpPort: 587, useSsl: true },
    icloud: { imapHost: 'imap.mail.me.com', imapPort: 993, smtpHost: 'smtp.mail.me.com', smtpPort: 587, useSsl: true },
  };

  let form: MailAccount = account ?? {
    id: Date.now().toString(), name: '', email: '', imapHost: '', imapPort: 993,
    smtpHost: '', smtpPort: 587, username: '', password: '', useSsl: true,
  };
  let showPass = false;
  let preset = '';

  function applyPreset(p: string) {
    preset = p;
    if (PRESETS[p]) form = { ...form, ...PRESETS[p] };
  }

  function handleSave() {
    if (!form.email || !form.imapHost || !form.smtpHost) return;
    const name = form.name || form.email.split('@')[0];
    dispatch('save', { ...form, name, username: form.username || form.email });
    dispatch('close');
  }
</script>

<div class="absolute inset-0 bg-black/60 flex items-center justify-center z-50">
  <div class="bg-slate-900 border border-white/10 rounded-2xl w-[480px] max-h-[90%] overflow-y-auto shadow-2xl">
    <div class="flex items-center justify-between px-5 py-4 border-b border-white/5">
      <div class="flex items-center gap-2">
        <Settings size={16} class="text-blue-400" />
        <span class="font-semibold">{account ? 'Edit Account' : 'Add Account'}</span>
      </div>
      <button on:click={() => dispatch('close')} class="text-slate-400 hover:text-white"><X size={16} /></button>
    </div>

    <div class="p-5 space-y-4">
      <div>
        <span class="block text-xs text-slate-400 mb-2">Quick setup</span>
        <div class="flex gap-2 flex-wrap">
          {#each Object.keys(PRESETS) as p (p)}
            <button on:click={() => applyPreset(p)} class="px-3 py-1.5 rounded-lg text-xs capitalize transition-colors {preset === p ? 'bg-blue-600 text-white' : 'bg-slate-800 text-slate-300 hover:bg-slate-700'}">{p}</button>
          {/each}
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div>
          <span class="block text-xs text-slate-400 mb-1">Display name</span>
          <input bind:value={form.name} placeholder="Your Name" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
        </div>
        <div>
          <span class="block text-xs text-slate-400 mb-1">Email address</span>
          <input type="email" bind:value={form.email} placeholder="you@example.com" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div>
          <span class="block text-xs text-slate-400 mb-1">Login username</span>
          <input bind:value={form.username} placeholder="same as email" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
        </div>
        <div>
          <span class="block text-xs text-slate-400 mb-1">Password</span>
          <div class="relative">
            {#if showPass}
              <input type="text" bind:value={form.password} placeholder="App password / token"
                class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50 pr-9" />
            {:else}
              <input type="password" bind:value={form.password} placeholder="App password / token"
                class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50 pr-9" />
            {/if}
            <button type="button" on:click={() => (showPass = !showPass)} class="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white">
              {#if showPass}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
            </button>
          </div>
        </div>
      </div>

      <div class="border-t border-white/5 pt-3">
        <div class="flex items-center gap-2 text-xs text-slate-400 mb-3"><Server size={12} /> Incoming (IMAP)</div>
        <div class="grid grid-cols-3 gap-3">
          <div class="col-span-2">
            <span class="block text-xs text-slate-400 mb-1">Host</span>
            <input bind:value={form.imapHost} placeholder="imap.example.com" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
          </div>
          <div>
            <span class="block text-xs text-slate-400 mb-1">Port</span>
            <input type="number" bind:value={form.imapPort} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
          </div>
        </div>
      </div>

      <div class="border-t border-white/5 pt-3">
        <div class="flex items-center gap-2 text-xs text-slate-400 mb-3"><Mail size={12} /> Outgoing (SMTP)</div>
        <div class="grid grid-cols-3 gap-3">
          <div class="col-span-2">
            <span class="block text-xs text-slate-400 mb-1">Host</span>
            <input bind:value={form.smtpHost} placeholder="smtp.example.com" class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
          </div>
          <div>
            <span class="block text-xs text-slate-400 mb-1">Port</span>
            <input type="number" bind:value={form.smtpPort} class="w-full bg-slate-800 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-blue-500/50" />
          </div>
        </div>
      </div>

      <label class="flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
        <input type="checkbox" bind:checked={form.useSsl} class="w-4 h-4 accent-blue-500" />
        <Lock size={13} class="text-green-400" /> Use SSL/TLS
      </label>

      <div class="text-xs text-slate-500 bg-slate-800/50 rounded-lg p-3">
        <strong class="text-slate-400">Note for Gmail/Outlook:</strong> Use an app-specific password, not your main account password. Enable IMAP access in your provider's settings.
      </div>
    </div>

    <div class="flex justify-end gap-2 px-5 py-4 border-t border-white/5">
      <button on:click={() => dispatch('close')} class="px-4 py-2 text-sm text-slate-400 hover:text-white">Cancel</button>
      <button on:click={handleSave} disabled={!form.email || !form.imapHost} class="px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-50 rounded-xl text-sm text-white">
        {account ? 'Save changes' : 'Add account'}
      </button>
    </div>
  </div>
</div>

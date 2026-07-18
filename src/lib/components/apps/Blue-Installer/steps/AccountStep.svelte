<script lang="ts">
  import { User, Eye, EyeOff } from 'lucide-svelte';
  import type { InstallState } from '../installState';

  export let state: InstallState;
  const { config } = state;
  let showPassword = false;
  let confirmPassword = '';

  $: passwordsMatch = $config.password.length > 0 && $config.password === confirmPassword;
  $: passwordStrength = $config.password.length >= 12 ? 'strong' : $config.password.length >= 8 ? 'ok' : 'weak';
</script>

<div class="flex-1 flex flex-col px-10 py-8 overflow-y-auto">
  <div class="flex items-center gap-2 mb-6"><User size={20} class="text-blue-400" /><h2 class="text-xl font-semibold text-white">Create your account</h2></div>

  <div class="max-w-md space-y-4">
    <div>
      <label class="block text-xs text-slate-500 mb-1.5">Full name</label>
      <input bind:value={$config.fullName} placeholder="Jane Doe" class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-white focus:outline-none focus:border-blue-500/50" />
    </div>
    <div>
      <label class="block text-xs text-slate-500 mb-1.5">Username</label>
      <input bind:value={$config.username} placeholder="jane" class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-white focus:outline-none focus:border-blue-500/50"
        on:input={() => (config.update((c) => ({ ...c, username: c.username.toLowerCase().replace(/[^a-z0-9_-]/g, '') })))} />
    </div>
    <div>
      <label class="block text-xs text-slate-500 mb-1.5">Computer name</label>
      <input bind:value={$config.hostname} placeholder="blue-pc" class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 text-white focus:outline-none focus:border-blue-500/50" />
    </div>
    <div>
      <label class="block text-xs text-slate-500 mb-1.5">Password</label>
      <div class="relative">
        {#if showPassword}
          <input type="text" bind:value={$config.password} placeholder="Choose a password"
            class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 pr-10 text-white focus:outline-none focus:border-blue-500/50" />
        {:else}
          <input type="password" bind:value={$config.password} placeholder="Choose a password"
            class="w-full bg-slate-800 border border-white/10 rounded-xl px-4 py-2.5 pr-10 text-white focus:outline-none focus:border-blue-500/50" />
        {/if}
        <button type="button" on:click={() => (showPassword = !showPassword)} class="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white">
          {#if showPassword}<EyeOff size={15} />{:else}<Eye size={15} />{/if}
        </button>
      </div>
      {#if $config.password}
        <div class="h-1 mt-1.5 rounded-full bg-slate-700 overflow-hidden">
          <div class="h-full transition-all" style="width:{passwordStrength === 'strong' ? '100' : passwordStrength === 'ok' ? '60' : '25'}%; background:{passwordStrength === 'strong' ? '#22c55e' : passwordStrength === 'ok' ? '#f59e0b' : '#ef4444'};" />
        </div>
      {/if}
    </div>
    <div>
      <label class="block text-xs text-slate-500 mb-1.5">Confirm password</label>
      {#if showPassword}
        <input type="text" bind:value={confirmPassword} placeholder="Re-enter password"
          class="w-full bg-slate-800 border rounded-xl px-4 py-2.5 text-white focus:outline-none {confirmPassword && !passwordsMatch ? 'border-red-500/50' : 'border-white/10 focus:border-blue-500/50'}" />
      {:else}
        <input type="password" bind:value={confirmPassword} placeholder="Re-enter password"
          class="w-full bg-slate-800 border rounded-xl px-4 py-2.5 text-white focus:outline-none {confirmPassword && !passwordsMatch ? 'border-red-500/50' : 'border-white/10 focus:border-blue-500/50'}" />
      {/if}
      {#if confirmPassword && !passwordsMatch}<p class="text-xs text-red-400 mt-1">Passwords don't match</p>{/if}
    </div>
    <label class="flex items-center gap-2 text-sm text-slate-300 cursor-pointer pt-2">
      <input type="checkbox" bind:checked={$config.autoLogin} class="w-4 h-4 accent-blue-500" />
      Log in automatically (no password needed at startup)
    </label>
  </div>
</div>

<script lang="ts">
  import { onMount } from 'svelte';
  import { SystemBridge } from '../../utils/systemBridge';
  import { Plus, RefreshCw } from 'lucide-svelte';

  interface UserInfo { username: string; uid: string; shell: string; home: string; groups: string[]; }

  let users: UserInfo[] = [];
  let loading = true;
  const currentUser = SystemBridge.getUsername();

  async function load() {
    loading = true;
    try {
      const result = await SystemBridge.executeCommand(
        "awk -F: '$3 >= 1000 && $3 < 65534 && $7 !~ /nologin|false|sync|halt|shutdown/ && $6 != \"/var/empty\" && $6 != \"/nonexistent\" {print $1\":\"$3\":\"$6\":\"$7}' /proc/1/root/etc/passwd 2>/dev/null || " +
        "awk -F: '$3 >= 1000 && $3 < 65534 && $7 !~ /nologin|false|sync|halt|shutdown/ && $6 != \"/var/empty\" && $6 != \"/nonexistent\" {print $1\":\"$3\":\"$6\":\"$7}' /etc/passwd"
      );
      const out = typeof result === 'string' ? result : (result as any)?.stdout || '';
      const us: UserInfo[] = [];
      for (const line of out.split('\n').filter(Boolean)) {
        const parts = line.split(':');
        if (parts.length < 4) continue;
        const [username, uid, home, shell] = parts;
        const grpResult = await SystemBridge.executeCommand(`groups ${username} 2>/dev/null`);
        const grpOut = typeof grpResult === 'string' ? grpResult : (grpResult as any)?.stdout || '';
        const groups = grpOut.split(':').slice(-1)[0]?.trim().split(' ') || [];
        us.push({ username, uid, home, shell, groups });
      }
      users = us.length > 0 ? us : [{ username: currentUser, uid: '1000', home: `/home/${currentUser}`, shell: '/bin/bash', groups: ['sudo', 'audio', 'video'] }];
    } catch { users = []; }
    loading = false;
  }

  onMount(load);

  async function changePassword(username: string) {
    if (username === currentUser) {
      await SystemBridge.launchApp('bash -c "xterm -e passwd" &').catch(() => {});
    } else {
      await SystemBridge.launchApp(`bash -c "xterm -e 'sudo passwd ${username}'" &`).catch(() => {});
    }
  }
</script>

{#if loading}
  <div class="flex items-center justify-center py-12"><RefreshCw size={20} class="animate-spin text-blue-400" /></div>
{:else}
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <p class="text-sm text-slate-400">System users (UID 1000+)</p>
      <button on:click={() => SystemBridge.launchApp('xterm -e "sudo adduser" &').catch(() => {})}
        class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 rounded-lg text-sm transition-colors">
        <Plus size={13} /> Add User
      </button>
    </div>
    {#each users as u (u.username)}
      <div class="bg-slate-800 rounded-xl p-4 border border-white/5">
        <div class="flex items-center gap-4">
          <div class="w-12 h-12 rounded-full bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center text-xl font-bold shrink-0">
            {u.username.charAt(0).toUpperCase()}
          </div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="font-medium text-white">{u.username}</span>
              {#if u.username === currentUser}
                <span class="text-xs bg-blue-600/20 text-blue-300 px-1.5 py-0.5 rounded border border-blue-500/20">You</span>
              {/if}
              {#if u.groups.includes('sudo') || u.groups.includes('wheel')}
                <span class="text-xs bg-orange-600/20 text-orange-300 px-1.5 py-0.5 rounded border border-orange-500/20">Admin</span>
              {/if}
            </div>
            <div class="text-xs text-slate-500 mt-0.5">UID: {u.uid} · {u.home} · {u.shell.split('/').pop()}</div>
            <div class="flex flex-wrap gap-1 mt-1.5">
              {#each u.groups.slice(0, 6) as g (g)}
                <span class="text-[10px] bg-white/5 text-slate-400 px-1.5 py-0.5 rounded">{g}</span>
              {/each}
              {#if u.groups.length > 6}<span class="text-[10px] text-slate-600">+{u.groups.length - 6} more</span>{/if}
            </div>
          </div>
          <div class="flex gap-1">
            <button on:click={() => changePassword(u.username)} class="px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors whitespace-nowrap">
              Change Password
            </button>
          </div>
        </div>
      </div>
    {/each}
  </div>
{/if}

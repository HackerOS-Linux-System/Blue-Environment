<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, ChevronDown, FileText, Globe, Mail, Music, Film, Image, Archive, Code2, Terminal, FileDown, CalendarDays } from 'lucide-svelte';
  import { SystemBridge } from '../../../../utils/systemBridge';
  import { t } from '../../../../stores/language';

  interface AppRole { id: string; labelKey: string; descKey: string; icon: any; mimeTypes: string[]; current: string | null; alternatives: AppOption[]; }
  interface AppOption { id: string; name: string; exec: string; icon?: string; }

  const ROLES: Omit<AppRole, 'current' | 'alternatives'>[] = [
    { id: 'text', labelKey: 'settings.defaultapps.role_text', descKey: 'settings.defaultapps.role_text_desc', icon: FileText, mimeTypes: ['text/plain'] },
    { id: 'web', labelKey: 'settings.defaultapps.role_web', descKey: 'settings.defaultapps.role_web_desc', icon: Globe, mimeTypes: ['text/html', 'x-scheme-handler/http', 'x-scheme-handler/https'] },
    { id: 'mail', labelKey: 'settings.defaultapps.role_mail', descKey: 'settings.defaultapps.role_mail_desc', icon: Mail, mimeTypes: ['x-scheme-handler/mailto'] },
    { id: 'music', labelKey: 'settings.defaultapps.role_music', descKey: 'settings.defaultapps.role_music_desc', icon: Music, mimeTypes: ['audio/mpeg', 'audio/ogg', 'audio/flac'] },
    { id: 'video', labelKey: 'settings.defaultapps.role_video', descKey: 'settings.defaultapps.role_video_desc', icon: Film, mimeTypes: ['video/mp4', 'video/mkv', 'video/webm'] },
    { id: 'image', labelKey: 'settings.defaultapps.role_image', descKey: 'settings.defaultapps.role_image_desc', icon: Image, mimeTypes: ['image/png', 'image/jpeg', 'image/webp'] },
    { id: 'archive', labelKey: 'settings.defaultapps.role_archive', descKey: 'settings.defaultapps.role_archive_desc', icon: Archive, mimeTypes: ['application/zip', 'application/x-tar'] },
    { id: 'code', labelKey: 'settings.defaultapps.role_code', descKey: 'settings.defaultapps.role_code_desc', icon: Code2, mimeTypes: ['text/x-python', 'text/x-csrc', 'text/x-script'] },
    { id: 'terminal', labelKey: 'settings.defaultapps.role_terminal', descKey: 'settings.defaultapps.role_terminal_desc', icon: Terminal, mimeTypes: [] },
    { id: 'pdf', labelKey: 'settings.defaultapps.role_pdf', descKey: 'settings.defaultapps.role_pdf_desc', icon: FileDown, mimeTypes: ['application/pdf'] },
    { id: 'calendar', labelKey: 'settings.defaultapps.role_calendar', descKey: 'settings.defaultapps.role_calendar_desc', icon: CalendarDays, mimeTypes: ['text/calendar', 'x-scheme-handler/webcal'] },
  ];

  const BLUE_ALTS: Record<string, AppOption[]> = {
    text: [{ id: 'notepad', name: 'Notepad (Blue)', exec: 'blue-environment --app notepad' }, { id: 'blue_docs', name: 'Blue Docs', exec: 'blue-environment --app blue_docs' }],
    code: [{ id: 'blue_code', name: 'Blue Code', exec: 'blue-environment --app blue_code' }],
    mail: [{ id: 'mail', name: 'Blue Mail', exec: 'blue-environment --app mail' }],
    archive: [{ id: 'blue_archive', name: 'Blue Archive', exec: 'blue-environment --app blue_archive' }],
    image: [{ id: 'blue_images', name: 'Blue Images', exec: 'blue-environment --app blue_images' }],
    video: [{ id: 'blue_video', name: 'Blue Video', exec: 'blue-environment --app blue_video' }],
    web: [{ id: 'blue_web', name: 'Blue Browser', exec: 'blue-environment --app blue_web' }],
    calendar: [{ id: 'blue_calendar', name: 'Blue Calendar', exec: 'blue-environment --app blue_calendar' }],
  };

  function out(r: any): string { return typeof r === 'string' ? r : r?.stdout ?? ''; }

  async function getXdgDefault(mimeType: string): Promise<string> {
    try {
      const result = await SystemBridge.executeCommand(`xdg-mime query default '${mimeType}' 2>/dev/null`);
      return out(result).trim().replace(/\.desktop$/, '');
    } catch { return ''; }
  }
  async function setXdgDefault(mimeType: string, desktop: string): Promise<void> {
    try { await SystemBridge.executeCommand(`xdg-mime default '${desktop}.desktop' '${mimeType}' 2>/dev/null`); } catch {}
  }
  async function findAlternatives(mimeTypes: string[]): Promise<AppOption[]> {
    if (mimeTypes.length === 0) return [];
    try {
      const mime = mimeTypes[0];
      const result = await SystemBridge.executeCommand(`grep -rl 'MimeType.*${mime.split('/')[0]}' /usr/share/applications/ 2>/dev/null | head -20`);
      const apps: AppOption[] = [];
      for (const file of out(result).trim().split('\n').filter(Boolean)) {
        const content = await SystemBridge.executeCommand(`cat '${file}' 2>/dev/null`).catch(() => '');
        const text = out(content);
        const name = text.match(/^Name=(.+)$/m)?.[1]?.trim() ?? '';
        const exec = text.match(/^Exec=(.+)$/m)?.[1]?.trim() ?? '';
        const id = file.split('/').pop()?.replace('.desktop', '') ?? '';
        if (name && exec && id) apps.push({ id, name, exec });
        if (apps.length >= 8) break;
      }
      return apps;
    } catch { return []; }
  }

  let roles: AppRole[] = [];
  let expanded: string | null = null;
  let loading = true;
  let setting: string | null = null;

  async function loadRoles() {
    loading = true;
    roles = await Promise.all(ROLES.map(async (role) => {
      const current = role.mimeTypes.length > 0 ? await getXdgDefault(role.mimeTypes[0]) : '';
      const systemAlts = await findAlternatives(role.mimeTypes);
      const blueAlts = BLUE_ALTS[role.id] ?? [];
      const seen = new Set<string>();
      const alternatives = [...blueAlts, ...systemAlts].filter((a) => { if (seen.has(a.id)) return false; seen.add(a.id); return true; });
      return { ...role, current: current || null, alternatives };
    }));
    loading = false;
  }
  onMount(loadRoles);

  async function choose(role: AppRole, alt: AppOption) {
    setting = role.id;
    for (const mime of role.mimeTypes) await setXdgDefault(mime, alt.id);
    roles = roles.map((r) => (r.id === role.id ? { ...r, current: alt.id } : r));
    setting = null;
    expanded = null;
  }
</script>

{#if loading}
  <div class="flex items-center justify-center h-40 text-slate-500 text-sm">{$t('settings.defaultapps.loading')}</div>
{:else}
  <div class="space-y-6">
    <h2 class="text-2xl font-bold text-white">{$t('settings.defaultapps.title')}</h2>
    <p class="text-slate-500 text-sm -mt-4">{$t('settings.defaultapps.desc')}</p>

    <div class="space-y-2">
      {#each roles as role (role.id)}
        {@const isOpen = expanded === role.id}
        {@const hasAlts = role.alternatives.length > 1 || (role.alternatives.length === 1 && role.alternatives[0].id !== role.current)}
        <div class="bg-slate-800 rounded-2xl border border-white/5 overflow-hidden">
          <button class="w-full flex items-center gap-4 px-5 py-4 transition-colors {hasAlts ? 'hover:bg-white/5 cursor-pointer' : 'cursor-default'}"
            on:click={() => hasAlts && (expanded = isOpen ? null : role.id)}>
            <div class="w-9 h-9 rounded-xl bg-slate-700 flex items-center justify-center text-blue-400 shrink-0">
              <svelte:component this={role.icon} size={18} />
            </div>
            <div class="flex-1 text-left">
              <div class="text-sm text-white font-medium">{$t(role.labelKey)}</div>
              <div class="text-xs text-slate-500 mt-0.5">
                {#if role.current}
                  <span class="text-blue-400">{role.alternatives.find((a) => a.id === role.current)?.name ?? role.current}</span>
                {:else}
                  <span class="text-slate-600">{$t('settings.defaultapps.no_default')}</span>
                {/if}
              </div>
            </div>
            {#if hasAlts}<ChevronDown size={14} class="text-slate-600 transition-transform {isOpen ? 'rotate-180' : ''}" />
            {:else}<span class="text-xs text-slate-700 italic">{$t('settings.defaultapps.no_alternatives')}</span>{/if}
          </button>

          {#if isOpen}
            <div class="border-t border-white/5">
              {#each role.alternatives as alt (alt.id)}
                <button on:click={() => choose(role, alt)} disabled={setting === role.id} class="w-full flex items-center gap-3 px-5 py-3 hover:bg-white/5 transition-colors">
                  <div class="w-5 h-5 rounded-full border-2 flex items-center justify-center shrink-0" style="border-color:{role.current === alt.id ? '#3b82f6' : '#334155'};">
                    {#if role.current === alt.id}<div class="w-2.5 h-2.5 rounded-full bg-blue-500" />{/if}
                  </div>
                  <span class="text-sm text-slate-300">{alt.name}</span>
                  {#if role.current === alt.id}<Check size={13} class="ml-auto text-blue-400" />{/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <p class="text-xs text-slate-700">{$t('settings.defaultapps.applied_hint')}</p>
  </div>
{/if}

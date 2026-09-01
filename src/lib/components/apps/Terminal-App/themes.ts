export const THEMES = {
    'Blue Dark': {
        background: '#0d1117', foreground: '#e6edf3', cursor: '#58a6ff',
        black: '#161b22', red: '#ff7b72', green: '#3fb950', yellow: '#d29922',
        blue: '#58a6ff', magenta: '#bc8cff', cyan: '#39d353', white: '#b1bac4',
        brightBlack: '#6e7681', brightRed: '#ffa198', brightGreen: '#56d364',
        brightYellow: '#e3b341', brightBlue: '#79c0ff', brightMagenta: '#d2a8ff',
        brightCyan: '#56d364', brightWhite: '#f0f6fc',
    },
    'Dracula': {
        background: '#282a36', foreground: '#f8f8f2', cursor: '#f8f8f2',
        black: '#21222c', red: '#ff5555', green: '#50fa7b', yellow: '#f1fa8c',
        blue: '#bd93f9', magenta: '#ff79c6', cyan: '#8be9fd', white: '#f8f8f2',
        brightBlack: '#6272a4', brightRed: '#ff6e6e', brightGreen: '#69ff94',
        brightYellow: '#ffffa5', brightBlue: '#d6acff', brightMagenta: '#ff92df',
        brightCyan: '#a4ffff', brightWhite: '#ffffff',
    },
    'Solarized': {
        background: '#002b36', foreground: '#839496', cursor: '#839496',
        black: '#073642', red: '#dc322f', green: '#859900', yellow: '#b58900',
        blue: '#268bd2', magenta: '#d33682', cyan: '#2aa198', white: '#eee8d5',
        brightBlack: '#002b36', brightRed: '#cb4b16', brightGreen: '#586e75',
        brightYellow: '#657b83', brightBlue: '#839496', brightMagenta: '#6c71c4',
        brightCyan: '#93a1a1', brightWhite: '#fdf6e3',
    },
    // Matches the Hydra shell theme's palette (see
    // src/lib/data/builtinThemes.ts) — not auto-selected by picking a
    // shell theme (this app's own theme picker in SettingsPanel.svelte
    // is a fully separate, person-controlled setting; silently
    // overriding someone's chosen terminal color scheme just because
    // they picked a shell theme would be surprising, same reasoning as
    // why Window.svelte's `--shell-radius` only overrides Tailwind's
    // *fallback* value, never forces a specific look on content that
    // already has its own explicit choice). `terminalSession.ts`
    // defaults new installs to this when no theme has ever been chosen
    // *and* Hydra is the active shell theme — see that file's
    // `defaultThemeFor` — but never overrides an explicit pick.
    'Hydra': {
        background: '#12071f', foreground: '#fdf2ff', cursor: '#ec4899',
        black: '#1d0f2e', red: '#f43f5e', green: '#22d3ee', yellow: '#fbbf24',
        blue: '#3b82f6', magenta: '#ec4899', cyan: '#22d3ee', white: '#d8b4fe',
        brightBlack: '#2a1642', brightRed: '#fb7185', brightGreen: '#67e8f9',
        brightYellow: '#fde047', brightBlue: '#60a5fa', brightMagenta: '#f472b6',
        brightCyan: '#67e8f9', brightWhite: '#fdf2ff',
    },
} as const;

export type ThemeName = keyof typeof THEMES;

export interface Tab {
    id: string;
    title: string;
}

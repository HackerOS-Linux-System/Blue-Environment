let Terminal: any = null;
let FitAddon: any = null;
let WebLinksAddon: any = null;

export function getXtermClasses() {
    return { Terminal, FitAddon, WebLinksAddon };
}

export async function loadXterm(): Promise<void> {
    const term = await import('xterm');
    Terminal = term.Terminal;
    const fit = await import('xterm-addon-fit');
    FitAddon = fit.FitAddon;
    try {
        const links = await import('xterm-addon-web-links');
        WebLinksAddon = links.WebLinksAddon;
    } catch {
        // Web-links addon is optional — terminal still works without clickable URLs.
    }
}

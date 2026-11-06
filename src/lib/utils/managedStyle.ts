export interface ManagedStyle {
    set(css: string): void;
    clear(): void;
    remove(): void;
}

const supportsAdopted = () =>
    typeof CSSStyleSheet !== 'undefined' && typeof (CSSStyleSheet.prototype as { replaceSync?: unknown }).replaceSync === 'function' && 'adoptedStyleSheets' in document;

export function createManagedStyle(id: string, root: Document = document): ManagedStyle {
    if (supportsAdopted()) {
        const sheet = new CSSStyleSheet();
        let attached = false;
        const attach = () => {
            if (attached) return;
            root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
            attached = true;
        };
        return {
            set(css) { sheet.replaceSync(css); attach(); },
            clear() { sheet.replaceSync(''); },
            remove() { root.adoptedStyleSheets = root.adoptedStyleSheets.filter((s) => s !== sheet); attached = false; },
        };
    }
    let el: HTMLStyleElement | null = null;
    return {
        set(css) {
            if (!el) { el = root.createElement('style'); el.id = id; root.head.appendChild(el); }
            el.textContent = css;
        },
        clear() { if (el) el.textContent = ''; },
        remove() { el?.remove(); el = null; },
    };
}

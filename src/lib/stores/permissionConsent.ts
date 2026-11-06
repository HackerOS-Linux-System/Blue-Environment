import { writable, get } from 'svelte/store';

/**
 * Consent for package permissions (Blue Store). The backend
 * (`BlueStore/permissions.rs`) lists what a package asks for; the person
 * decides here, and the decision is what the runtime enforces.
 */

export type Risk = 'low' | 'medium' | 'high';

export interface PermissionInfo { id: string; risk: Risk; title: string; description: string }

export type SignatureStatus =
    | { status: 'unsigned' }
    | { status: 'trusted'; signer: string; name: string }
    | { status: 'unknownSigner'; signer: string }
    | { status: 'invalid'; reason: string };

/** The part of `store_resolve`'s answer the consent dialog needs. */
export interface ConsentPreview {
    manifest: { id: string; name: string; version: string; author: string; icon?: string };
    permissions: PermissionInfo[];
    risk: Risk;
    newPermissions: string[];
    previousGranted: string[] | null;
    signature: SignatureStatus;
}

/**
 * A decision is needed when the package asks for something the person has not
 * decided on yet: a first install, or an update that adds permissions.
 * (An update that asks for nothing new keeps the earlier decision silently —
 * the backend enforces the same rule.)
 */
export function needsConsent(p: ConsentPreview): boolean {
    if (p.permissions.length === 0) return false;
    return p.previousGranted === null || p.newPermissions.length > 0;
}

/**
 * Pre-selection: what was granted before (updates), plus low-risk permissions.
 * Medium and high risk are never pre-selected on a first install — the person opts in.
 */
export function defaultSelection(p: ConsentPreview): string[] {
    const before = new Set(p.previousGranted ?? []);
    return p.permissions.filter((x) => before.has(x.id) || x.risk === 'low').map((x) => x.id);
}

export const RISK_ORDER: Record<Risk, number> = { high: 0, medium: 1, low: 2 };

export function isNew(p: ConsentPreview, id: string): boolean {
    return p.newPermissions.includes(id);
}

export interface ConsentRequest {
    preview: ConsentPreview;
    answer: (granted: string[] | null) => void;
}

export const consentRequest = writable<ConsentRequest | null>(null);

let chain: Promise<unknown> = Promise.resolve();

/** Asks the person; resolves with the granted ids, or `null` if they cancelled. Requests queue up. */
export function requestPermissionConsent(preview: ConsentPreview): Promise<string[] | null> {
    const next = chain.then(
        () =>
            new Promise<string[] | null>((resolve) => {
                consentRequest.set({
                    preview,
                    answer: (g) => {
                        consentRequest.set(null);
                        resolve(g);
                    },
                });
            }),
    );
    chain = next.catch(() => undefined);
    return next;
}

export function isConsentOpen(): boolean {
    return get(consentRequest) !== null;
}

export interface VaultEntry {
  id: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  createdAt: string;
  updatedAt: string;
  /** Secret Service (libsecret) attributes; empty for entries made here. */
  attributes?: Record<string, string>;
  contentType?: string;
  /** 'manager' (default) or 'secret-service' — stored by another application. */
  source?: string;
  createdBy?: string;
}

export function newEntry(): VaultEntry {
  const now = new Date().toISOString();
  return {
    id: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    title: '',
    username: '',
    password: '',
    url: '',
    notes: '',
    createdAt: now,
    updatedAt: now,
  };
}

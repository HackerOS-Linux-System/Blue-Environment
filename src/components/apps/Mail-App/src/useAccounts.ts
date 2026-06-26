import { useState, useCallback, useEffect } from 'react';
import { SystemBridge } from '../../../../utils/systemBridge';

export interface MailAccount {
    id: string; name: string; email: string;
    imapHost: string; imapPort: number;
    smtpHost: string; smtpPort: number;
    username: string; password: string;
    useSsl: boolean;
}

function toRust(a: MailAccount) {
    return { ...a, imap_host: a.imapHost, imap_port: a.imapPort, smtp_host: a.smtpHost, smtp_port: a.smtpPort, use_ssl: a.useSsl };
}

function fromRust(r: any): MailAccount {
    return { id: r.id, name: r.name, email: r.email, imapHost: r.imap_host, imapPort: r.imap_port, smtpHost: r.smtp_host, smtpPort: r.smtp_port, username: r.username, password: '', useSsl: r.use_ssl };
}

export function useAccounts() {
    const [accounts,   setAccounts]   = useState<MailAccount[]>([]);
    const [activeAcct, setActiveAcct] = useState<string | null>(null);
    const [loading,    setLoading]    = useState(false);

    useEffect(() => {
        if (!SystemBridge.isTauri()) return;
        SystemBridge.invoke<any[]>('mail_get_accounts').then(list => {
            if (list?.length) { setAccounts(list.map(fromRust)); setActiveAcct(list[0].id); }
        }).catch(() => {});
    }, []);

    const saveAccount = useCallback(async (acct: MailAccount) => {
        if (SystemBridge.isTauri()) await SystemBridge.invoke('mail_save_account', { account: toRust(acct) });
        setAccounts(prev => { const next = [...prev.filter(a => a.id !== acct.id), acct]; return next; });
        setActiveAcct(acct.id);
    }, []);

    const deleteAccount = useCallback(async (id: string) => {
        if (SystemBridge.isTauri()) await SystemBridge.invoke('mail_delete_account', { accountId: id });
        setAccounts(prev => prev.filter(a => a.id !== id));
        setActiveAcct(prev => prev === id ? null : prev);
    }, []);

    const activeAccount = accounts.find(a => a.id === activeAcct) ?? null;

    return { accounts, activeAcct, setActiveAcct, activeAccount, loading, setLoading, saveAccount, deleteAccount };
}

export type Channel = 'local' | 'sms' | 'xmpp' | 'matrix';

export interface Conversation {
  id: string;
  title: string;
  participant: string;
  channel: Channel;
  createdAt: string;
  lastMessagePreview: string;
  lastMessageAt: string;
  unreadCount: number;
  pinned: boolean;
}

export type MessageDirection = 'outgoing' | 'incoming';

export interface Message {
  id: string;
  conversationId: string;
  body: string;
  direction: MessageDirection;
  sentAt: string;
  read: boolean;
}

/** Labels + icon names for each channel. `local` and `matrix` are fully
 * wired; `xmpp` sends/receives through the best-effort transport in
 * `xmpp.rs` (real, but ephemeral-connection-per-action — see that
 * module's doc); `sms` sends/receives through a locally-attached modem
 * via ModemManager (see `sms.rs`'s doc) — there is still no support for
 * routing SMS through a paired phone. */
export const CHANNEL_LABELS: Record<Channel, string> = {
  local: 'Local note',
  sms: 'SMS (modem or paired phone)',
  xmpp: 'XMPP',
  matrix: 'Matrix',
};

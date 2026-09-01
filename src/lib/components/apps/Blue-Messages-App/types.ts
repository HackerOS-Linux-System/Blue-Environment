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

/** Labels + icon names for each channel — `local` is the only one with
 * a working transport today (see BlueMessagesApp/mod.rs's module doc);
 * the others render as selectable so the data model/UI are ready, but
 * `channel !== 'local'` conversations don't actually send anywhere yet. */
export const CHANNEL_LABELS: Record<Channel, string> = {
  local: 'Local note',
  sms: 'SMS (paired phone)',
  xmpp: 'XMPP',
  matrix: 'Matrix',
};

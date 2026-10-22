export interface ThreadEntry { a: string; t: number; x: string }
export interface ReviewComment {
  id: string; author: string; ts: number; text: string; quote: string; resolved: boolean; replies: ThreadEntry[];
}
export interface ReviewChange {
  id: string; type: 'insert' | 'delete'; author: string; ts: number; text: string; color: string;
}
export interface Author { name: string; color: string }

let seq = 0;
const newId = (prefix: string) => `${prefix}${Date.now().toString(36)}${(seq++).toString(36)}`;

const safeThread = (raw: string | null): ThreadEntry[] => {
  try {
    const v = JSON.parse(raw ?? '[]');
    return Array.isArray(v) ? v.filter((e) => e && typeof e.x === 'string').map((e) => ({ a: String(e.a ?? ''), t: Number(e.t) || 0, x: e.x })) : [];
  } catch { return []; }
};

// ── reading (works on any element / parsed HTML) ──────────────────────────

export function readComments(root: ParentNode): ReviewComment[] {
  return [...root.querySelectorAll<HTMLElement>('span.bd-comment')].map((el) => {
    const [first, ...replies] = safeThread(el.getAttribute('data-thread'));
    return {
      id: el.getAttribute('data-comment-id') ?? '',
      author: first?.a ?? '', ts: first?.t ?? 0, text: first?.x ?? '',
      quote: (el.textContent ?? '').slice(0, 160),
      resolved: el.getAttribute('data-resolved') === '1',
      replies,
    };
  }).filter((c) => c.id);
}

export function readChanges(root: ParentNode): ReviewChange[] {
  return [...root.querySelectorAll<HTMLElement>('ins.bd-ins, del.bd-del')].map((el) => ({
    id: el.getAttribute('data-change-id') ?? '',
    type: el.tagName === 'INS' ? 'insert' as const : 'delete' as const,
    author: el.getAttribute('data-author') ?? '',
    ts: Number(el.getAttribute('data-ts')) || 0,
    text: (el.textContent ?? '').slice(0, 160),
    color: el.getAttribute('data-color') ?? '',
  })).filter((c) => c.id);
}

/** Parses saved HTML into the review lists (used by the side panel; no live iframe needed). */
export function parseReview(html: string): { comments: ReviewComment[]; changes: ReviewChange[] } {
  if (!html || (!html.includes('bd-comment') && !html.includes('bd-ins') && !html.includes('bd-del'))) {
    return { comments: [], changes: [] };
  }
  const doc = new DOMParser().parseFromString(html, 'text/html');
  return { comments: readComments(doc.body), changes: readChanges(doc.body) };
}

const unwrap = (el: Element) => {
  const parent = el.parentNode;
  if (!parent) return;
  while (el.firstChild) parent.insertBefore(el.firstChild, el);
  parent.removeChild(el);
  parent.normalize(); // merge the text nodes that were split by the markup
};

const removeEl = (el: Element) => {
  const parent = el.parentNode;
  el.remove();
  parent?.normalize();
};

/** The document "as if every change were accepted and without comment anchors" — for export/print. */
export function acceptedHtml(html: string): string {
  if (!html.includes('bd-')) return html;
  const doc = new DOMParser().parseFromString(html, 'text/html');
  doc.body.querySelectorAll('del.bd-del').forEach(removeEl);
  doc.body.querySelectorAll('ins.bd-ins, span.bd-comment').forEach(unwrap);
  return doc.body.innerHTML;
}

// ── comments ──────────────────────────────────────────────────────────────

export function addCommentToRange(doc: Document, range: Range, author: Author, text: string, now = Date.now()): string | null {
  if (range.collapsed || !text.trim()) return null;
  const id = newId('c');
  const span = doc.createElement('span');
  span.className = 'bd-comment';
  span.setAttribute('data-comment-id', id);
  span.setAttribute('data-resolved', '0');
  span.setAttribute('data-thread', JSON.stringify([{ a: author.name, t: now, x: text.trim() }]));
  span.appendChild(range.extractContents());
  range.insertNode(span);
  return id;
}

const commentEl = (root: ParentNode, id: string) =>
  [...root.querySelectorAll<HTMLElement>('span.bd-comment')].find((e) => e.getAttribute('data-comment-id') === id) ?? null;

export function replyToComment(root: ParentNode, id: string, author: Author, text: string, now = Date.now()): boolean {
  const el = commentEl(root, id);
  if (!el || !text.trim()) return false;
  const thread = safeThread(el.getAttribute('data-thread'));
  thread.push({ a: author.name, t: now, x: text.trim() });
  el.setAttribute('data-thread', JSON.stringify(thread));
  return true;
}

export function setCommentResolved(root: ParentNode, id: string, resolved: boolean): boolean {
  const el = commentEl(root, id);
  if (!el) return false;
  el.setAttribute('data-resolved', resolved ? '1' : '0');
  return true;
}

/** Removes the comment but keeps the commented text. */
export function deleteComment(root: ParentNode, id: string): boolean {
  const el = commentEl(root, id);
  if (!el) return false;
  unwrap(el);
  return true;
}

// ── tracked changes ───────────────────────────────────────────────────────

const markup = (doc: Document, tag: 'ins' | 'del', author: Author, now: number) => {
  const el = doc.createElement(tag);
  el.className = tag === 'ins' ? 'bd-ins' : 'bd-del';
  el.setAttribute('data-change-id', newId('r'));
  el.setAttribute('data-author', author.name);
  el.setAttribute('data-ts', String(now));
  el.setAttribute('data-color', author.color);
  el.style.color = author.color;
  return el;
};

const closestIns = (node: Node | null, author: Author): HTMLElement | null => {
  let n: Node | null = node;
  while (n) {
    if (n.nodeType === 1 && (n as Element).matches('ins.bd-ins') && (n as Element).getAttribute('data-author') === author.name) return n as HTMLElement;
    n = n.parentNode;
  }
  return null;
};

const insideDel = (node: Node | null): boolean => {
  for (let n = node; n; n = n.parentNode) if (n.nodeType === 1 && (n as Element).matches('del.bd-del')) return true;
  return false;
};

function placeCaretAfter(doc: Document, node: Node, inside = false, before = false) {
  const sel = doc.getSelection();
  if (!sel) return;
  const r = doc.createRange();
  if (before) { r.setStartBefore(node); r.collapse(true); }
  else {
    if (inside) { r.selectNodeContents(node); r.collapse(false); } else r.setStartAfter(node);
    r.collapse(false);
  }
  sel.removeAllRanges();
  sel.addRange(r);
}

/** Marks the range as deleted. Text the same author inserted earlier is removed for real. */
export function trackedDeleteRange(doc: Document, range: Range, author: Author, now = Date.now(), caret: 'after' | 'before' = 'after'): boolean {
  if (range.collapsed) return false;
  // already marked as deleted: just move the caret across it
  if (insideDel(range.startContainer) && insideDel(range.endContainer)) {
    const sel = doc.getSelection();
    if (sel) { const r = range.cloneRange(); r.collapse(caret === 'before'); sel.removeAllRanges(); sel.addRange(r); }
    return true;
  }
  const own = closestIns(range.commonAncestorContainer, author);
  if (own && own.contains(range.startContainer) && own.contains(range.endContainer)) {
    range.deleteContents();
    if (!(own.textContent ?? '').length) removeEl(own); // nothing left of this insertion
    return true;
  }
  const frag = range.extractContents();
  // content already marked deleted stays as is; own insertions vanish; the rest is wrapped
  frag.querySelectorAll('ins.bd-ins').forEach((i) => { if (i.getAttribute('data-author') === author.name) i.remove(); });
  if (!(frag.textContent ?? '').length && !frag.querySelector('img,br')) return true;
  const del = markup(doc, 'del', author, now);
  del.appendChild(frag);
  range.insertNode(del);
  placeCaretAfter(doc, del, false, caret === 'before');
  return true;
}

/** Inserts text as a tracked insertion at the caret (replacing a non-collapsed selection as a tracked delete). */
export function trackedInsert(doc: Document, text: string, author: Author, now = Date.now()): boolean {
  const sel = doc.getSelection();
  if (!sel || sel.rangeCount === 0 || !text) return false;
  let range = sel.getRangeAt(0);
  if (!range.collapsed) {
    trackedDeleteRange(doc, range, author, now);
    const s2 = doc.getSelection();
    if (!s2 || s2.rangeCount === 0) return false;
    range = s2.getRangeAt(0);
  }
  // typing inside a deletion: continue after it, never inside it
  const del = insideDel(range.startContainer) ? (() => { let n: Node | null = range.startContainer; while (n && !(n.nodeType === 1 && (n as Element).matches('del.bd-del'))) n = n.parentNode; return n; })() : null;
  if (del) { range.setStartAfter(del); range.collapse(true); }

  const existing = closestIns(range.startContainer, author);
  const node = doc.createTextNode(text);
  if (existing) {
    range.insertNode(node);
    placeCaretAfter(doc, node);
  } else {
    const ins = markup(doc, 'ins', author, now);
    ins.appendChild(node);
    range.insertNode(ins);
    placeCaretAfter(doc, ins, true);
  }
  existing?.normalize();
  return true;
}

const changeEl = (root: ParentNode, id: string) =>
  [...root.querySelectorAll<HTMLElement>('ins.bd-ins, del.bd-del')].find((e) => e.getAttribute('data-change-id') === id) ?? null;

export function acceptChange(root: ParentNode, id: string): boolean {
  const el = changeEl(root, id);
  if (!el) return false;
  if (el.tagName === 'INS') unwrap(el); else removeEl(el);
  return true;
}
export function rejectChange(root: ParentNode, id: string): boolean {
  const el = changeEl(root, id);
  if (!el) return false;
  if (el.tagName === 'INS') removeEl(el); else unwrap(el);
  return true;
}
export function acceptAll(root: ParentNode): number {
  const all = readChanges(root); all.forEach((c) => acceptChange(root, c.id)); return all.length;
}
export function rejectAll(root: ParentNode): number {
  const all = readChanges(root); all.forEach((c) => rejectChange(root, c.id)); return all.length;
}

/** CSS for the review markup (injected into the editor iframe). */
export const REVIEW_CSS = `
  ins.bd-ins{text-decoration:underline;text-decoration-style:solid;background:rgba(59,130,246,.10)}
  del.bd-del{text-decoration:line-through;background:rgba(239,68,68,.10)}
  span.bd-comment{background:rgba(250,204,21,.28);border-bottom:2px solid #eab308;cursor:pointer}
  span.bd-comment[data-resolved="1"]{background:transparent;border-bottom:1px dotted #94a3b8}
  span.bd-comment.bd-flash{outline:2px solid #3b82f6}
`;

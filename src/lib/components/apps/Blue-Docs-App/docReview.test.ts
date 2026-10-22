import { describe, it, expect } from 'vitest';
import {
  addCommentToRange, readComments, replyToComment, setCommentResolved, deleteComment, trackedInsert, trackedDeleteRange,
  readChanges, acceptChange, rejectChange, acceptAll, rejectAll, acceptedHtml, parseReview, type Author,
} from './docReview';

const ann: Author = { name: 'Ann', color: '#3b82f6' };
const bob: Author = { name: 'Bob', color: '#ef4444' };

function setup(html: string) {
  document.body.innerHTML = html;
  return document;
}
function selectText(node: Node, from: number, to: number) {
  const r = document.createRange();
  r.setStart(node, from); r.setEnd(node, to);
  const s = document.getSelection()!;
  s.removeAllRanges(); s.addRange(r);
  return r;
}

describe('comments', () => {
  it('anchors a comment to the selected text and reads it back', () => {
    setup('<p>Hello brave new world</p>');
    const text = document.querySelector('p')!.firstChild!;
    const id = addCommentToRange(document, selectText(text, 6, 11), ann, 'Is this word right?', 1000)!;
    expect(id).toBeTruthy();
    expect(document.body.innerHTML).toContain('<span class="bd-comment"');
    const [c] = readComments(document.body);
    expect(c).toMatchObject({ id, author: 'Ann', text: 'Is this word right?', quote: 'brave', resolved: false, ts: 1000 });
    expect(document.querySelector('p')!.textContent).toBe('Hello brave new world'); // text unchanged
  });
  it('refuses empty selections / empty comments', () => {
    setup('<p>abc</p>');
    const t = document.querySelector('p')!.firstChild!;
    expect(addCommentToRange(document, selectText(t, 1, 1), ann, 'x')).toBeNull();
    expect(addCommentToRange(document, selectText(t, 0, 2), ann, '   ')).toBeNull();
  });
  it('replies, resolves and deletes (keeping the text)', () => {
    setup('<p>Hello world</p>');
    const id = addCommentToRange(document, selectText(document.querySelector('p')!.firstChild!, 0, 5), ann, 'Hi?')!;
    expect(replyToComment(document.body, id, bob, 'Fixed', 2000)).toBe(true);
    expect(readComments(document.body)[0].replies).toEqual([{ a: 'Bob', t: 2000, x: 'Fixed' }]);
    expect(setCommentResolved(document.body, id, true)).toBe(true);
    expect(readComments(document.body)[0].resolved).toBe(true);
    expect(deleteComment(document.body, id)).toBe(true);
    expect(readComments(document.body)).toHaveLength(0);
    expect(document.body.textContent).toBe('Hello world');
    expect(replyToComment(document.body, 'nope', bob, 'x')).toBe(false);
  });
  it('survives a serialise → parse round trip', () => {
    setup('<p>Quote me</p>');
    addCommentToRange(document, selectText(document.querySelector('p')!.firstChild!, 0, 5), ann, 'a "quoted" <b>comment</b>');
    const review = parseReview(document.body.innerHTML);
    expect(review.comments[0].text).toBe('a "quoted" <b>comment</b>');
  });
});

describe('tracked changes', () => {
  it('records typing as one insertion per run', () => {
    setup('<p>Hello </p>');
    const t = document.querySelector('p')!.firstChild!;
    selectText(t, 6, 6);
    trackedInsert(document, 'wor', ann, 5);
    trackedInsert(document, 'ld', ann, 6);
    const ch = readChanges(document.body);
    expect(ch).toHaveLength(1);
    expect(ch[0]).toMatchObject({ type: 'insert', author: 'Ann', text: 'world' });
    expect(document.querySelector('p')!.textContent).toBe('Hello world');
  });
  it('records deletion without removing the text', () => {
    setup('<p>Hello cruel world</p>');
    trackedDeleteRange(document, selectText(document.querySelector('p')!.firstChild!, 5, 11), ann, 7);
    const ch = readChanges(document.body);
    expect(ch).toHaveLength(1);
    expect(ch[0]).toMatchObject({ type: 'delete', text: ' cruel' });
    expect(document.querySelector('p')!.textContent).toBe('Hello cruel world');
    expect(document.querySelector('del')).toBeTruthy();
  });
  it('replacing a selection = delete + insert', () => {
    setup('<p>Hello cruel world</p>');
    selectText(document.querySelector('p')!.firstChild!, 6, 11);
    trackedInsert(document, 'kind', ann, 9);
    expect(readChanges(document.body).map((c) => c.type).sort()).toEqual(['delete', 'insert']);
    expect(acceptedHtml(document.body.innerHTML)).toBe('<p>Hello kind world</p>');
  });
  it('deleting your own fresh insertion removes it for real', () => {
    setup('<p>ab</p>');
    selectText(document.querySelector('p')!.firstChild!, 2, 2);
    trackedInsert(document, 'XYZ', ann);
    const inner = document.querySelector('ins')!.firstChild!;
    trackedDeleteRange(document, selectText(inner, 0, 3), ann);
    expect(readChanges(document.body)).toHaveLength(0);
    expect(document.body.textContent).toBe('ab');
  });
  it("another author's insertion is deleted as a tracked change, not removed", () => {
    setup('<p>ab</p>');
    selectText(document.querySelector('p')!.firstChild!, 2, 2);
    trackedInsert(document, 'XYZ', bob);
    const inner = document.querySelector('ins')!.firstChild!;
    trackedDeleteRange(document, selectText(inner, 0, 3), ann);
    expect(readChanges(document.body).some((c) => c.type === 'delete' && c.author === 'Ann')).toBe(true);
  });
  it('accept / reject individually and in bulk', () => {
    setup('<p>Hello cruel world</p>');
    trackedDeleteRange(document, selectText(document.querySelector('p')!.firstChild!, 5, 11), ann);
    const id = readChanges(document.body)[0].id;
    expect(rejectChange(document.body, id)).toBe(true);
    expect(document.body.innerHTML).toBe('<p>Hello cruel world</p>');

    trackedDeleteRange(document, selectText(document.querySelector('p')!.firstChild!, 5, 11), ann);
    expect(acceptChange(document.body, readChanges(document.body)[0].id)).toBe(true);
    expect(document.body.textContent).toBe('Hello world');

    selectText(document.querySelector('p')!.firstChild!, 5, 5);
    trackedInsert(document, '!', ann);
    expect(rejectAll(document.body)).toBe(1);
    expect(document.body.textContent).toBe('Hello world');
    selectText(document.querySelector('p')!.firstChild!, 5, 5);
    trackedInsert(document, '?', ann);
    expect(acceptAll(document.body)).toBe(1);
    expect(document.body.textContent).toBe('Hello? world');
    expect(readChanges(document.body)).toHaveLength(0);
  });
});

describe('export helpers', () => {
  it('acceptedHtml strips deletions, unwraps insertions and comment anchors', () => {
    const html = '<p>A <del class="bd-del" data-change-id="r1">gone </del><ins class="bd-ins" data-change-id="r2">new </ins><span class="bd-comment" data-comment-id="c1" data-thread="[]">note</span></p>';
    expect(acceptedHtml(html)).toBe('<p>A new note</p>');
    expect(acceptedHtml('<p>plain</p>')).toBe('<p>plain</p>');
  });
  it('parseReview is cheap for plain documents', () => {
    expect(parseReview('<p>nothing here</p>')).toEqual({ comments: [], changes: [] });
  });
});

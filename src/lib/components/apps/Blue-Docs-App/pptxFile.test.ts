import { describe, it, expect } from 'vitest';
import { writeFileSync } from 'node:fs';
import JSZip from 'jszip';
import { exportPptxBytes, importPptx, textColorFor, parseNotesXml } from './pptxFile';
import type { Presentation } from './types';

// 1×1 transparent PNG
const PNG = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';

const deck = (): Presentation => ({
  slides: [
    { id: 's1', title: 'Quarterly **review**', bullets: ['Revenue up *12%*', 'Costs flat'], background: '#0f172a', images: [], notes: 'Say hello.\nMention Q3.' },
    { id: 's2', title: 'Numbers', bullets: ['See table'], background: '#ffffff', images: [PNG], table: [['Region', 'Sales'], ['EU', '120'], ['US', '95']] },
    { id: 's3', title: 'No extras', bullets: [], background: '#1e293b', images: [] },
  ],
});

const toBuffer = (u: Uint8Array) => u.buffer.slice(u.byteOffset, u.byteOffset + u.byteLength) as ArrayBuffer;

describe('pptx round trip', () => {
  it('keeps titles, bullets, formatting, images, tables and speaker notes', async () => {
    const bytes = await exportPptxBytes(deck());
    if (process.env.BLUE_PPTX_OUT) writeFileSync(process.env.BLUE_PPTX_OUT, bytes);
    const back = await importPptx(toBuffer(bytes));
    expect(back.slides).toHaveLength(3);
    expect(back.slides[0].title).toBe('Quarterly **review**');
    expect(back.slides[0].bullets).toEqual(['Revenue up *12%*', 'Costs flat']);
    expect(back.slides[0].notes).toBe('Say hello.\nMention Q3.');
    expect(back.slides[1].table).toEqual([['Region', 'Sales'], ['EU', '120'], ['US', '95']]);
    expect(back.slides[1].images).toHaveLength(1);
    expect(back.slides[1].notes).toBeUndefined();
    expect(back.slides[2].table).toBeUndefined();
    expect(back.slides[0].background).toBe('#0f172a');
  });

  it('only writes the notes parts when some slide has notes', async () => {
    const plain = await JSZip.loadAsync(await exportPptxBytes({ slides: [deck().slides[2]] }));
    expect(plain.file('ppt/notesMasters/notesMaster1.xml')).toBeNull();
    const withNotes = await JSZip.loadAsync(await exportPptxBytes(deck()));
    expect(withNotes.file('ppt/notesMasters/notesMaster1.xml')).not.toBeNull();
    expect(withNotes.file('ppt/notesSlides/notesSlide1.xml')).not.toBeNull();
    expect(withNotes.file('ppt/notesSlides/notesSlide2.xml')).toBeNull();
    const ct = await withNotes.file('[Content_Types].xml')!.async('string');
    expect(ct).toContain('/ppt/notesSlides/notesSlide1.xml');
    expect(ct).not.toContain('notesSlide2');
    expect(ct).not.toContain('{{');
  });

  it('writes placeholders with explicit geometry and readable text colour', async () => {
    const zip = await JSZip.loadAsync(await exportPptxBytes(deck()));
    const dark = await zip.file('ppt/slides/slide1.xml')!.async('string');
    expect(dark).toMatch(/<p:ph type="title"\/>[\s\S]*?<a:off x="\d+" y="\d+"\/><a:ext cx="\d+" cy="\d+"\/>/);
    expect(dark).toContain('<a:srgbClr val="F8FAFC"/>'); // light text on a dark slide
    const light = await zip.file('ppt/slides/slide2.xml')!.async('string');
    expect(light).toContain('<a:srgbClr val="0F172A"/>'); // dark text on a white slide
    expect(light).toContain('<a:tbl>');
  });

  it('picks text colour by luminance', () => {
    expect(textColorFor('#000000')).toBe('F8FAFC');
    expect(textColorFor('#ffffff')).toBe('0F172A');
    expect(textColorFor('#3b82f6')).toBe('F8FAFC');
  });

  it('parses notes bodies only', () => {
    const xml = '<p:notes xmlns:a="a" xmlns:p="p"><p:sp><p:nvSpPr><p:nvPr><p:ph type="sldNum"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>7</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Real notes</a:t></a:r></a:p><a:p/></p:txBody></p:sp></p:notes>';
    expect(parseNotesXml(xml)).toBe('Real notes');
  });

  it('rejects files that are not presentations', async () => {
    const z = new JSZip(); z.file('hello.txt', 'x');
    await expect(importPptx(toBuffer(await z.generateAsync({ type: 'uint8array' })))).rejects.toThrow(/not a valid/i);
  });
});

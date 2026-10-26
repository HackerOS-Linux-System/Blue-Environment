import { describe, it, expect } from 'vitest';
import { writeFileSync } from 'node:fs';
import JSZip from 'jszip';
import { exportPptxBytes, importPptx, textColorFor, parseNotesXml, transitionXml } from './pptxFile';
import { normalizeSlide, newImageBox, newTextBox, newSlide, resolveRect, LAYOUTS } from './slideModel';
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
    expect(back.slides[1].images).toEqual([]); // pictures live in `elements` now
    expect(back.slides[1].elements?.filter((e) => e.kind === 'image')).toHaveLength(1);
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

  it('keeps layouts, text boxes, styles, picture positions and transitions', async () => {
    const base = newSlide('two-content', 1);
    const rich: Presentation = {
      slides: [
        {
          ...base, id: 'r1', title: 'Two columns', bullets: ['Left one', 'Left **two**'], bullets2: ['Right one'], background: '#ffffff',
          titleStyle: { fontFamily: 'Georgia', fontSize: 40, color: '#ff0000', align: 'center' },
          bodyStyle: { fontSize: 20, italic: true },
          elements: [
            { kind: 'text', id: 't1', x: 10, y: 80, w: 30, h: 10, text: 'Footer note\nsecond line', style: { fontSize: 14, color: '#112233', align: 'right', bold: true, bullets: false }, fill: '#ffee00' },
            { kind: 'image', id: 'i1', x: 55, y: 60, w: 20, h: 25, src: PNG },
          ],
          transition: { type: 'push', direction: 'u', durationMs: 1200, advanceAfterMs: 4000 },
        },
        { ...newSlide('blank', 2), id: 'r2', transition: { type: 'none', advanceAfterMs: 2500 } },
        { ...newSlide('title', 3), id: 'r3', title: 'Hello', bullets: ['Subtitle'], rects: { title: { x: 5, y: 10, w: 90, h: 30 } } },
      ],
    };
    const back = await importPptx(toBuffer(await exportPptxBytes(rich)));
    expect(back.slides).toHaveLength(3);

    const s = back.slides[0];
    expect(s.layout).toBe('two-content');
    expect(s.bullets).toEqual(['Left one', 'Left **two**']);
    expect(s.bullets2).toEqual(['Right one']);
    expect(s.titleStyle).toMatchObject({ fontFamily: 'Georgia', fontSize: 40, color: '#ff0000', align: 'center' });
    expect(s.bodyStyle).toMatchObject({ fontSize: 20, italic: true });

    const text = s.elements!.find((e) => e.kind === 'text') as Extract<NonNullable<typeof s.elements>[number], { kind: 'text' }>;
    expect(text.text).toBe('Footer note\nsecond line');
    expect(text).toMatchObject({ fill: '#ffee00' });
    expect(text.style).toMatchObject({ fontSize: 14, color: '#112233', align: 'right', bold: true });
    expect(text.x).toBeCloseTo(10, 0);
    expect(text.w).toBeCloseTo(30, 0);

    const pic = s.elements!.find((e) => e.kind === 'image')!;
    expect(pic.x).toBeCloseTo(55, 0);
    expect(pic.y).toBeCloseTo(60, 0);
    expect(pic.w).toBeCloseTo(20, 0);
    expect(pic.h).toBeCloseTo(25, 0);

    expect(s.transition).toEqual({ type: 'push', direction: 'u', durationMs: 1200, advanceAfterMs: 4000 });
    expect(back.slides[1].layout).toBe('blank');
    expect(back.slides[1].transition).toEqual({ type: 'none', advanceAfterMs: 2500 });
    expect(back.slides[2].layout).toBe('title');
    expect(back.slides[2].rects?.title).toMatchObject({ x: 5, y: 10, w: 90, h: 30 });
  });

  it('writes <p:transition> after clrMapOvr and nothing when there is none', async () => {
    expect(transitionXml(undefined)).toBe('');
    expect(transitionXml({ type: 'fade', durationMs: 300 })).toBe('<p:transition spd="fast"><p:fade/></p:transition>');
    expect(transitionXml({ type: 'wipe', direction: 'r', durationMs: 800 })).toContain('<p:wipe dir="r"/>');
    const zip = await JSZip.loadAsync(await exportPptxBytes({ slides: [{ ...newSlide('title-only', 0), transition: { type: 'zoom' } }] }));
    const xml = await zip.file('ppt/slides/slide1.xml')!.async('string');
    expect(xml.indexOf('<p:clrMapOvr>')).toBeLessThan(xml.indexOf('<p:transition'));
  });

  it('migrates the old flat picture list into positioned pictures', () => {
    const old = normalizeSlide({ id: 'o', title: 'Old', bullets: [], background: '#0f172a', images: [PNG, PNG] });
    expect(old.images).toEqual([]);
    expect(old.elements).toHaveLength(2);
    expect(old.elements![0].x).toBeLessThan(old.elements![1].x);
    expect(normalizeSlide(old).elements).toHaveLength(2); // idempotent
    expect(old.layout).toBe('title-content');
  });

  it('places new elements inside the slide and honours layout geometry', () => {
    const slide = newSlide('content-image', 0);
    const img = newImageBox(slide, PNG, 2);
    expect(img.x).toBeGreaterThanOrEqual(0);
    expect(img.x + img.w).toBeLessThanOrEqual(100);
    expect(img.y + img.h).toBeLessThanOrEqual(100);
    const box = newTextBox(slide);
    expect(box.kind).toBe('text');
    expect(resolveRect(slide, 'body')).toEqual(LAYOUTS['content-image'].body);
    expect(resolveRect({ ...slide, rects: { body: { x: 1, y: 2, w: 3, h: 4 } } }, 'body')).toMatchObject({ x: 1, y: 2 });
    expect(resolveRect(newSlide('blank', 0), 'title')).toBeNull();
  });

  it('opens a PowerPoint-style file that has a 4:3 size and no explicit placeholder geometry', async () => {
    const zip = await JSZip.loadAsync(await exportPptxBytes({ slides: [{ ...newSlide('title-content', 0), id: 'q', title: 'T', bullets: ['a'] }] }));
    let pres = await zip.file('ppt/presentation.xml')!.async('string');
    pres = pres.replace(/<p:sldSz[^>]*\/>/, '<p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>');
    zip.file('ppt/presentation.xml', pres);
    const back = await importPptx(toBuffer(await zip.generateAsync({ type: 'uint8array' })));
    expect(back.slides[0].title).toBe('T');
    expect(back.slides[0].bullets).toEqual(['a']);
  });
});

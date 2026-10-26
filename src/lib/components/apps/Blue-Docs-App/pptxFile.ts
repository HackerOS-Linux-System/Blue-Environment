import JSZip from 'jszip';
import type { Presentation, Rect, Slide, SlideElement, SlideLayoutId, SlideTextStyle, SlideTransition, TransitionType } from './types';
import {
  LAYOUTS, SLIDE_H, SLIDE_W, clampRect, hexColor, isHexColor, layoutOf, newId, normalizeSlide, normalizeTransition,
  pictureSlot, rectsDiffer, resolveBoxStyle, resolveRect, resolveStyle, round1, textColorFor,
} from './slideModel';

export { textColorFor };

const EMU_PER_INCH = 914400;
const SLIDE_WIDTH_IN = 13.333; // 16:9, matches PowerPoint's modern default
const SLIDE_HEIGHT_IN = 7.5;
const SLIDE_WIDTH_EMU = Math.round(SLIDE_WIDTH_IN * EMU_PER_INCH);
const SLIDE_HEIGHT_EMU = Math.round(SLIDE_HEIGHT_IN * EMU_PER_INCH);

function xmlEscape(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');
}

/** Parses this app's `**bold**`/`*italic*` markdown-lite into a flat
 * list of `{ text, bold, italic }` runs — shared by the OOXML writer
 * (turns each run into an `<a:r>`) and, in principle, could be reused
 * by a plain-text renderer; kept simple/regex-based since the format
 * is intentionally tiny (no nesting, no escaping beyond the two
 * markers). */
export interface FormattedRun { text: string; bold: boolean; italic: boolean }

export function parseInlineFormatting(line: string): FormattedRun[] {
  const runs: FormattedRun[] = [];
  const pattern = /(\*\*(.+?)\*\*|\*(.+?)\*)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(line))) {
    if (match.index > lastIndex) {
      runs.push({ text: line.slice(lastIndex, match.index), bold: false, italic: false });
    }
    if (match[2] !== undefined) {
      runs.push({ text: match[2], bold: true, italic: false });
    } else if (match[3] !== undefined) {
      runs.push({ text: match[3], bold: false, italic: true });
    }
    lastIndex = pattern.lastIndex;
  }
  if (lastIndex < line.length) {
    runs.push({ text: line.slice(lastIndex), bold: false, italic: false });
  }
  return runs.length > 0 ? runs : [{ text: '', bold: false, italic: false }];
}

/** Inverse of `parseInlineFormatting`, for writing formatted OOXML runs
 * back out as this app's plain-string bullet format on import. */
function runsToMarkdownLite(runs: FormattedRun[]): string {
  return runs
    .map((r) => {
      if (r.bold) return `**${r.text}**`;
      if (r.italic) return `*${r.text}*`;
      return r.text;
    })
    .join('');
}

type ResolvedStyle = ReturnType<typeof resolveStyle>;

const ALGN: Record<ResolvedStyle['align'], string> = { left: 'l', center: 'ctr', right: 'r', justify: 'just' };
const ALGN_BACK: Record<string, ResolvedStyle['align']> = { l: 'left', ctr: 'center', r: 'right', just: 'justify' };

/** One `<a:r>`. Shape-wide bold/italic/underline live in the shape's list style
 * (see `lstStyleXml`), so run-level `b`/`i` always mean "inline emphasis". */
function runXml(run: FormattedRun, style?: { color: string; size: number; font?: string }, forceBold = false): string {
  const props: string[] = [];
  if (style) props.push(`sz="${Math.round(style.size * 100)}"`);
  if (run.bold || forceBold) props.push('b="1"');
  if (run.italic) props.push('i="1"');
  const attrs = props.length > 0 ? ` ${props.join(' ')}` : '';
  const inner = style
    ? `<a:solidFill><a:srgbClr val="${hexColor(style.color)}"/></a:solidFill>${style.font ? `<a:latin typeface="${xmlEscape(style.font)}"/>` : ''}`
    : '';
  const rPr = inner ? `<a:rPr lang="en-US"${attrs} dirty="0">${inner}</a:rPr>` : `<a:rPr lang="en-US"${attrs} dirty="0"/>`;
  return `<a:r>${rPr}<a:t>${xmlEscape(run.text)}</a:t></a:r>`;
}

function lstStyleXml(st: ResolvedStyle): string {
  const props = [st.bold ? 'b="1"' : '', st.italic ? 'i="1"' : '', st.underline ? 'u="sng"' : ''].filter(Boolean).join(' ');
  return props ? `<a:lstStyle><a:lvl1pPr><a:defRPr ${props}/></a:lvl1pPr></a:lstStyle>` : '<a:lstStyle/>';
}

function paragraphXml(line: string, st: ResolvedStyle): string {
  const algn = ALGN[st.align];
  const pPr = st.bullets
    ? `<a:pPr marL="285750" indent="-285750" algn="${algn}"><a:buChar char="•"/></a:pPr>`
    : `<a:pPr marL="0" indent="0" algn="${algn}"><a:buNone/></a:pPr>`;
  const runs = parseInlineFormatting(line).map((r) => runXml(r, { color: st.color, size: st.fontSize, font: st.fontFamily })).join('');
  return `<a:p>${pPr}${runs}</a:p>`;
}

const xfrm = (x: number, y: number, w: number, h: number) =>
  `<a:xfrm><a:off x="${Math.round(x)}" y="${Math.round(y)}"/><a:ext cx="${Math.round(w)}" cy="${Math.round(h)}"/></a:xfrm>`;

/** Native table (<a:tbl>) inside a graphicFrame. */
function tableXml(rows: string[][], x: number, y: number, w: number, rowH: number, color: string, id: number): string {
  const cols = Math.max(1, ...rows.map((r) => r.length));
  const colW = Math.round(w / cols);
  const cell = (text: string, header: boolean) => {
    const paras = (text || '').split('\n').map((line) =>
      `<a:p>${parseInlineFormatting(line).map((r) => runXml(r, { color, size: 16 }, header)).join('') || `<a:endParaRPr lang="en-US" sz="1600"/>`}</a:p>`).join('');
    const border = ['lnL', 'lnR', 'lnT', 'lnB'].map((t) => `<a:${t} w="9525"><a:solidFill><a:srgbClr val="${color}"><a:alpha val="45000"/></a:srgbClr></a:solidFill></a:${t}>`).join('');
    return `<a:tc><a:txBody><a:bodyPr/><a:lstStyle/>${paras}</a:txBody><a:tcPr>${border}<a:noFill/></a:tcPr></a:tc>`;
  };
  const trs = rows.map((r, ri) => {
    const cells = Array.from({ length: cols }, (_, ci) => cell(r[ci] ?? '', ri === 0)).join('');
    return `<a:tr h="${rowH}">${cells}</a:tr>`;
  }).join('');
  const grid = Array.from({ length: cols }, () => `<a:gridCol w="${colW}"/>`).join('');
  return `<p:graphicFrame>
        <p:nvGraphicFramePr><p:cNvPr id="${id}" name="Table"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>
        <p:xfrm><a:off x="${Math.round(x)}" y="${Math.round(y)}"/><a:ext cx="${colW * cols}" cy="${rowH * rows.length}"/></p:xfrm>
        <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1"/><a:tblGrid>${grid}</a:tblGrid>${trs}</a:tbl></a:graphicData></a:graphic>
      </p:graphicFrame>`;
}

const pctX = (v: number) => (v / 100) * SLIDE_WIDTH_EMU;
const pctY = (v: number) => (v / 100) * SLIDE_HEIGHT_EMU;
const rectXfrm = (r: Rect) => xfrm(pctX(r.x), pctY(r.y), pctX(r.w), pctY(r.h));

function placeholderSp(id: number, name: string, ph: string, rect: Rect, st: ResolvedStyle, lines: string[], anchor: string): string {
  const paras = lines.length ? lines.map((l) => paragraphXml(l, st)).join('') : '<a:p/>';
  return `<p:sp>
        <p:nvSpPr>
          <p:cNvPr id="${id}" name="${name}"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph ${ph}/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>${rectXfrm(rect)}</p:spPr>
        <p:txBody>
          <a:bodyPr anchor="${anchor}"><a:normAutofit/></a:bodyPr>
          ${lstStyleXml(st)}
          ${paras}
        </p:txBody>
      </p:sp>`;
}

function textBoxSp(id: number, box: Extract<SlideElement, { kind: 'text' }>, st: ResolvedStyle): string {
  const lines = box.text.split('\n');
  const fill = box.fill && isHexColor(box.fill) ? `<a:solidFill><a:srgbClr val="${hexColor(box.fill)}"/></a:solidFill>` : '<a:noFill/>';
  return `<p:sp>
        <p:nvSpPr>
          <p:cNvPr id="${id}" name="Text ${id}"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>${rectXfrm(box)}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>${fill}</p:spPr>
        <p:txBody>
          <a:bodyPr wrap="square" anchor="t"><a:normAutofit/></a:bodyPr>
          ${lstStyleXml(st)}
          ${lines.map((l) => paragraphXml(l, st)).join('')}
        </p:txBody>
      </p:sp>`;
}

function pictureSp(id: number, relId: string, rect: Rect): string {
  return `<p:pic>
          <p:nvPicPr>
            <p:cNvPr id="${id}" name="Picture ${id}"/>
            <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
            <p:nvPr/>
          </p:nvPicPr>
          <p:blipFill>
            <a:blip r:embed="${relId}"/>
            <a:stretch><a:fillRect/></a:stretch>
          </p:blipFill>
          <p:spPr>
            ${rectXfrm(rect)}
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
        </p:pic>`;
}

const TRANSITION_XML: Record<Exclude<TransitionType, 'none'>, (dir: string) => string> = {
  fade: () => '<p:fade/>',
  push: (d) => `<p:push dir="${d}"/>`,
  wipe: (d) => `<p:wipe dir="${d}"/>`,
  cover: (d) => `<p:cover dir="${d}"/>`,
  zoom: () => '<p:zoom dir="in"/>',
  split: () => '<p:split orient="vert" dir="out"/>',
};

/** `<p:transition>` (goes right after `<p:clrMapOvr>`). */
export function transitionXml(t: SlideTransition | undefined): string {
  const tr = normalizeTransition(t);
  if (!tr) return '';
  const ms = tr.durationMs ?? 600;
  const spd = ms <= 400 ? 'fast' : ms <= 900 ? 'med' : 'slow';
  const adv = tr.advanceAfterMs ? ` advTm="${tr.advanceAfterMs}"` : '';
  if (tr.type === 'none') return `<p:transition${adv}/>`;
  return `<p:transition spd="${spd}"${adv}>${TRANSITION_XML[tr.type](tr.direction ?? 'l')}</p:transition>`;
}

/** `relIds` maps a picture element's id to its relationship id in the slide's .rels file. */
function slideXml(rawSlide: Slide, relIds: Map<string, string>): string {
  const slide = normalizeSlide(rawSlide);
  const layout = layoutOf(slide);
  const color = textColorFor(slide.background);

  const shapes: string[] = [];
  let nextId = 2;

  const titleRect = resolveRect(slide, 'title');
  if (titleRect) {
    const st = resolveStyle(slide, 'title');
    shapes.push(placeholderSp(nextId++, 'Title', slide.layout === 'title' ? 'type="ctrTitle"' : 'type="title"', titleRect, st, slide.title ? [slide.title] : [], 'ctr'));
  }
  const bodyRect = resolveRect(slide, 'body');
  if (bodyRect) {
    const st = resolveStyle(slide, 'body');
    const lines = slide.bullets.filter((b) => b.trim().length > 0);
    shapes.push(placeholderSp(nextId++, 'Content', slide.layout === 'title' || slide.layout === 'section' ? 'type="subTitle" idx="1"' : 'idx="1"', bodyRect, st, lines, 't'));
  }
  const body2Rect = resolveRect(slide, 'body2');
  if (body2Rect) {
    const st = resolveStyle(slide, 'body2');
    const lines = (slide.bullets2 ?? []).filter((b) => b.trim().length > 0);
    shapes.push(placeholderSp(nextId++, 'Content 2', 'sz="half" idx="2"', body2Rect, st, lines, 't'));
  }

  const table = slide.table && slide.table.length > 0 ? slide.table : null;
  const tableRect = table ? resolveRect(slide, 'table') : null;
  if (table && tableRect) {
    const rowH = Math.round(pctY(tableRect.h) / table.length);
    shapes.push(tableXml(table, pctX(tableRect.x), pctY(tableRect.y), pctX(tableRect.w), rowH, color, nextId++));
  }

  for (const el of slide.elements ?? []) {
    if (el.kind === 'text') {
      shapes.push(textBoxSp(nextId++, el, resolveBoxStyle(slide, el)));
    } else {
      const relId = relIds.get(el.id);
      if (relId) shapes.push(pictureSp(nextId++, relId, el));
    }
  }
  void layout;

  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:bg>
      <p:bgPr>
        <a:solidFill><a:srgbClr val="${hexColor(slide.background)}"/></a:solidFill>
        <a:effectLst/>
      </p:bgPr>
    </p:bg>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm>
      </p:grpSpPr>
      ${shapes.join('\n      ')}
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
  ${transitionXml(slide.transition)}
</p:sld>`;
}

// ── speaker notes (notesMaster + one notesSlide per slide that has notes) ──

const NS = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"';
const GRP = '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>';

const NOTES_MASTER_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notesMaster ${NS}>
  <p:cSld>
    <p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>
    <p:spTree>
      ${GRP}
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr>
        <p:spPr>${xfrm(685800, 1143000, 5486400, 3086100)}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr>
      </p:sp>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr>
        <p:spPr>${xfrm(685800, 4400550, 5486400, 3600450)}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:notesStyle><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle>
</p:notesMaster>`;

const NOTES_MASTER_RELS_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme2.xml"/>
</Relationships>`;

function notesSlideXml(notes: string): string {
  const paras = notes.split('\n').map((l) => `<a:p>${l ? `<a:r><a:rPr lang="en-US" dirty="0"/><a:t>${xmlEscape(l)}</a:t></a:r>` : '<a:endParaRPr lang="en-US"/>'}</a:p>`).join('');
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes ${NS}>
  <p:cSld>
    <p:spTree>
      ${GRP}
      <p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>
      <p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>${paras}</p:txBody></p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:notes>`;
}

const notesSlideRels = (n: number) => `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide${n}.xml"/>
</Relationships>`;

const CONTENT_TYPES_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Default Extension="jpg" ContentType="image/jpeg"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  {{SLIDE_OVERRIDES}}
  {{NOTES_OVERRIDES}}
</Types>`;

const ROOT_RELS_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>`;

const THEME_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Blue Docs">
  <a:themeElements>
    <a:clrScheme name="Blue Docs">
      <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
      <a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="0F172A"/></a:dk2>
      <a:lt2><a:srgbClr val="E2E8F0"/></a:lt2>
      <a:accent1><a:srgbClr val="3B82F6"/></a:accent1>
      <a:accent2><a:srgbClr val="6366F1"/></a:accent2>
      <a:accent3><a:srgbClr val="10B981"/></a:accent3>
      <a:accent4><a:srgbClr val="F59E0B"/></a:accent4>
      <a:accent5><a:srgbClr val="EF4444"/></a:accent5>
      <a:accent6><a:srgbClr val="EC4899"/></a:accent6>
      <a:hlink><a:srgbClr val="3B82F6"/></a:hlink>
      <a:folHlink><a:srgbClr val="8B5CF6"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Blue Docs">
      <a:majorFont><a:latin typeface="Calibri"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Blue Docs">
      <a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
      <a:lnStyleLst><a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
      <a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>
      <a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>`;

const SLIDE_MASTER_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>
</p:sldMaster>`;

const SLIDE_MASTER_RELS_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>`;

const SLIDE_LAYOUT_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="titleOnly" preserve="1">
  <p:cSld name="Title and Content">
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    </p:spTree>
  </p:cSld>
</p:sldLayout>`;

const SLIDE_LAYOUT_RELS_XML = `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>`;

function presentationXml(slideCount: number, hasNotes = false): string {
  const sldIdLst = Array.from({ length: slideCount }, (_, i) => `<p:sldId id="${256 + i}" r:id="rIdSlide${i + 1}"/>`).join('');
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rIdMaster"/></p:sldMasterIdLst>
  ${hasNotes ? '<p:notesMasterIdLst><p:notesMasterId r:id="rIdNotesMaster"/></p:notesMasterIdLst>' : ''}
  <p:sldIdLst>${sldIdLst}</p:sldIdLst>
  <p:sldSz cx="${SLIDE_WIDTH_EMU}" cy="${SLIDE_HEIGHT_EMU}" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>`;
}

function presentationRelsXml(slideCount: number, hasNotes = false): string {
  const slideRels = Array.from(
    { length: slideCount },
    (_, i) => `<Relationship Id="rIdSlide${i + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide${i + 1}.xml"/>`
  ).join('');
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdMaster" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rIdTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  ${hasNotes ? '<Relationship Id="rIdNotesMaster" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/>' : ''}
  ${slideRels}
</Relationships>`;
}

function dataUrlToBytesAndExt(dataUrl: string): { bytes: Uint8Array; ext: string } {
  const match = dataUrl.match(/^data:image\/(png|jpeg|jpg);base64,(.*)$/);
  const ext = match ? (match[1] === 'jpg' ? 'jpeg' : match[1]) : 'png';
  const base64 = match ? match[2] : dataUrl.split(',')[1] ?? '';
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return { bytes, ext };
}

/** Builds a real `.pptx` file from this app's `Presentation` model. */
export async function exportPptx(presentation: Presentation): Promise<Blob> {
  const bytes = await exportPptxBytes(presentation);
  return new Blob([bytes as BlobPart], { type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation' });
}

/** Same as `exportPptx`, as raw bytes (used by tests and anywhere a Blob isn't convenient). */
export async function exportPptxBytes(presentation: Presentation): Promise<Uint8Array> {
  const zip = new JSZip();
  const slides = presentation.slides.map(normalizeSlide);

  // Collect every image across every slide into a shared ppt/media/
  // pool (OOXML doesn't require images be de-duplicated, but there's
  // no reason to embed the same bytes twice if the same image somehow
  // appears on two slides).
  let mediaIndex = 1;
  const slideRelsXmls: string[] = [];

  const slideRelIds: Map<string, string>[] = [];

  for (const slide of slides) {
    const relIds = new Map<string, string>();
    const relEntries: string[] = [];
    for (const el of slide.elements ?? []) {
      if (el.kind !== 'image') continue;
      const { bytes, ext } = dataUrlToBytesAndExt(el.src);
      const mediaName = `image${mediaIndex}.${ext}`;
      zip.file(`ppt/media/${mediaName}`, bytes);
      const relId = `rIdImg${mediaIndex}`;
      relIds.set(el.id, relId);
      relEntries.push(`<Relationship Id="${relId}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/${mediaName}"/>`);
      mediaIndex++;
    }
    slideRelIds.push(relIds);
    if (slide.notes && slide.notes.trim()) {
      relEntries.push(`<Relationship Id="rIdNotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide${slideRelsXmls.length + 1}.xml"/>`);
    }
    slideRelsXmls.push(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  ${relEntries.join('\n  ')}
</Relationships>`);
  }

  const hasNotes = slides.some((sl) => sl.notes && sl.notes.trim());
  const notesOverrides = hasNotes
    ? [
        '<Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>',
        '<Override PartName="/ppt/theme/theme2.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>',
        ...slides.map((sl, i) => (sl.notes && sl.notes.trim() ? `<Override PartName="/ppt/notesSlides/notesSlide${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>` : '')),
      ].filter(Boolean).join('\n  ')
    : '';
  zip.file('[Content_Types].xml', CONTENT_TYPES_XML.replace(
    '{{SLIDE_OVERRIDES}}',
    slides.map((_, i) => `<Override PartName="/ppt/slides/slide${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>`).join('\n  ')
  ).replace('{{NOTES_OVERRIDES}}', notesOverrides));
  zip.file('_rels/.rels', ROOT_RELS_XML);
  zip.file('ppt/presentation.xml', presentationXml(slides.length, hasNotes));
  zip.file('ppt/_rels/presentation.xml.rels', presentationRelsXml(slides.length, hasNotes));
  zip.file('ppt/theme/theme1.xml', THEME_XML);
  zip.file('ppt/slideMasters/slideMaster1.xml', SLIDE_MASTER_XML);
  zip.file('ppt/slideMasters/_rels/slideMaster1.xml.rels', SLIDE_MASTER_RELS_XML);
  zip.file('ppt/slideLayouts/slideLayout1.xml', SLIDE_LAYOUT_XML);
  zip.file('ppt/slideLayouts/_rels/slideLayout1.xml.rels', SLIDE_LAYOUT_RELS_XML);

  slides.forEach((slide, i) => {
    zip.file(`ppt/slides/slide${i + 1}.xml`, slideXml(slide, slideRelIds[i]));
    zip.file(`ppt/slides/_rels/slide${i + 1}.xml.rels`, slideRelsXmls[i]);
    if (slide.notes && slide.notes.trim()) {
      zip.file(`ppt/notesSlides/notesSlide${i + 1}.xml`, notesSlideXml(slide.notes));
      zip.file(`ppt/notesSlides/_rels/notesSlide${i + 1}.xml.rels`, notesSlideRels(i + 1));
    }
  });
  if (hasNotes) {
    zip.file('ppt/notesMasters/notesMaster1.xml', NOTES_MASTER_XML);
    zip.file('ppt/notesMasters/_rels/notesMaster1.xml.rels', NOTES_MASTER_RELS_XML);
    zip.file('ppt/theme/theme2.xml', THEME_XML);
  }

  return zip.generateAsync({ type: 'uint8array' });
}

function mimeForExt(ext: string): string {
  const lower = ext.toLowerCase();
  if (lower === 'jpg' || lower === 'jpeg') return 'image/jpeg';
  if (lower === 'gif') return 'image/gif';
  return 'image/png';
}

/** Reads a real `.pptx` file into this app's `Presentation` model — see
 * this module's doc for exactly what is and isn't preserved. */
export async function importPptx(data: ArrayBuffer): Promise<Presentation> {
  const zip = await JSZip.loadAsync(data);
  const parser = new DOMParser();

  const presentationXmlText = await zip.file('ppt/presentation.xml')?.async('string');
  if (!presentationXmlText) throw new Error('Not a valid .pptx: missing ppt/presentation.xml');
  const presentationRelsText = await zip.file('ppt/_rels/presentation.xml.rels')?.async('string');
  if (!presentationRelsText) throw new Error('Not a valid .pptx: missing ppt/_rels/presentation.xml.rels');

  const presDoc = parser.parseFromString(presentationXmlText, 'application/xml');
  const relsDoc = parser.parseFromString(presentationRelsText, 'application/xml');

  // Map relationship id -> target path, then walk <p:sldId> in document
  // order (the order slides actually appear in) to resolve each to its
  // slide XML part path.
  const relTargets = new Map<string, string>();
  Array.from(relsDoc.getElementsByTagNameNS('*', 'Relationship')).forEach((rel) => {
    const id = rel.getAttribute('Id');
    const target = rel.getAttribute('Target');
    if (id && target) relTargets.set(id, target);
  });

  const slideRIds = Array.from(presDoc.getElementsByTagNameNS('*', 'sldId')).map((el) =>
    el.getAttributeNS('http://schemas.openxmlformats.org/officeDocument/2006/relationships', 'id')
  );

  const sldSz = presDoc.getElementsByTagNameNS('*', 'sldSz')[0];
  const geo: SlideGeometry = {
    cx: Number(sldSz?.getAttribute('cx')) || SLIDE_WIDTH_EMU,
    cy: Number(sldSz?.getAttribute('cy')) || SLIDE_HEIGHT_EMU,
  };

  const slides: Slide[] = [];
  for (let i = 0; i < slideRIds.length; i++) {
    const rId = slideRIds[i];
    const target = rId ? relTargets.get(rId) : undefined;
    if (!target) continue;
    const slidePath = `ppt/${target.replace(/^\.\.\//, '')}`;
    const slideXmlText = await zip.file(slidePath)?.async('string');
    if (!slideXmlText) continue;

    const slideRelsPath = slidePath.replace(/\/slides\/([^/]+)$/, '/slides/_rels/$1.rels');
    const slideRelsText = await zip.file(slideRelsPath)?.async('string');
    const slideRelTargets = new Map<string, string>();
    let notesTarget: string | undefined;
    if (slideRelsText) {
      Array.from(parser.parseFromString(slideRelsText, 'application/xml').getElementsByTagNameNS('*', 'Relationship')).forEach((rel) => {
        const id = rel.getAttribute('Id');
        const t = rel.getAttribute('Target');
        if (id && t) slideRelTargets.set(id, t);
        if (t && (rel.getAttribute('Type') ?? '').endsWith('/notesSlide')) notesTarget = t;
      });
    }

    const slide = await parseSlideXml(slideXmlText, zip, slideRelTargets, geo);
    if (notesTarget) {
      const notesPath = `ppt/${notesTarget.replace(/^\.\.\//, '')}`;
      const notesText = await zip.file(notesPath)?.async('string');
      const notes = notesText ? parseNotesXml(notesText) : '';
      if (notes) slide.notes = notes;
    }
    slides.push(slide);
  }

  if (slides.length === 0) throw new Error('This .pptx has no readable slides');
  return { slides };
}

/** Text of the notes body placeholder (not the slide-number / header placeholders). */
export function parseNotesXml(xmlText: string): string {
  const doc = new DOMParser().parseFromString(xmlText, 'application/xml');
  const lines: string[] = [];
  for (const sp of Array.from(doc.getElementsByTagNameNS('*', 'sp'))) {
    const ph = sp.getElementsByTagNameNS('*', 'ph')[0];
    if (ph?.getAttribute('type') !== 'body') continue;
    for (const p of Array.from(sp.getElementsByTagNameNS('*', 'p'))) {
      lines.push(Array.from(p.getElementsByTagNameNS('*', 't')).map((t) => t.textContent ?? '').join(''));
    }
  }
  while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
  return lines.join('\n');
}

/** First <a:tbl> of a slide as rows of cell text. */
export function parseTable(doc: Document | Element): string[][] | undefined {
  const tbl = doc.getElementsByTagNameNS('*', 'tbl')[0];
  if (!tbl) return undefined;
  const rows = Array.from(tbl.getElementsByTagNameNS('*', 'tr')).map((tr) =>
    Array.from(tr.getElementsByTagNameNS('*', 'tc')).map((tc) =>
      Array.from(tc.getElementsByTagNameNS('*', 'p')).map((p) => Array.from(p.getElementsByTagNameNS('*', 't')).map((t) => t.textContent ?? '').join('')).join('\n')));
  return rows.length ? rows : undefined;
}

interface SlideGeometry { cx: number; cy: number }

const R_NS = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';

/** `a:xfrm` of a shape (EMU) → percent of the slide. */
function shapeRect(el: Element, geo: SlideGeometry): Rect | undefined {
  const xf = el.getElementsByTagNameNS('*', 'xfrm')[0];
  const off = xf?.getElementsByTagNameNS('*', 'off')[0];
  const ext = xf?.getElementsByTagNameNS('*', 'ext')[0];
  if (!off || !ext) return undefined;
  const [x, y, cx, cy] = [off.getAttribute('x'), off.getAttribute('y'), ext.getAttribute('cx'), ext.getAttribute('cy')].map(Number);
  if (![x, y, cx, cy].every(Number.isFinite) || cx <= 0 || cy <= 0) return undefined;
  return clampRect({ x: round1((x / geo.cx) * 100), y: round1((y / geo.cy) * 100), w: round1((cx / geo.cx) * 100), h: round1((cy / geo.cy) * 100) });
}

interface ParsedText { lines: string[]; style: SlideTextStyle }

/** Text and look of one shape's `<p:txBody>`. Properties that every run shares
 * become the shape's style; inline emphasis that only some runs carry stays as
 * `**bold**` / `*italic*` markers. */
function parseTextBody(sp: Element, fontScale: number): ParsedText {
  const tx = sp.getElementsByTagNameNS('*', 'txBody')[0];
  const style: SlideTextStyle = {};
  if (!tx) return { lines: [], style };

  const lst = tx.getElementsByTagNameNS('*', 'lstStyle')[0]?.getElementsByTagNameNS('*', 'defRPr')[0];
  const paragraphs = Array.from(tx.getElementsByTagNameNS('*', 'p'));

  type Run = FormattedRun & { u: boolean; sz?: number; color?: string; font?: string };
  const perParagraph: Run[][] = paragraphs.map((p) =>
    Array.from(p.getElementsByTagNameNS('*', 'r')).map((r) => {
      const rPr = r.getElementsByTagNameNS('*', 'rPr')[0];
      const sz = Number(rPr?.getAttribute('sz'));
      const clr = rPr?.getElementsByTagNameNS('*', 'srgbClr')[0]?.getAttribute('val');
      const u = rPr?.getAttribute('u');
      return {
        text: r.getElementsByTagNameNS('*', 't')[0]?.textContent ?? '',
        bold: rPr?.getAttribute('b') === '1',
        italic: rPr?.getAttribute('i') === '1',
        u: !!u && u !== 'none',
        sz: Number.isFinite(sz) && sz > 0 ? sz / 100 : undefined,
        color: clr ? `#${clr.toLowerCase()}` : undefined,
        font: rPr?.getElementsByTagNameNS('*', 'latin')[0]?.getAttribute('typeface') ?? undefined,
      };
    }),
  );
  const flat = perParagraph.flat().filter((r) => r.text.length > 0);

  const allBold = flat.length > 0 && flat.every((r) => r.bold);
  const allItalic = flat.length > 0 && flat.every((r) => r.italic);
  if (lst?.getAttribute('b') === '1' || allBold) style.bold = true;
  if (lst?.getAttribute('i') === '1' || allItalic) style.italic = true;
  if ((lst?.getAttribute('u') && lst.getAttribute('u') !== 'none') || (flat.length > 0 && flat.every((r) => r.u))) style.underline = true;
  const first = flat[0];
  if (first?.sz) style.fontSize = Math.round(first.sz * fontScale);
  if (first?.color && isHexColor(first.color)) style.color = first.color;
  if (first?.font && !first.font.startsWith('+')) style.fontFamily = first.font;

  const pPr = paragraphs[0]?.getElementsByTagNameNS('*', 'pPr')[0];
  const algn = pPr?.getAttribute('algn');
  if (algn && ALGN_BACK[algn]) style.align = ALGN_BACK[algn];
  if (pPr) {
    if (pPr.getElementsByTagNameNS('*', 'buChar').length || pPr.getElementsByTagNameNS('*', 'buAutoNum').length) style.bullets = true;
    else if (pPr.getElementsByTagNameNS('*', 'buNone').length) style.bullets = false;
  }

  const lines = perParagraph.map((runs) =>
    runsToMarkdownLite(
      (runs.length > 0 ? runs : [{ text: '', bold: false, italic: false }]).map((r) => ({
        text: r.text,
        bold: r.bold && !allBold,
        italic: r.italic && !allItalic,
      })),
    ),
  );
  return { lines, style };
}

/** Keeps only the style fields that differ from `base`, so a normal deck does not
 * carry a copy of the defaults on every slide. */
function styleDiff(found: SlideTextStyle, base: ReturnType<typeof resolveStyle>): SlideTextStyle | undefined {
  const out: SlideTextStyle = {};
  if (found.fontFamily && found.fontFamily !== base.fontFamily) out.fontFamily = found.fontFamily;
  if (found.fontSize && found.fontSize !== base.fontSize) out.fontSize = found.fontSize;
  if (found.color && found.color.toLowerCase() !== base.color.toLowerCase()) out.color = found.color;
  if (found.align && found.align !== base.align) out.align = found.align;
  if (found.bold !== undefined && found.bold !== base.bold) out.bold = found.bold;
  if (found.italic !== undefined && found.italic !== base.italic) out.italic = found.italic;
  if (found.underline !== undefined && found.underline !== base.underline) out.underline = found.underline;
  if (found.bullets !== undefined && found.bullets !== base.bullets) out.bullets = found.bullets;
  return Object.keys(out).length ? out : undefined;
}

const TRANSITION_FROM_XML: Record<string, TransitionType> = {
  fade: 'fade', dissolve: 'fade', push: 'push', pull: 'cover', wipe: 'wipe', cover: 'cover', zoom: 'zoom', split: 'split', randomBar: 'wipe', blinds: 'wipe',
};

/** `<p:transition>` → model (anything unknown becomes a fade rather than being dropped). */
export function parseTransition(doc: Document): SlideTransition | undefined {
  const el = doc.getElementsByTagNameNS('*', 'transition')[0];
  if (!el) return undefined;
  const adv = Number(el.getAttribute('advTm'));
  const spd = el.getAttribute('spd');
  const child = Array.from(el.children).find((c) => c.namespaceURI?.includes('presentationml') || c.localName === 'fade');
  const type: TransitionType = child ? TRANSITION_FROM_XML[child.localName] ?? 'fade' : 'none';
  const dir = child?.getAttribute('dir');
  return normalizeTransition({
    type,
    direction: dir === 'l' || dir === 'r' || dir === 'u' || dir === 'd' ? dir : undefined,
    durationMs: child ? (spd === 'fast' ? 350 : spd === 'slow' ? 1200 : 700) : undefined,
    advanceAfterMs: Number.isFinite(adv) && adv > 0 ? adv : undefined,
  });
}

async function parseSlideXml(xmlText: string, zip: JSZip, relTargets: Map<string, string>, geo: SlideGeometry = { cx: SLIDE_WIDTH_EMU, cy: SLIDE_HEIGHT_EMU }): Promise<Slide> {
  const doc = new DOMParser().parseFromString(xmlText, 'application/xml');
  const fontScale = SLIDE_WIDTH_EMU / geo.cx;
  const spTree = doc.getElementsByTagNameNS('*', 'spTree')[0];
  const children = spTree ? Array.from(spTree.children) : [];

  let title = '';
  let hasCtrTitle = false;
  let titleParsed: ParsedText | undefined;
  let titleRect: Rect | undefined;
  const bodies: { parsed: ParsedText; rect?: Rect }[] = [];
  const elements: SlideElement[] = [];
  let table: string[][] | undefined;
  let tableRect: Rect | undefined;

  for (const el of children) {
    const kind = el.localName;
    if (kind === 'sp') {
      const ph = el.getElementsByTagNameNS('*', 'ph')[0];
      const phType = ph?.getAttribute('type') ?? '';
      if (['dt', 'ftr', 'sldNum', 'hdr'].includes(phType)) continue; // footers are not content
      const parsed = parseTextBody(el, fontScale);
      const rect = shapeRect(el, geo);
      if (phType === 'title' || phType === 'ctrTitle') {
        if (phType === 'ctrTitle') hasCtrTitle = true;
        title += parsed.lines.join(' ');
        titleParsed = parsed;
        titleRect = rect;
      } else if (ph) {
        bodies.push({ parsed: { ...parsed, lines: parsed.lines.filter((l) => l.trim().length > 0) }, rect });
      } else if (parsed.lines.some((l) => l.trim().length > 0)) {
        while (parsed.lines.length && !parsed.lines[parsed.lines.length - 1].trim()) parsed.lines.pop();
        const spPr = el.getElementsByTagNameNS('*', 'spPr')[0];
        const fillEl = Array.from(spPr?.children ?? []).find((c) => c.localName === 'solidFill');
        const fill = fillEl?.getElementsByTagNameNS('*', 'srgbClr')[0]?.getAttribute('val');
        elements.push({
          kind: 'text',
          id: newId('text'),
          ...(rect ?? { x: 10, y: 40, w: 50, h: 16 }),
          text: parsed.lines.join('\n'),
          style: { bullets: false, ...parsed.style },
          ...(fill && isHexColor(`#${fill}`) ? { fill: `#${fill.toLowerCase()}` } : {}),
        });
      }
    } else if (kind === 'pic') {
      const blip = el.getElementsByTagNameNS('*', 'blip')[0];
      const embedId = blip?.getAttributeNS(R_NS, 'embed');
      const target = embedId ? relTargets.get(embedId) : undefined;
      const mediaPath = target ? `ppt/${target.replace(/^\.\.\//, '')}` : '';
      const file = mediaPath ? zip.file(mediaPath) : null;
      if (!file) continue;
      const base64 = await file.async('base64');
      const ext = mediaPath.split('.').pop() ?? 'png';
      const rect = shapeRect(el, geo) ?? pictureSlot('title-content');
      elements.push({ kind: 'image', id: newId('img'), ...rect, src: `data:${mimeForExt(ext)};base64,${base64}` });
    } else if (kind === 'graphicFrame') {
      const rows = parseTable(el);
      if (rows && !table) {
        table = rows;
        tableRect = shapeRect(el, geo);
      }
    }
  }

  // Layout: what the slide actually contains decides which layout it is.
  let layout: SlideLayoutId = 'title-content';
  if (hasCtrTitle) layout = 'title';
  else if (bodies.length >= 2) layout = 'two-content';
  else if (bodies.length === 0 && !titleParsed) layout = 'blank';
  else if (bodies.length === 0) layout = 'title-only';

  // Background: a solid <p:bg><p:bgPr><a:solidFill><a:srgbClr val=.../> — anything else
  // (gradients, images, theme-colour refs) falls back to the app's default background.
  const bgFill = doc.getElementsByTagNameNS('*', 'bg')[0]?.getElementsByTagNameNS('*', 'srgbClr')[0]?.getAttribute('val');
  const background = bgFill ? `#${bgFill.toLowerCase()}` : '#0f172a';

  const slide: Slide = {
    id: `slide-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    title: title || (layout === 'blank' ? '' : 'Untitled'),
    bullets: bodies[0]?.parsed.lines ?? [],
    background,
    images: [],
    layout,
    elements,
  };
  if (bodies[1]) slide.bullets2 = bodies[1].parsed.lines;
  if (table) slide.table = table;

  // Per-slide overrides: only where the file differs from the layout defaults.
  const rects: NonNullable<Slide['rects']> = {};
  const L = LAYOUTS[layout];
  if (titleRect && L.title && rectsDiffer(titleRect, L.title)) rects.title = titleRect;
  if (bodies[0]?.rect && L.body && rectsDiffer(bodies[0].rect, L.body)) rects.body = bodies[0].rect;
  if (bodies[1]?.rect && L.body2 && rectsDiffer(bodies[1].rect, L.body2)) rects.body2 = bodies[1].rect;
  if (tableRect) rects.table = tableRect;
  if (Object.keys(rects).length) slide.rects = rects;

  const ts = titleParsed && styleDiff(titleParsed.style, resolveStyle(slide, 'title'));
  if (ts) slide.titleStyle = ts;
  const bs = bodies[0] && styleDiff(bodies[0].parsed.style, resolveStyle(slide, 'body'));
  if (bs) slide.bodyStyle = bs;
  const b2 = bodies[1] && styleDiff(bodies[1].parsed.style, resolveStyle(slide, 'body2'));
  if (b2) slide.body2Style = b2;

  const transition = parseTransition(doc);
  if (transition) slide.transition = transition;
  return slide;
}

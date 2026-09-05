import JSZip from 'jszip';
import type { Presentation, Slide } from './types';

const EMU_PER_INCH = 914400;
const SLIDE_WIDTH_IN = 13.333; // 16:9, matches PowerPoint's modern default
const SLIDE_HEIGHT_IN = 7.5;
const SLIDE_WIDTH_EMU = Math.round(SLIDE_WIDTH_IN * EMU_PER_INCH);
const SLIDE_HEIGHT_EMU = Math.round(SLIDE_HEIGHT_IN * EMU_PER_INCH);

function xmlEscape(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&apos;');
}

function hexColor(cssColor: string): string {
  // Slide backgrounds in this app are always `#rrggbb` (see the
  // BACKGROUNDS palette in PresentationEditor.svelte) — strip the `#`
  // for OOXML's bare-hex `srgbClr val="RRGGBB"` attribute form.
  return cssColor.replace('#', '').toUpperCase().padEnd(6, '0').slice(0, 6);
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

function runXml(run: FormattedRun): string {
  const props = [];
  if (run.bold) props.push('b="1"');
  if (run.italic) props.push('i="1"');
  const rPr = props.length > 0 ? `<a:rPr lang="en-US" ${props.join(' ')} dirty="0"/>` : '<a:rPr lang="en-US" dirty="0"/>';
  return `<a:r>${rPr}<a:t>${xmlEscape(run.text)}</a:t></a:r>`;
}

function bulletParagraphXml(line: string): string {
  const runs = parseInlineFormatting(line);
  return `<a:p><a:pPr marL="285750" indent="-285750"><a:buChar char="•"/></a:pPr>${runs.map(runXml).join('')}</a:p>`;
}

function slideXml(slide: Slide, imageRelIds: string[]): string {
  const titleParagraph = `<a:p>${parseInlineFormatting(slide.title).map(runXml).join('')}</a:p>`;
  const bodyParagraphs = slide.bullets.filter((b) => b.trim().length > 0).map(bulletParagraphXml).join('') || '<a:p/>';

  // Images laid out in a simple row under the text — see this module's
  // doc for why there's no per-image position/size model to draw from.
  const imageCount = imageRelIds.length;
  const imgWidthEmu = Math.round((SLIDE_WIDTH_EMU * 0.8) / Math.max(imageCount, 1));
  const imgHeightEmu = Math.round(imgWidthEmu * 0.6);
  const imagesXml = imageRelIds
    .map((relId, i) => {
      const xOffset = Math.round(SLIDE_WIDTH_EMU * 0.1 + i * imgWidthEmu);
      const yOffset = Math.round(SLIDE_HEIGHT_EMU * 0.55);
      return `
        <p:pic>
          <p:nvPicPr>
            <p:cNvPr id="${100 + i}" name="Picture ${i + 1}"/>
            <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
            <p:nvPr/>
          </p:nvPicPr>
          <p:blipFill>
            <a:blip r:embed="${relId}"/>
            <a:stretch><a:fillRect/></a:stretch>
          </p:blipFill>
          <p:spPr>
            <a:xfrm>
              <a:off x="${xOffset}" y="${yOffset}"/>
              <a:ext cx="${imgWidthEmu}" cy="${imgHeightEmu}"/>
            </a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
        </p:pic>`;
    })
    .join('');

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
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          ${titleParagraph}
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          ${bodyParagraphs}
        </p:txBody>
      </p:sp>
      ${imagesXml}
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:overrideClrMapping bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>
</p:sld>`;
}

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

function presentationXml(slideCount: number): string {
  const sldIdLst = Array.from({ length: slideCount }, (_, i) => `<p:sldId id="${256 + i}" r:id="rIdSlide${i + 1}"/>`).join('');
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rIdMaster"/></p:sldMasterIdLst>
  <p:sldIdLst>${sldIdLst}</p:sldIdLst>
  <p:sldSz cx="${SLIDE_WIDTH_EMU}" cy="${SLIDE_HEIGHT_EMU}" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>`;
}

function presentationRelsXml(slideCount: number): string {
  const slideRels = Array.from(
    { length: slideCount },
    (_, i) => `<Relationship Id="rIdSlide${i + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide${i + 1}.xml"/>`
  ).join('');
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdMaster" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rIdTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
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
  const zip = new JSZip();
  const slides = presentation.slides;

  // Collect every image across every slide into a shared ppt/media/
  // pool (OOXML doesn't require images be de-duplicated, but there's
  // no reason to embed the same bytes twice if the same image somehow
  // appears on two slides).
  let mediaIndex = 1;
  const slideImageRelIds: string[][] = [];
  const slideRelsXmls: string[] = [];

  for (const slide of slides) {
    const relIds: string[] = [];
    const relEntries: string[] = [];
    for (const dataUrl of slide.images) {
      const { bytes, ext } = dataUrlToBytesAndExt(dataUrl);
      const mediaName = `image${mediaIndex}.${ext}`;
      zip.file(`ppt/media/${mediaName}`, bytes);
      const relId = `rIdImg${mediaIndex}`;
      relIds.push(relId);
      relEntries.push(`<Relationship Id="${relId}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/${mediaName}"/>`);
      mediaIndex++;
    }
    slideImageRelIds.push(relIds);
    slideRelsXmls.push(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  ${relEntries.join('\n  ')}
</Relationships>`);
  }

  zip.file('[Content_Types].xml', CONTENT_TYPES_XML.replace(
    '{{SLIDE_OVERRIDES}}',
    slides.map((_, i) => `<Override PartName="/ppt/slides/slide${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>`).join('\n  ')
  ));
  zip.file('_rels/.rels', ROOT_RELS_XML);
  zip.file('ppt/presentation.xml', presentationXml(slides.length));
  zip.file('ppt/_rels/presentation.xml.rels', presentationRelsXml(slides.length));
  zip.file('ppt/theme/theme1.xml', THEME_XML);
  zip.file('ppt/slideMasters/slideMaster1.xml', SLIDE_MASTER_XML);
  zip.file('ppt/slideMasters/_rels/slideMaster1.xml.rels', SLIDE_MASTER_RELS_XML);
  zip.file('ppt/slideLayouts/slideLayout1.xml', SLIDE_LAYOUT_XML);
  zip.file('ppt/slideLayouts/_rels/slideLayout1.xml.rels', SLIDE_LAYOUT_RELS_XML);

  slides.forEach((slide, i) => {
    zip.file(`ppt/slides/slide${i + 1}.xml`, slideXml(slide, slideImageRelIds[i]));
    zip.file(`ppt/slides/_rels/slide${i + 1}.xml.rels`, slideRelsXmls[i]);
  });

  return zip.generateAsync({ type: 'blob', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation' });
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
    if (slideRelsText) {
      Array.from(parser.parseFromString(slideRelsText, 'application/xml').getElementsByTagNameNS('*', 'Relationship')).forEach((rel) => {
        const id = rel.getAttribute('Id');
        const t = rel.getAttribute('Target');
        if (id && t) slideRelTargets.set(id, t);
      });
    }

    slides.push(await parseSlideXml(slideXmlText, zip, slideRelTargets));
  }

  if (slides.length === 0) throw new Error('This .pptx has no readable slides');
  return { slides };
}

async function parseSlideXml(xmlText: string, zip: JSZip, relTargets: Map<string, string>): Promise<Slide> {
  const doc = new DOMParser().parseFromString(xmlText, 'application/xml');
  const shapes = Array.from(doc.getElementsByTagNameNS('*', 'sp'));

  let title = '';
  const bodyLines: string[] = [];

  for (const sp of shapes) {
    const ph = sp.getElementsByTagNameNS('*', 'ph')[0];
    const isTitle = ph?.getAttribute('type') === 'title' || ph?.getAttribute('type') === 'ctrTitle';
    const paragraphs = Array.from(sp.getElementsByTagNameNS('*', 'txBody')[0]?.getElementsByTagNameNS('*', 'p') ?? []);

    for (const p of paragraphs) {
      const runs: FormattedRun[] = Array.from(p.getElementsByTagNameNS('*', 'r')).map((r) => {
        const t = r.getElementsByTagNameNS('*', 't')[0]?.textContent ?? '';
        const rPr = r.getElementsByTagNameNS('*', 'rPr')[0];
        const bold = rPr?.getAttribute('b') === '1';
        const italic = rPr?.getAttribute('i') === '1';
        return { text: t, bold, italic };
      });
      const line = runsToMarkdownLite(runs.length > 0 ? runs : [{ text: '', bold: false, italic: false }]);
      if (isTitle) {
        title += line;
      } else if (line.trim().length > 0) {
        bodyLines.push(line);
      }
    }
  }

  // Background: a solid <p:bg><p:bgPr><a:solidFill><a:srgbClr val=.../>
  // — anything else (gradients, images, theme-color refs) falls back
  // to this app's default slide background rather than attempting a
  // full OOXML fill-model translation.
  const bgFill = doc.getElementsByTagNameNS('*', 'bg')[0]?.getElementsByTagNameNS('*', 'srgbClr')[0]?.getAttribute('val');
  const background = bgFill ? `#${bgFill.toLowerCase()}` : '#0f172a';

  // Images: every <p:pic><p:blipFill><a:blip r:embed="..."/>, resolved
  // through this slide's own relationship file to an actual media part.
  const images: string[] = [];
  const blips = Array.from(doc.getElementsByTagNameNS('*', 'blip'));
  for (const blip of blips) {
    const embedId = blip.getAttributeNS('http://schemas.openxmlformats.org/officeDocument/2006/relationships', 'embed');
    const target = embedId ? relTargets.get(embedId) : undefined;
    if (!target) continue;
    const mediaPath = `ppt/${target.replace(/^\.\.\//, '')}`;
    const file = zip.file(mediaPath);
    if (!file) continue;
    const base64 = await file.async('base64');
    const ext = mediaPath.split('.').pop() ?? 'png';
    images.push(`data:${mimeForExt(ext)};base64,${base64}`);
  }

  return {
    id: `slide-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    title: title || 'Untitled',
    bullets: bodyLines,
    background,
    images,
  };
}

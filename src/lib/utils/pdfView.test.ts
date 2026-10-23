import { describe, it, expect } from 'vitest';
import { zoomIn, zoomOut, fitWidthZoom, fitPageZoom, dpiFor, pageSizePx, clampPage, parsePageInput, visiblePages, currentPage, MAX_ZOOM, MIN_ZOOM } from './pdfView';

describe('pdfView', () => {
  it('steps through zoom levels and stops at the ends', () => {
    expect(zoomIn(1)).toBe(1.25);
    expect(zoomIn(1.1)).toBe(1.25);
    expect(zoomIn(MAX_ZOOM)).toBe(MAX_ZOOM);
    expect(zoomOut(1)).toBe(0.75);
    expect(zoomOut(MIN_ZOOM)).toBe(MIN_ZOOM);
  });

  it('fits width: an A4 page (595pt) in 1000px', () => {
    // 595pt at zoom 1 = 793px; (1000-32)/793 = 1.22
    expect(fitWidthZoom(1000, 595)).toBeCloseTo(1.22, 2);
    expect(fitWidthZoom(10, 595)).toBe(1);
    expect(fitWidthZoom(1000, 0)).toBe(1);
    expect(fitWidthZoom(100000, 100)).toBe(MAX_ZOOM);
  });

  it('fit page is limited by the smaller dimension', () => {
    const z = fitPageZoom(2000, 600, 595, 842);
    expect(z).toBeLessThan(fitWidthZoom(2000, 595));
    expect(z).toBeCloseTo(((600 - 32) * 72) / (842 * 96), 1);
  });

  it('chooses a render dpi, honouring HiDPI and the backend clamp', () => {
    expect(dpiFor(1)).toBe(96);
    expect(dpiFor(2, 2)).toBe(300);
    expect(dpiFor(0.25)).toBe(30);
    expect(pageSizePx(595, 842, 1)).toEqual({ w: 793, h: 1123 });
  });

  it('parses page input', () => {
    expect(clampPage(0, 10)).toBe(1);
    expect(clampPage(99, 10)).toBe(10);
    expect(clampPage(NaN, 10)).toBe(1);
    expect(parsePageInput(' 7 ', 10)).toBe(7);
    expect(parsePageInput('abc', 10)).toBeNull();
    expect(parsePageInput('', 10)).toBeNull();
    expect(parsePageInput('50', 10)).toBe(10);
  });

  it('finds visible and current pages', () => {
    const heights = [1000, 1000, 1000, 1000];
    const tops = [0, 1012, 2024, 3036];
    expect(visiblePages(tops, heights, 0, 800, 100)).toEqual([1]);
    expect(visiblePages(tops, heights, 1500, 800, 100)).toEqual([2, 3]); // page 3 starts at 2024, inside 1500–2300
    expect(visiblePages(tops, heights, 900, 800, 600)).toEqual([1, 2, 3]);
    expect(currentPage(tops, heights, 0, 800)).toBe(1);
    expect(currentPage(tops, heights, 1100, 800)).toBe(2);
    expect(currentPage(tops, heights, 9999, 800)).toBe(4);
    expect(currentPage([], [], 0, 800)).toBe(1);
  });
});

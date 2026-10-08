import { describe, it, expect } from 'vitest';
import { rowsToCsv, sheetDocName, SPREADSHEET_EXTS } from './spreadsheetImport';

describe('spreadsheet import', () => {
  it('escapes csv cells and trims empty tail rows/columns', () => {
    const csv = rowsToCsv([['a', 'b,c', ''], ['"q"', 'line\nbreak', ''], ['', '', '']]);
    expect(csv).toBe('a,"b,c"\n"""q""","line\nbreak"');
  });
  it('keeps inner empty cells', () => {
    expect(rowsToCsv([['1', '', '3']])).toBe('1,,3');
  });
  it('names documents per sheet', () => {
    expect(sheetDocName('budżet.xlsx', 'Q1', 1)).toBe('budżet.csv');
    expect(sheetDocName('budżet.xlsx', 'Q1', 3)).toBe('budżet – Q1.csv');
    expect(SPREADSHEET_EXTS.has('ods') && !SPREADSHEET_EXTS.has('csv')).toBe(true);
  });
});

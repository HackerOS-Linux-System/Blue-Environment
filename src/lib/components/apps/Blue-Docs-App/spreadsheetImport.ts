export const SPREADSHEET_EXTS = new Set(['xlsx', 'xlsm', 'xlsb', 'xls', 'ods']);

export interface SheetData { name: string; rows: string[][] }

function csvCell(v: string): string {
  return /[",\n\r]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

export function rowsToCsv(rows: string[][]): string {
  // Obetnij puste wiersze z końca i puste kolumny z prawej (calamine zwraca prostokąt).
  let last = rows.length;
  while (last > 0 && rows[last - 1].every((c) => c === '')) last--;
  const trimmed = rows.slice(0, last);
  const width = trimmed.reduce((w, r) => {
    let n = r.length;
    while (n > 0 && r[n - 1] === '') n--;
    return Math.max(w, n);
  }, 0);
  return trimmed.map((r) => Array.from({ length: width }, (_, i) => csvCell(r[i] ?? '')).join(',')).join('\n');
}

/** Nazwa dokumentu dla arkusza: `plik.csv` albo `plik – Arkusz2.csv` przy wielu arkuszach. */
export function sheetDocName(fileName: string, sheet: string, total: number): string {
  const base = fileName.replace(/\.[^.]+$/, '');
  return total > 1 ? `${base} – ${sheet}.csv` : `${base}.csv`;
}

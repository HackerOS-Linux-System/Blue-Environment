import type { EditorView } from '@codemirror/view';

export type MarkerSeverity = 'error' | 'warning' | 'info';

/** A diagnostic in 1-based line/column coordinates (what Blue Code's panels already use). */
export interface EditorMarker {
  severity: MarkerSeverity;
  startLine: number; startCol: number;
  endLine: number; endCol: number;
  message: string; source?: string;
}

/**
 * The small, editor-agnostic surface the rest of Blue Code (and Notepad) use.
 * It replaces the raw Monaco `editor`/`monaco` objects that used to be passed around.
 */
export interface EditorHandle {
  readonly view: EditorView;
  focus(): void;
  getValue(): string;
  /** Scrolls the line to the centre of the viewport and puts the caret at its start. */
  revealLine(line: number): void;
  goToLine(): void;
  foldAll(): void;
  unfoldAll(): void;
  selectNextOccurrence(): void;
  /** Runs the language formatter; resolves to false when there is none for this language. */
  format(): Promise<boolean>;
  /** Replaces all markers of one source ('blue-lint', 'blue-lsp', …). */
  setMarkers(source: string, markers: EditorMarker[]): void;
}

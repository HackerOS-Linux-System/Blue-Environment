export interface LangOption { code: string; name: string; }

export const LANGUAGES: LangOption[] = [
  { code: 'auto', name: 'Detect language' },
  { code: 'en', name: 'English' }, { code: 'pl', name: 'Polish' }, { code: 'de', name: 'German' },
  { code: 'fr', name: 'French' }, { code: 'es', name: 'Spanish' }, { code: 'it', name: 'Italian' },
  { code: 'pt', name: 'Portuguese' }, { code: 'ru', name: 'Russian' }, { code: 'uk', name: 'Ukrainian' },
  { code: 'cs', name: 'Czech' }, { code: 'nl', name: 'Dutch' }, { code: 'sv', name: 'Swedish' },
  { code: 'tr', name: 'Turkish' }, { code: 'ja', name: 'Japanese' }, { code: 'ko', name: 'Korean' },
  { code: 'zh', name: 'Chinese' }, { code: 'ar', name: 'Arabic' }, { code: 'hi', name: 'Hindi' },
];

export interface HistoryEntry {
  id: string; from: string; to: string; sourceText: string; translatedText: string; time: number;
}

export interface TranslateResult { text: string; detectedLang?: string; }

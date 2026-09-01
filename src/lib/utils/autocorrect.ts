const TYPO_MAP: Record<string, string> = {
    teh: 'the', adn: 'and', taht: 'that', thier: 'their', recieve: 'receive',
    recieved: 'received', wich: 'which', wierd: 'weird', definately: 'definitely',
    seperate: 'separate', occured: 'occurred', untill: 'until', becuase: 'because',
    alot: 'a lot', accross: 'across', arguement: 'argument', beleive: 'believe',
    calender: 'calendar', cant: "can't", dont: "don't", wont: "won't",
    goverment: 'government', neccessary: 'necessary', occassion: 'occasion',
    priviledge: 'privilege', publically: 'publicly', reccommend: 'recommend',
    tommorow: 'tomorrow', tomatos: 'tomatoes', truely: 'truly',
    youre: "you're", thats: "that's", theyre: "they're", ive: "I've", im: "I'm",
    doesnt: "doesn't", isnt: "isn't", didnt: "didn't", couldnt: "couldn't",
    wouldnt: "wouldn't", shouldnt: "shouldn't", hasnt: "hasn't", havent: "haven't",
    absoultely: 'absolutely', apparant: 'apparent', appearence: 'appearance',
    begining: 'beginning', concious: 'conscious', embarass: 'embarrass',
    enviroment: 'environment', excercise: 'exercise', existance: 'existence',
    immediatly: 'immediately', independant: 'independent',
    knowlege: 'knowledge', maintainance: 'maintenance', persistant: 'persistent',
    posession: 'possession', prefered: 'preferred', probaly: 'probably',
    recomend: 'recommend', succesful: 'successful', suprise: 'surprise',
};

function matchCase(source: string, replacement: string): string {
    if (source === source.toUpperCase() && source !== source.toLowerCase()) return replacement.toUpperCase();
    if (source[0] === source[0]?.toUpperCase() && source[0] !== source[0]?.toLowerCase()) {
        return replacement.charAt(0).toUpperCase() + replacement.slice(1);
    }
    return replacement;
}

/** Straightens curly quotes/dashes typed via autocorrect-adjacent muscle
 * memory back to plain ASCII — the opposite direction of what a word
 * processor usually does, deliberately: this is a code-and-plain-text
 * editor (Notepad, Blue Code), where a curly `’` in source code or a
 * config file is a real bug, not a stylistic nicety. */
export function straightenPunctuation(text: string): string {
    return text
        .replace(/[\u2018\u2019]/g, "'")
        .replace(/[\u201C\u201D]/g, '"')
        .replace(/\u2013|\u2014/g, '-');
}

export interface AutocorrectOptions {
    /** Fix common typos from TYPO_MAP. Default true. */
    fixTypos?: boolean;
    /** Capitalize the first letter after `. ! ?` + space, and at the
     * very start of the text. Default false — off by default because
     * it's wrong for code/plain-data files (Notepad's default use
     * case includes editing JSON/YAML/logs, where forcing capitals
     * would actively corrupt content); callers editing prose should
     * opt in explicitly. */
    capitalizeSentences?: boolean;
}

/**
 * Given the full text and the cursor position right after a
 * word-boundary keystroke was typed, returns a correction if the word
 * immediately before the cursor matches something fixable — or `null`
 * if nothing applies (the overwhelmingly common case; callers should
 * treat `null` as "do nothing" and not re-render/move the cursor).
 */
export function autocorrectAt(
    text: string,
    cursor: number,
    opts: AutocorrectOptions = {},
): { newText: string; newCursor: number } | null {
    const { fixTypos = true, capitalizeSentences = false } = opts;
    if (cursor === 0 || cursor > text.length) return null;

    const boundaryChar = text[cursor - 1];
    if (!/[\s.,!?;:)\]"']/.test(boundaryChar)) return null; // only trigger right after a boundary char was just typed

    // Find the word ending right before the boundary char.
    const before = text.slice(0, cursor - 1);
    const wordMatch = before.match(/([A-Za-z']+)$/);
    let result: { newText: string; newCursor: number } | null = null;

    if (fixTypos && wordMatch) {
        const word = wordMatch[1];
        const lower = word.toLowerCase();
        const correction = TYPO_MAP[lower];
        if (correction && correction.toLowerCase() !== lower) {
            const wordStart = cursor - 1 - word.length;
            const corrected = matchCase(word, correction);
            const newText = text.slice(0, wordStart) + corrected + text.slice(cursor - 1);
            result = { newText, newCursor: wordStart + corrected.length + 1 };
        }
    }

    if (capitalizeSentences) {
        const base = result?.newText ?? text;
        const baseCursor = result?.newCursor ?? cursor;
        const beforeBoundary = base.slice(0, baseCursor - 1);
        const sentenceStart = Math.max(
            beforeBoundary.lastIndexOf('. '),
            beforeBoundary.lastIndexOf('! '),
            beforeBoundary.lastIndexOf('? '),
        );
        const wordStart = sentenceStart === -1 ? 0 : sentenceStart + 2;
        const firstChar = base[wordStart];
        if (firstChar && /[a-z]/.test(firstChar) && wordStart < baseCursor) {
            const newText = base.slice(0, wordStart) + firstChar.toUpperCase() + base.slice(wordStart + 1);
            result = { newText, newCursor: baseCursor };
        }
    }

    return result;
}

/**
 * Attaches autocorrect to a plain `<textarea>` element via an `input`
 * listener. Returns a cleanup function. This is the integration point
 * Notepad (and, later, other plain-text inputs) call — keeps the DOM
 * event wiring in one place rather than every caller reimplementing
 * "figure out cursor position, call autocorrectAt, splice the value
 * back in, restore selection".
 */
export function attachAutocorrect(
    el: HTMLTextAreaElement,
    getValue: () => string,
    setValue: (v: string) => void,
    opts: AutocorrectOptions = {},
): () => void {
    const handler = () => {
        const cursor = el.selectionStart;
        const result = autocorrectAt(getValue(), cursor, opts);
        if (!result) return;
        setValue(result.newText);
        // `setValue` is expected to synchronously update the bound
        // value in the caller's framework (Svelte's `bind:value`
        // reactivity is synchronous within an event handler), so the
        // selection can be restored immediately after.
        el.setSelectionRange(result.newCursor, result.newCursor);
    };
    el.addEventListener('input', handler);
    return () => el.removeEventListener('input', handler);
}

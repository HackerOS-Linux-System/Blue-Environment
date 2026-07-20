export type InstallStep = 'welcome' | 'language' | 'keyboard' | 'timezone' | 'disk' | 'account' | 'summary' | 'installing' | 'done' | 'error';

export interface DiskInfo { path: string; model: string; sizeBytes: number; sizeLabel: string; removable: boolean; }
export type DiskMode = 'erase' | 'manual';

/** One row of a manually-defined partition table. `sizeMiB: null` means "use remaining space". */
export interface PartitionPlanEntry {
  id: string;
  role: 'esp' | 'root' | 'home' | 'swap' | 'other';
  filesystem: 'fat32' | 'ext4' | 'btrfs' | 'xfs' | 'swap';
  mountpoint: string;
  sizeMiB: number | null;
}

export interface InstallConfig {
  locale: string;
  keyboardLayout: string;
  timezone: string;
  disk: DiskInfo | null;
  diskMode: DiskMode;
  /** Only used when diskMode === 'manual'; null/empty falls back to the default erase layout. */
  partitions: PartitionPlanEntry[];
  hostname: string;
  username: string;
  fullName: string;
  password: string;
  autoLogin: boolean;
}

export const LOCALES = [
  { code: 'en_US.UTF-8', label: 'English (United States)', defaultTz: 'America/New_York' },
  { code: 'en_GB.UTF-8', label: 'English (United Kingdom)', defaultTz: 'Europe/London' },
  { code: 'pl_PL.UTF-8', label: 'Polski (Polska)', defaultTz: 'Europe/Warsaw' },
  { code: 'de_DE.UTF-8', label: 'Deutsch (Deutschland)', defaultTz: 'Europe/Berlin' },
  { code: 'fr_FR.UTF-8', label: 'Français (France)', defaultTz: 'Europe/Paris' },
  { code: 'es_ES.UTF-8', label: 'Español (España)', defaultTz: 'Europe/Madrid' },
  { code: 'it_IT.UTF-8', label: 'Italiano (Italia)', defaultTz: 'Europe/Rome' },
  { code: 'pt_PT.UTF-8', label: 'Português (Portugal)', defaultTz: 'Europe/Lisbon' },
  { code: 'pt_BR.UTF-8', label: 'Português (Brasil)', defaultTz: 'America/Sao_Paulo' },
  { code: 'nl_NL.UTF-8', label: 'Nederlands (Nederland)', defaultTz: 'Europe/Amsterdam' },
  { code: 'sv_SE.UTF-8', label: 'Svenska (Sverige)', defaultTz: 'Europe/Stockholm' },
  { code: 'ru_RU.UTF-8', label: 'Русский (Россия)', defaultTz: 'Europe/Moscow' },
  { code: 'uk_UA.UTF-8', label: 'Українська (Україна)', defaultTz: 'Europe/Kyiv' },
  { code: 'cs_CZ.UTF-8', label: 'Čeština (Česko)', defaultTz: 'Europe/Prague' },
  { code: 'hu_HU.UTF-8', label: 'Magyar (Magyarország)', defaultTz: 'Europe/Budapest' },
  { code: 'tr_TR.UTF-8', label: 'Türkçe (Türkiye)', defaultTz: 'Europe/Istanbul' },
  { code: 'ja_JP.UTF-8', label: '日本語 (日本)', defaultTz: 'Asia/Tokyo' },
  { code: 'zh_CN.UTF-8', label: '中文 (中国)', defaultTz: 'Asia/Shanghai' },
];

/** IANA timezone identifiers grouped roughly by region, shown in the Timezone step. */
export const TIMEZONES = [
  { region: 'Europe', zones: [
    'Europe/London', 'Europe/Lisbon', 'Europe/Dublin', 'Europe/Paris', 'Europe/Madrid', 'Europe/Amsterdam',
    'Europe/Berlin', 'Europe/Rome', 'Europe/Warsaw', 'Europe/Prague', 'Europe/Budapest', 'Europe/Stockholm',
    'Europe/Vienna', 'Europe/Athens', 'Europe/Istanbul', 'Europe/Kyiv', 'Europe/Moscow',
  ]},
  { region: 'Americas', zones: [
    'America/New_York', 'America/Chicago', 'America/Denver', 'America/Los_Angeles', 'America/Anchorage',
    'America/Mexico_City', 'America/Bogota', 'America/Sao_Paulo', 'America/Argentina/Buenos_Aires', 'America/Toronto',
  ]},
  { region: 'Asia', zones: [
    'Asia/Istanbul', 'Asia/Dubai', 'Asia/Kolkata', 'Asia/Bangkok', 'Asia/Shanghai', 'Asia/Hong_Kong',
    'Asia/Tokyo', 'Asia/Seoul', 'Asia/Singapore', 'Asia/Jakarta',
  ]},
  { region: 'Africa', zones: ['Africa/Cairo', 'Africa/Lagos', 'Africa/Johannesburg', 'Africa/Nairobi'] },
  { region: 'Oceania', zones: ['Australia/Sydney', 'Australia/Perth', 'Pacific/Auckland'] },
  { region: 'Other', zones: ['UTC'] },
];

/**
 * Localized XDG user-dirs names, keyed by the language prefix of `locale`
 * (e.g. "pl" for "pl_PL.UTF-8"). Mirrors the translations shipped by the
 * real `xdg-user-dirs` package so a Polish install gets "Pobrane" instead
 * of "Downloads", a German install gets "Downloads"/"Dokumente", etc.
 * Falls back to English for any language not listed here.
 */
export interface UserDirNames {
  desktop: string; documents: string; downloads: string;
  music: string; pictures: string; videos: string; templates: string; public: string;
}

export const USER_DIR_NAMES: Record<string, UserDirNames> = {
  en: { desktop: 'Desktop', documents: 'Documents', downloads: 'Downloads', music: 'Music', pictures: 'Pictures', videos: 'Videos', templates: 'Templates', public: 'Public' },
  pl: { desktop: 'Pulpit', documents: 'Dokumenty', downloads: 'Pobrane', music: 'Muzyka', pictures: 'Obrazy', videos: 'Wideo', templates: 'Szablony', public: 'Publiczny' },
  de: { desktop: 'Schreibtisch', documents: 'Dokumente', downloads: 'Downloads', music: 'Musik', pictures: 'Bilder', videos: 'Videos', templates: 'Vorlagen', public: 'Öffentlich' },
  fr: { desktop: 'Bureau', documents: 'Documents', downloads: 'Téléchargements', music: 'Musique', pictures: 'Images', videos: 'Vidéos', templates: 'Modèles', public: 'Public' },
  es: { desktop: 'Escritorio', documents: 'Documentos', downloads: 'Descargas', music: 'Música', pictures: 'Imágenes', videos: 'Vídeos', templates: 'Plantillas', public: 'Público' },
  it: { desktop: 'Scrivania', documents: 'Documenti', downloads: 'Scaricati', music: 'Musica', pictures: 'Immagini', videos: 'Video', templates: 'Modelli', public: 'Pubblici' },
  pt: { desktop: 'Área de Trabalho', documents: 'Documentos', downloads: 'Transferências', music: 'Música', pictures: 'Imagens', videos: 'Vídeos', templates: 'Modelos', public: 'Público' },
  nl: { desktop: 'Bureaublad', documents: 'Documenten', downloads: 'Downloads', music: 'Muziek', pictures: "Afbeeldingen", videos: 'Video\u2019s', templates: 'Sjablonen', public: 'Openbaar' },
  sv: { desktop: 'Skrivbord', documents: 'Dokument', downloads: 'Hämtningar', music: 'Musik', pictures: 'Bilder', videos: 'Videoklipp', templates: 'Mallar', public: 'Publikt' },
  ru: { desktop: 'Рабочий стол', documents: 'Документы', downloads: 'Загрузки', music: 'Музыка', pictures: 'Изображения', videos: 'Видео', templates: 'Шаблоны', public: 'Общедоступные' },
  uk: { desktop: 'Стільниця', documents: 'Документи', downloads: 'Завантаження', music: 'Музика', pictures: 'Зображення', videos: 'Відео', templates: 'Шаблони', public: 'Спільнодоступні' },
  cs: { desktop: 'Plocha', documents: 'Dokumenty', downloads: 'Stažené', music: 'Hudba', pictures: 'Obrázky', videos: 'Videa', templates: 'Šablony', public: 'Veřejné' },
  hu: { desktop: 'Asztal', documents: 'Dokumentumok', downloads: 'Letöltések', music: 'Zene', pictures: 'Képek', videos: 'Videók', templates: 'Sablonok', public: 'Nyilvános' },
  tr: { desktop: 'Masaüstü', documents: 'Belgeler', downloads: 'İndirilenler', music: 'Müzik', pictures: 'Resimler', videos: 'Videolar', templates: 'Şablonlar', public: 'Genel' },
  ja: { desktop: 'デスクトップ', documents: 'ドキュメント', downloads: 'ダウンロード', music: '音楽', pictures: '画像', videos: 'ビデオ', templates: 'テンプレート', public: '公開' },
  zh: { desktop: '桌面', documents: '文档', downloads: '下载', music: '音乐', pictures: '图片', videos: '视频', templates: '模板', public: '公共' },
};

export function userDirNamesForLocale(locale: string): UserDirNames {
  const lang = locale.split(/[_.]/)[0]?.toLowerCase() ?? 'en';
  return USER_DIR_NAMES[lang] ?? USER_DIR_NAMES.en;
}

export const KEYBOARD_LAYOUTS = [
  { id: 'us', label: 'US English' }, { id: 'gb', label: 'UK English' }, { id: 'pl', label: 'Polish' },
  { id: 'de', label: 'German' }, { id: 'fr', label: 'French' }, { id: 'es', label: 'Spanish' },
  { id: 'it', label: 'Italian' }, { id: 'pt', label: 'Portuguese' }, { id: 'br', label: 'Portuguese (Brazil)' },
  { id: 'nl', label: 'Dutch' }, { id: 'se', label: 'Swedish' }, { id: 'ru', label: 'Russian' },
  { id: 'ua', label: 'Ukrainian' }, { id: 'cz', label: 'Czech' }, { id: 'hu', label: 'Hungarian' },
  { id: 'tr', label: 'Turkish' }, { id: 'jp', label: 'Japanese' }, { id: 'cn', label: 'Chinese' },
];

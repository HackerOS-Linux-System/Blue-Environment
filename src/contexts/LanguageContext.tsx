import React, { createContext, useContext, useState, useEffect } from 'react';
import { SystemBridge } from '../utils/systemBridge';

const translations = {
    en: {
        // App
        'app.name': 'Blue Environment',
        'app.version': 'Version',
        // TopBar
        'topbar.search': 'Search apps…',
        'topbar.start': 'Start',
        'topbar.workspace': 'Workspace',
        // StartMenu
        'startmenu.recent': 'Recent',
        'startmenu.all_apps': 'All apps',
        'startmenu.power': 'Power',
        'startmenu.shutdown': 'Shut Down',
        'startmenu.restart': 'Restart',
        'startmenu.logout': 'Log Out',
        'startmenu.suspend': 'Suspend',
        'startmenu.hibernate': 'Hibernate',
        'startmenu.new_folder': 'New Folder',
        'startmenu.new_text_file': 'New Text File',
        'startmenu.session': 'Session',
        // Settings
        'settings.title': 'Settings',
        'settings.display': 'Display',
        'settings.personalization': 'Personalization',
        'settings.wifi': 'Wi-Fi',
        'settings.bluetooth': 'Bluetooth',
        'settings.power': 'Power',
        'settings.panel': 'Panel',
        'settings.about': 'About',
        'settings.language': 'Language',
        'settings.accounts': 'Accounts',
        'settings.apps': 'Applications',
        'settings.brightness': 'Brightness',
        'settings.display_scale': 'Display Scale',
        'settings.resolution': 'Resolution',
        'settings.refresh_rate': 'Refresh Rate',
        'settings.wallpaper': 'Wallpaper',
        'settings.builtin_themes': 'Built-in Themes',
        'settings.custom_themes': 'Custom Themes',
        'settings.add_theme': 'Add Theme',
        'settings.new_css_theme': 'New CSS Theme',
        'settings.theme_name': 'Theme Name',
        'settings.css_code': 'CSS Code',
        'settings.accent': 'Accent',
        'settings.custom_theme': 'Custom Theme',
        'settings.apply': 'Apply',
        'settings.delete': 'Delete',
        'settings.cancel': 'Cancel',
        'settings.save': 'Save',
        'settings.connected': 'Connected',
        'settings.secured': 'Secured',
        'settings.open': 'Open',
        'settings.disconnect': 'Disconnect',
        'settings.connect': 'Connect',
        'settings.connecting': 'Connecting...',
        'settings.no_networks': 'No networks',
        'settings.no_devices': 'No devices',
        'settings.disconnected': 'Disconnected',
        'settings.battery': 'Battery',
        'settings.charging': 'Charging',
        'settings.on_battery': 'On battery',
        'settings.power_profiles': 'Power Profiles',
        'settings.power_saver': 'Power Saver',
        'settings.balanced': 'Balanced',
        'settings.performance': 'Performance',
        'settings.enable_panel': 'Enable Panel',
        'settings.panel_position': 'Position',
        'settings.panel_size': 'Size',
        'settings.panel_opacity': 'Opacity',
        'settings.top': 'Top',
        'settings.bottom': 'Bottom',
        'settings.left': 'Left',
        'settings.right': 'Right',
        'settings.select_language': 'Select language',
        'settings.language_restart_hint': 'Language change requires restart',
        'settings.night_light': 'Night Light',
        'settings.night_light_temperature': 'Color temperature',
        'settings.night_light_schedule': 'Schedule',
        'settings.manual': 'Manual',
        'settings.sunset': 'Sunset to sunrise',
        'settings.start_hour': 'Start hour',
        'settings.end_hour': 'End hour',
        // Apps
        'blueai.title': 'Blue AI',
        'bluecode.title': 'Blue Code',
        'bluesoftware.title': 'Blue Software',
        'mail.title': 'Mail',
        // Terminal
        'terminal.title': 'Terminal',
        'terminal.help': 'Type "help" for commands.',
        // Software
        'software.installed': 'Installed',
        'software.available': 'Available',
        'software.updates': 'Updates',
        'software.install': 'Install',
        'software.uninstall': 'Uninstall',
        'software.update': 'Update',
        // Mail
        'mail.inbox': 'Inbox',
        'mail.sent': 'Sent',
        'mail.drafts': 'Drafts',
        'mail.compose': 'Compose',
        'mail.subject': 'Subject',
        'mail.body': 'Message',
        // Code
        'code.file': 'File',
        'code.edit': 'Edit',
        'code.view': 'View',
        'code.new_file': 'New File',
        'code.open_file': 'Open File',
        'code.save': 'Save',
        // AI
        'ai.service': 'AI Service',
        'ai.select_service': 'Select AI Service',
        'ai.login': 'Login',
        'ai.logout': 'Logout',
        'ai.placeholder': 'Ask me anything...',
        'ai.thinking': 'Thinking...',
        'ai.welcome': 'Welcome! Choose an AI service and log in.',
    },
    pl: {
        'app.name': 'Blue Environment',
        'app.version': 'Wersja',
        'topbar.search': 'Szukaj aplikacji…',
        'topbar.start': 'Start',
        'topbar.workspace': 'Obszar roboczy',
        'startmenu.recent': 'Ostatnie',
        'startmenu.all_apps': 'Wszystkie aplikacje',
        'startmenu.power': 'Zasilanie',
        'startmenu.shutdown': 'Wyłącz',
        'startmenu.restart': 'Uruchom ponownie',
        'startmenu.logout': 'Wyloguj',
        'startmenu.suspend': 'Uśpij',
        'startmenu.hibernate': 'Hibernuj',
        'startmenu.new_folder': 'Nowy folder',
        'startmenu.new_text_file': 'Nowy plik tekstowy',
        'startmenu.session': 'Sesja',
        'settings.title': 'Ustawienia',
        'settings.display': 'Ekran',
        'settings.personalization': 'Personalizacja',
        'settings.wifi': 'Wi-Fi',
        'settings.bluetooth': 'Bluetooth',
        'settings.power': 'Zasilanie',
        'settings.panel': 'Panel',
        'settings.about': 'O systemie',
        'settings.language': 'Język',
        'settings.accounts': 'Konta',
        'settings.apps': 'Aplikacje',
        'settings.brightness': 'Jasność',
        'settings.display_scale': 'Skalowanie',
        'settings.resolution': 'Rozdzielczość',
        'settings.refresh_rate': 'Częstotliwość',
        'settings.wallpaper': 'Tapeta',
        'settings.builtin_themes': 'Motywy wbudowane',
        'settings.custom_themes': 'Motywy własne',
        'settings.add_theme': 'Dodaj motyw',
        'settings.new_css_theme': 'Nowy motyw CSS',
        'settings.theme_name': 'Nazwa motywu',
        'settings.css_code': 'Kod CSS',
        'settings.accent': 'Akcent',
        'settings.custom_theme': 'Motyw własny',
        'settings.apply': 'Zastosuj',
        'settings.delete': 'Usuń',
        'settings.cancel': 'Anuluj',
        'settings.save': 'Zapisz',
        'settings.connected': 'Połączono',
        'settings.secured': 'Zabezpieczona',
        'settings.open': 'Otwarta',
        'settings.disconnect': 'Rozłącz',
        'settings.connect': 'Połącz',
        'settings.connecting': 'Łączenie...',
        'settings.no_networks': 'Brak sieci',
        'settings.no_devices': 'Brak urządzeń',
        'settings.disconnected': 'Rozłączono',
        'settings.battery': 'Bateria',
        'settings.charging': 'Ładowanie',
        'settings.on_battery': 'Na baterii',
        'settings.power_profiles': 'Profile zasilania',
        'settings.power_saver': 'Oszczędzanie energii',
        'settings.balanced': 'Zrównoważony',
        'settings.performance': 'Wydajność',
        'settings.enable_panel': 'Włącz panel',
        'settings.panel_position': 'Pozycja',
        'settings.panel_size': 'Rozmiar',
        'settings.panel_opacity': 'Przezroczystość',
        'settings.top': 'Góra',
        'settings.bottom': 'Dół',
        'settings.left': 'Lewo',
        'settings.right': 'Prawo',
        'settings.select_language': 'Wybierz język',
        'settings.language_restart_hint': 'Zmiana języka wymaga restartu',
        'settings.night_light': 'Nocne światło',
        'settings.night_light_temperature': 'Temperatura barwowa',
        'settings.night_light_schedule': 'Harmonogram',
        'settings.manual': 'Ręcznie',
        'settings.sunset': 'Zachód – wschód słońca',
        'settings.start_hour': 'Godzina rozpoczęcia',
        'settings.end_hour': 'Godzina zakończenia',
        'blueai.title': 'Blue AI',
        'bluecode.title': 'Blue Code',
        'bluesoftware.title': 'Blue Software',
        'mail.title': 'Poczta',
        'terminal.title': 'Terminal',
        'terminal.help': 'Wpisz "help" aby zobaczyć dostępne komendy.',
        'software.installed': 'Zainstalowane',
        'software.available': 'Dostępne',
        'software.updates': 'Aktualizacje',
        'software.install': 'Zainstaluj',
        'software.uninstall': 'Odinstaluj',
        'software.update': 'Aktualizuj',
        'mail.inbox': 'Odebrane',
        'mail.sent': 'Wysłane',
        'mail.drafts': 'Szkice',
        'mail.compose': 'Nowa wiadomość',
        'mail.subject': 'Temat',
        'mail.body': 'Treść',
        'code.file': 'Plik',
        'code.edit': 'Edycja',
        'code.view': 'Widok',
        'code.new_file': 'Nowy plik',
        'code.open_file': 'Otwórz plik',
        'code.save': 'Zapisz',
        'ai.service': 'Usługa AI',
        'ai.select_service': 'Wybierz usługę AI',
        'ai.login': 'Zaloguj',
        'ai.logout': 'Wyloguj',
        'ai.placeholder': 'Zadaj pytanie...',
        'ai.thinking': 'Myślenie...',
        'ai.welcome': 'Witaj! Wybierz usługę AI i zaloguj się.',
    },
};

type Language = keyof typeof translations;

interface LanguageContextType {
    language: Language;
    setLanguage: (lang: Language) => void;
    t: (key: string) => string;
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

export const useLanguage = () => {
    const ctx = useContext(LanguageContext);
    if (!ctx) throw new Error('useLanguage must be used within LanguageProvider');
    return ctx;
};

export const LanguageProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [language, setLanguage] = useState<Language>('en');

    useEffect(() => {
        SystemBridge.loadConfig().then(config => {
            if (config.language === 'pl') setLanguage('pl');
            else setLanguage('en');
        });
    }, []);

    const t = (key: string): string => {
        return translations[language][key as keyof typeof translations[typeof language]] || key;
    };

    const handleSetLanguage = (lang: Language) => {
        setLanguage(lang);
        SystemBridge.loadConfig().then(config => {
            SystemBridge.saveConfig({ ...config, language: lang });
        });
    };

    return (
        <LanguageContext.Provider value={{ language, setLanguage: handleSetLanguage, t }}>
        {children}
        </LanguageContext.Provider>
    );
};

export interface HelpArticle {
  id: string;
  category: 'start' | 'apps' | 'privacy' | 'shortcuts' | 'troubleshooting';
  title: { en: string; pl: string };
  body: { en: string[]; pl: string[] };
}

export const HELP_CATEGORIES: Record<HelpArticle['category'], { en: string; pl: string }> = {
  start: { en: 'Getting started', pl: 'Pierwsze kroki' },
  apps: { en: 'Apps', pl: 'Aplikacje' },
  privacy: { en: 'Privacy & Security', pl: 'Prywatność i bezpieczeństwo' },
  shortcuts: { en: 'Keyboard shortcuts', pl: 'Skróty klawiszowe' },
  troubleshooting: { en: 'Troubleshooting', pl: 'Rozwiązywanie problemów' },
};

export const HELP_ARTICLES: HelpArticle[] = [
  {
    id: 'welcome-tour',
    category: 'start',
    title: { en: 'Replay the welcome tour', pl: 'Uruchom ponownie samouczek powitalny' },
    body: {
      en: [
        'Blue Welcome runs automatically the first time you start Blue Environment, walking through language, theme, a tour of the apps, and privacy-related settings like Screen Time and the on-screen keyboard.',
        'You can open it again any time from the Start Menu \u2014 search for "Blue Welcome" and launch it like any other app. Going through it again won\u2019t change anything you\u2019ve already configured unless you explicitly change a setting on one of its screens.',
      ],
      pl: [
        'Blue Welcome uruchamia się automatycznie przy pierwszym starcie Blue Environment, prowadząc przez wybór języka, motywu, krótkie zwiedzanie aplikacji oraz ustawienia związane z prywatnością, takie jak Czas korzystania z urządzenia czy klawiatura ekranowa.',
        'Możesz go uruchomić ponownie w dowolnym momencie z Menu Start \u2014 wyszukaj "Blue Welcome" i otwórz jak każdą inną aplikację. Ponowne przejście przez niego nie zmieni niczego, co już skonfigurowałeś, chyba że świadomie zmienisz ustawienie na jednym z jego ekranów.',
      ],
    },
  },
  {
    id: 'wallpaper-theme',
    category: 'start',
    title: { en: 'Change your wallpaper and theme', pl: 'Zmień tapetę i motyw' },
    body: {
      en: [
        'Open Settings \u2192 Personalization to change your wallpaper, and Settings \u2192 Themes to switch between light/dark and any custom shell themes you\u2019ve installed.',
        'Themes control window button style and position, corner rounding, and the color palette used throughout the shell \u2014 not just the desktop background.',
      ],
      pl: [
        'Otwórz Ustawienia \u2192 Personalizacja, aby zmienić tapetę, oraz Ustawienia \u2192 Motywy, aby przełączać się między jasnym/ciemnym trybem i dowolnymi zainstalowanymi niestandardowymi motywami powłoki.',
        'Motywy kontrolują styl i pozycję przycisków okna, zaokrąglenie rogów oraz paletę kolorów używaną w całej powłoce \u2014 nie tylko tło pulpitu.',
      ],
    },
  },
  {
    id: 'wifi-bluetooth',
    category: 'start',
    title: { en: 'Connect to Wi-Fi and Bluetooth', pl: 'Połącz się z Wi-Fi i Bluetooth' },
    body: {
      en: [
        'Click the Wi-Fi or Bluetooth icon in Control Center (top-right of the panel) to see nearby networks/devices, or go to Settings \u2192 Network / Bluetooth for the full list and saved-device management.',
      ],
      pl: [
        'Kliknij ikonę Wi-Fi lub Bluetooth w Centrum sterowania (prawy górny róg panelu), aby zobaczyć pobliskie sieci/urządzenia, lub przejdź do Ustawienia \u2192 Sieć / Bluetooth po pełną listę i zarządzanie zapisanymi urządzeniami.',
      ],
    },
  },
  {
    id: 'screen-time',
    category: 'privacy',
    title: { en: 'Screen Time: how it works', pl: 'Czas korzystania z urządzenia: jak to działa' },
    body: {
      en: [
        'Settings \u2192 Screen Time shows how much time you\u2019ve spent in each app, broken down by today, the last 7 days, the last 30 days, or your entire history.',
        'It works by checking, once every minute, which app\u2019s window is currently focused and not minimized \u2014 that app gets credited with one more minute. This measures focus time, not real attention: an app left open and focused while you\u2019re away from the keyboard still counts as used.',
        'The history is stored locally and never leaves your device. You can clear it entirely from the same Settings page.',
      ],
      pl: [
        'Ustawienia \u2192 Czas korzystania z urządzenia pokazują, ile czasu spędziłeś w każdej aplikacji, w podziale na dzisiaj, ostatnie 7 dni, ostatnie 30 dni lub całą historię.',
        'Działa to poprzez sprawdzanie raz na minutę, które okno aplikacji jest aktualnie aktywne i niezminimalizowane \u2014 ta aplikacja dostaje zaliczoną kolejną minutę. Mierzy to czas skupienia uwagi na ekranie, a nie prawdziwą uwagę: aplikacja pozostawiona otwarta i aktywna pod nieobecność użytkownika nadal liczy się jako używana.',
        'Historia jest przechowywana lokalnie i nigdy nie opuszcza Twojego urządzenia. Możesz ją całkowicie wyczyścić na tej samej stronie Ustawień.',
      ],
    },
  },
  {
    id: 'cloned-apps',
    category: 'privacy',
    title: { en: 'Cloned Apps: using two accounts', pl: 'Sklonowane aplikacje: korzystanie z dwóch kont' },
    body: {
      en: [
        'Settings \u2192 Cloned Apps lets you create a second, separately-named and separately-iconed launch entry for an existing app \u2014 similar to Android\u2019s "clone app" feature.',
        'Important: a clone gets its own name and window right away, but whether it also gets its own separate data (a second logged-in account, separate browsing history, etc.) depends on that specific app supporting it. Most apps don\u2019t yet, so cloning one today mainly gives you a conveniently separate window/taskbar entry into the same shared data \u2014 check an app\u2019s own documentation (or ask in Blue Help\u2019s feedback channel) if you need guaranteed data separation for a specific app.',
      ],
      pl: [
        'Ustawienia \u2192 Sklonowane aplikacje pozwalają utworzyć drugi, osobno nazwany i osobno oznaczony wpis uruchamiania dla istniejącej aplikacji \u2014 podobnie do funkcji "klonuj aplikację" znanej z Androida.',
        'Ważne: klon od razu dostaje własną nazwę i okno, ale to, czy dostanie też własne, osobne dane (drugie zalogowane konto, osobną historię przeglądania itd.), zależy od tego, czy dana aplikacja to obsługuje. Większość jeszcze nie, więc sklonowanie dziś daje głównie wygodnie osobne okno/wpis na pasku zadań do tych samych, wspólnych danych \u2014 sprawdź dokumentację konkretnej aplikacji, jeśli potrzebujesz gwarantowanej separacji danych.',
      ],
    },
  },
  {
    id: 'onscreen-keyboard',
    category: 'privacy',
    title: { en: 'On-screen keyboard', pl: 'Klawiatura ekranowa' },
    body: {
      en: [
        'Settings \u2192 On-screen Keyboard (off by default) shows a touch keyboard automatically whenever a text field inside Blue Environment gains focus.',
        'Scope note: it currently types into Blue Environment\u2019s own apps only (Notepad, Blue Docs, search fields, address bars, and so on). The compositor now has the underlying protocol needed to type into other native windows too (gated to trusted system processes only, for security), but Blue Environment itself doesn\u2019t yet speak that protocol \u2014 that\u2019s the remaining piece.',
      ],
      pl: [
        'Ustawienia \u2192 Klawiatura ekranowa (domyślnie wyłączona) automatycznie pokazuje klawiaturę dotykową, gdy pole tekstowe wewnątrz Blue Environment zostanie zaznaczone.',
        'Zastrzeżenie co do zakresu: obecnie wpisuje tekst tylko we własnych aplikacjach Blue Environment (Notatnik, Blue Docs, pola wyszukiwania, paski adresu itd.). Kompozytor ma już protokół potrzebny do wpisywania tekstu również do innych, natywnych okien (ograniczony wyłącznie do zaufanych procesów systemowych, ze względów bezpieczeństwa), ale samo Blue Environment jeszcze nim nie mówi \u2014 to pozostały brakujący element.',
      ],
    },
  },
  {
    id: 'parental-controls',
    category: 'privacy',
    title: { en: 'Parental Controls', pl: 'Kontrola rodzicielska' },
    body: {
      en: [
        'Settings \u2192 Parental Controls lets you PIN-protect the feature, block specific apps entirely, set a daily time limit per app, and restrict allowed hours of use.',
        'This is separate from Screen Time: Parental Controls resets its own daily usage counters at midnight (since it only needs to know "how much has this app been used today" to enforce a limit), while Screen Time keeps permanent history for your own reference.',
      ],
      pl: [
        'Ustawienia \u2192 Kontrola rodzicielska pozwalają zabezpieczyć funkcję kodem PIN, całkowicie zablokować konkretne aplikacje, ustawić dzienny limit czasu na aplikację oraz ograniczyć dozwolone godziny korzystania.',
        'To coś innego niż Czas korzystania z urządzenia: Kontrola rodzicielska resetuje własne dzienne liczniki użycia o północy (bo do wymuszenia limitu wystarczy jej wiedzieć "ile ta aplikacja była dziś używana"), podczas gdy Czas korzystania z urządzenia zachowuje trwałą historię do Twojego własnego wglądu.',
      ],
    },
  },
  {
    id: 'lock-screen',
    category: 'privacy',
    title: { en: 'Setting up a lock screen', pl: 'Konfiguracja ekranu blokady' },
    body: {
      en: [
        'Settings \u2192 Security lets you set up a pattern lock or, on supported hardware, a fingerprint. Once configured, press Super+L (the Windows/Command key + L) any time to lock immediately.',
        'Blue Security and Privacy gives you a quick glance at whether a lock method is configured, alongside Parental Controls, Screen Time, Cloned Apps, and the on-screen keyboard, all in one dashboard.',
      ],
      pl: [
        'Ustawienia \u2192 Zabezpieczenia pozwalają skonfigurować blokadę wzorem lub, na wspieranym sprzęcie, odciskiem palca. Po skonfigurowaniu naciśnij Super+L (klawisz Windows/Command + L), aby natychmiast zablokować ekran.',
        'Blue Security and Privacy daje szybki podgląd, czy metoda blokady jest skonfigurowana, obok Kontroli rodzicielskiej, Czasu korzystania z urządzenia, Sklonowanych aplikacji i klawiatury ekranowej \u2014 wszystko w jednym miejscu.',
      ],
    },
  },
  {
    id: 'blue-web',
    category: 'apps',
    title: { en: 'Blue Web browser basics', pl: 'Podstawy przeglądarki Blue Web' },
    body: {
      en: [
        'Blue Web is a real embedded browser with tabs, downloads (visible from its Downloads panel), zoom, and find-in-page (Ctrl+F while a tab is focused).',
      ],
      pl: [
        'Blue Web to prawdziwa wbudowana przeglądarka z kartami, pobieraniem (widocznym w panelu Pobrane), powiększaniem i wyszukiwaniem na stronie (Ctrl+F przy aktywnej karcie).',
      ],
    },
  },
  {
    id: 'blue-play',
    category: 'apps',
    title: { en: 'Blue Play: tracking playtime', pl: 'Blue Play: śledzenie czasu gry' },
    body: {
      en: [
        'Blue Play tracks how long each of your games has run. Playtime is tracked as long as Blue Environment itself keeps running \u2014 it won\u2019t survive a full logout/restart of the desktop session while a game is still running outside of it.',
      ],
      pl: [
        'Blue Play śledzi, jak długo działała każda z Twoich gier. Czas gry jest śledzony, dopóki samo Blue Environment działa \u2014 nie przetrwa pełnego wylogowania/restartu sesji pulpitu, jeśli gra wciąż działa poza nim.',
      ],
    },
  },
  {
    id: 'blue-virt',
    category: 'apps',
    title: { en: 'Blue Virt: running virtual machines', pl: 'Blue Virt: uruchamianie maszyn wirtualnych' },
    body: {
      en: [
        'Blue Virt runs virtual machines using QEMU, with hardware acceleration (KVM) used automatically when available on your system.',
      ],
      pl: [
        'Blue Virt uruchamia maszyny wirtualne przy użyciu QEMU, automatycznie korzystając z akceleracji sprzętowej (KVM), gdy jest dostępna w Twoim systemie.',
      ],
    },
  },
  {
    id: 'blue-connect',
    category: 'apps',
    title: { en: 'Blue Connect: nearby devices', pl: 'Blue Connect: pobliskie urządzenia' },
    body: {
      en: [
        'Blue Connect discovers other devices on your local network. Pairing currently works between two Blue Connect instances \u2014 it can see, but not yet pair with, phones running a different app implementing the same discovery protocol.',
      ],
      pl: [
        'Blue Connect odkrywa inne urządzenia w Twojej sieci lokalnej. Parowanie obecnie działa między dwoma instancjami Blue Connect \u2014 może widzieć, ale jeszcze nie sparować się z telefonami korzystającymi z innej aplikacji implementującej ten sam protokół odkrywania.',
      ],
    },
  },
  {
    id: 'shortcuts-list',
    category: 'shortcuts',
    title: { en: 'Full keyboard shortcut list', pl: 'Pełna lista skrótów klawiszowych' },
    body: {
      en: [
        'Super (tap): open the full-screen Start Menu \u00b7 Super+Tab: same',
        'Alt+Tab / Alt+Shift+Tab: switch windows forward / backward',
        'Super+\u2192 / Super+\u2190: switch to the next / previous workspace',
        'Super+\u2191: maximize the active window \u00b7 Super+\u2193: minimize it',
        'Alt+F4: close the active window \u00b7 Alt+F1: toggle the Start Menu',
        'Super+L: lock the screen \u00b7 Super+D: show the desktop',
        'Ctrl+Alt+T: open a new Terminal window',
        'Ctrl+Shift+V: toggle the clipboard history panel',
        'Ctrl+Alt+C: toggle Control Center',
        'Print Screen: take a screenshot',
      ],
      pl: [
        'Super (dotknięcie): otwiera pełnoekranowe Menu Start \u00b7 Super+Tab: to samo',
        'Alt+Tab / Alt+Shift+Tab: przełączanie okien w przód / w tył',
        'Super+\u2192 / Super+\u2190: przełącz na następną / poprzednią przestrzeń roboczą',
        'Super+\u2191: maksymalizuje aktywne okno \u00b7 Super+\u2193: minimalizuje je',
        'Alt+F4: zamyka aktywne okno \u00b7 Alt+F1: przełącza Menu Start',
        'Super+L: blokuje ekran \u00b7 Super+D: pokazuje pulpit',
        'Ctrl+Alt+T: otwiera nowe okno Terminala',
        'Ctrl+Shift+V: przełącza panel historii schowka',
        'Ctrl+Alt+C: przełącza Centrum sterowania',
        'Print Screen: robi zrzut ekranu',
      ],
    },
  },
  {
    id: 'app-wont-open',
    category: 'troubleshooting',
    title: { en: 'An app won\u2019t open or looks broken', pl: 'Aplikacja się nie otwiera lub wygląda na uszkodzoną' },
    body: {
      en: [
        'Most apps in Blue Environment load lazily on first open \u2014 a spinning loader for a second or two is normal. If it shows a "Failed to load" message instead, try closing and reopening it; if that persists, it usually means a page reload of the whole shell is needed.',
        'For an app that opens but behaves oddly, check Settings \u2192 Applications to confirm it hasn\u2019t been disabled, and Settings \u2192 Parental Controls to confirm it isn\u2019t blocked or over its daily limit.',
      ],
      pl: [
        'Większość aplikacji w Blue Environment ładuje się leniwie przy pierwszym otwarciu \u2014 kręcący się wskaźnik ładowania przez sekundę lub dwie jest normalny. Jeśli zamiast tego pojawi się komunikat "Failed to load", spróbuj ją zamknąć i otworzyć ponownie; jeśli problem się powtarza, zwykle oznacza to konieczność przeładowania całej powłoki.',
        'Jeśli aplikacja się otwiera, ale zachowuje się dziwnie, sprawdź Ustawienia \u2192 Aplikacje, czy nie została wyłączona, oraz Ustawienia \u2192 Kontrola rodzicielska, czy nie jest zablokowana lub nie przekroczyła dziennego limitu.',
      ],
    },
  },
];

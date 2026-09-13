(def stage (os/getenv "ZPM_PACKAGE_STAGE_DIR"))

(defn fail [msg]
  (eprint "recipe.janet: " msg)
  (os/exit 1))

(defn run [cmd]
  # `os/shell` zwraca kod wyjścia polecenia (jak C-owe system()) --
  # zero == sukces.
  (def code (os/shell cmd))
  (unless (zero? code)
    (fail (string "'" cmd "' zakończone kodem " code))))

(defn try-run [cmd]
  # Jak `run`, ale nie przerywa recipe przy niepowodzeniu -- zwraca
  # true/false. Do kroków, które są "najlepszym wysiłkiem".
  (zero? (os/shell cmd)))

(defn have? [tool]
  (zero? (os/shell (string "command -v " tool " >/dev/null 2>&1"))))

(defn root? []
  (zero? (os/shell "test \"$(id -u)\" = 0")))

(defn sudo- []
  (if (root?) "" (if (have? "sudo") "sudo " "")))

(defn ensure-dir [path]
  # `os/mkdir` w Janet nie jest rekurencyjne i zgłasza błąd, jeśli katalog
  # już istnieje -- oba przypadki nieszkodliwe, więc łykamy błąd.
  (try (os/mkdir path) ([_] nil)))

(defn ensure-dir-p [path]
  # Rekurencyjny wariant `ensure-dir` -- buduje drzewo katalog po
  # katalogu (potrzebne dla usr/share/Blue-Environment i
  # usr/share/applications).
  (var acc "")
  (each part (string/split "/" path)
    (when (> (length part) 0)
      (set acc (string acc "/" part))
      (ensure-dir acc))))

# ---------------------------------------------------------------------
# Auto-instalacja brakujących narzędzi -- wykrywa menedżer pakietów
# (apt/dnf/pacman/zypper/apk/brew), nie tylko apt/Debian.
# ---------------------------------------------------------------------

(defn detect-pm []
  (cond
    (have? "apt-get") :apt
    (have? "dnf") :dnf
    (have? "pacman") :pacman
    (have? "zypper") :zypper
    (have? "apk") :apk
    (have? "brew") :brew
    :none))

(defn pm-install [pkgs-by-pm]
  (def pm (detect-pm))
  (def pkgs (get pkgs-by-pm pm))
  (if (not pkgs)
    false
    (let [sudo (sudo-)]
      (case pm
        :apt (do
               # `apt-get update` bywa niezerowy, jeśli JEDNO skonfigurowane
               # źródło (np. martwe stare repo firmy trzeciej) nie odpowiada
               # -- reszta list pakietów i tak się odświeża. Nie łączymy tego
               # przez `&&` z `install`, żeby zepsute jedno źródło nie blokowało
               # instalacji z pozostałych, działających repozytoriów.
               (try-run (string sudo "apt-get update"))
               (try-run (string sudo "env DEBIAN_FRONTEND=noninteractive apt-get install -y " pkgs)))
        :dnf (try-run (string sudo "dnf install -y " pkgs))
        :pacman (try-run (string sudo "pacman -Sy --noconfirm " pkgs))
        :zypper (try-run (string sudo "zypper --non-interactive install " pkgs))
        :apk (try-run (string sudo "apk add --no-cache " pkgs))
        :brew (try-run (string "brew install " pkgs))
        false))))

# ---------------------------------------------------------------------
# `cargo` i `git` są potrzebne w DWÓCH niezależnych krokach tego recipe
# (budowa powłoki blue-environment poniżej ORAZ budowa osobnego projektu
# HackerOS-Comp dalej) -- wydzielone jako funkcje, żeby nie duplikować
# tej samej logiki instalowania dwa razy.
# ---------------------------------------------------------------------

(defn ensure-cargo! []
  (unless (have? "cargo")
    (eprint "recipe.janet: brak 'cargo' -- próbuję zainstalować (" (detect-pm) ")...")
    (pm-install {:apt "cargo" :dnf "cargo" :pacman "rust" :zypper "cargo" :apk "cargo" :brew "rust"})
    (unless (have? "cargo")
      (eprint "recipe.janet: 'cargo' nadal niedostępne -- próbuję rustup (oficjalny instalator)...")
      (try-run "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable")
      (def cargo-bin-dir (string (os/getenv "HOME") "/.cargo/bin"))
      (when (os/stat (string cargo-bin-dir "/cargo") :mode)
        (os/setenv "PATH" (string cargo-bin-dir ":" (os/getenv "PATH"))))))
  (unless (have? "cargo")
    (fail "nie udało się zapewnić 'cargo' -- zainstaluj Rust ręcznie (rustup) i uruchom ponownie")))

(defn ensure-git! []
  (unless (have? "git")
    (eprint "recipe.janet: brak 'git' -- próbuję zainstalować (" (detect-pm) ")...")
    (pm-install {:apt "git" :dnf "git" :pacman "git" :zypper "git" :apk "git" :brew "git"}))
  (unless (have? "git")
    (fail "nie udało się zapewnić 'git' -- zainstaluj go ręcznie (menedżer pakietów dystrybucji) i uruchom ponownie")))

# packaging/zenit/recipe.janet leży dwa poziomy pod korzeniem repo
# (packaging/zenit -> packaging -> <root>) -- zpk zawsze ustawia cwd
# recipe na katalog z zpk.build, więc korzeń repo liczymy względem
# (os/cwd), niezależnie skąd faktycznie wywołano `zpk build`.
(def repo-root (string (os/cwd) "/../.."))

# Binarka powłoki (Tauri) trafia do <root>/target/release, bo src-tauri
# jest członkiem workspace'u zdefiniowanego w <root>/Cargo.toml -- patrz
# SHELL_BIN_PATH w build.hl, ten sam layout.
(var bin-path (string repo-root "/target/release/blue-environment"))

(def prebuilt (os/getenv "ZPK_PACKAGING_PREBUILT_BIN"))

(if (and prebuilt (> (length prebuilt) 0))
  # CI/operator już zbudował powłokę wcześniej w tym samym biegu (np.
  # `cargo build --release --manifest-path src-tauri/Cargo.toml` jako
  # osobny krok) -- nie buduj drugi raz. Pomijamy też poniższą logikę
  # instalowania npm/cargo/gtk.
  (set bin-path prebuilt)
  (do
    # -----------------------------------------------------------
    # Node.js/npm -- jeśli brak, próbujemy menedżera pakietów; jego
    # wersja bywa stara (Tauri/Vite potrzebują Node >= 18), więc jako
    # ostatnią deskę ratunku używamy NodeSource (działa tak samo na
    # apt/dnf/zypper -- nie tylko Debianie).
    # -----------------------------------------------------------
    (unless (have? "npm")
      (eprint "recipe.janet: brak 'npm' -- próbuję zainstalować (" (detect-pm) ")...")
      (pm-install {:apt "nodejs npm" :dnf "nodejs npm" :pacman "nodejs npm" :zypper "nodejs npm" :apk "nodejs npm" :brew "node"})
      (unless (have? "npm")
        (eprint "recipe.janet: 'npm' nadal niedostępne -- próbuję NodeSource (setup_22.x)...")
        (when (try-run "curl -fsSL https://deb.nodesource.com/setup_22.x -o /tmp/zpk-nodesource.sh")
          (try-run (string (sudo-) "bash /tmp/zpk-nodesource.sh"))
          (pm-install {:apt "nodejs" :dnf "nodejs" :zypper "nodejs"}))))
    (unless (have? "npm")
      (fail "nie udało się zapewnić 'npm' -- zainstaluj Node.js >= 18 ręcznie i uruchom ponownie"))

    # -----------------------------------------------------------
    # cargo -- pakiet dystrybucyjny, w ostateczności rustup (działa
    # identycznie na każdej dystrybucji, nie wymaga roota).
    # -----------------------------------------------------------
    (ensure-cargo!)

    # -----------------------------------------------------------
    # Nagłówki GTK/WebKit/libsoup -- Tauri na Linuksie linkuje się z
    # nimi przez pkg-config; bez nich `cargo build` w src-tauri pada
    # z "The system library `webkit2gtk-4.1` required ... not found"
    # zamiast czytelnego komunikatu. Nazwy pakietów różnią się mocno
    # między dystrybucjami, więc próbujemy najbardziej typowe zestawy.
    # -----------------------------------------------------------
    (unless (try-run "pkg-config --exists webkit2gtk-4.1 gtk+-3.0 libsoup-3.0")
      (eprint "recipe.janet: brak nagłówków GTK/WebKit/libsoup wymaganych przez Tauri -- próbuję zainstalować (" (detect-pm) ")...")
      (pm-install {:apt "libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf"
                   :dnf "gtk3-devel libsoup3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf"
                   :pacman "gtk3 libsoup3 webkit2gtk-4.1 libappindicator-gtk3 librsvg patchelf"
                   :zypper "gtk3-devel libsoup3-devel webkit2gtk3-soup2-devel libappindicator3-devel librsvg-devel patchelf"
                   :apk "gtk+3.0-dev libsoup3-dev webkit2gtk-4.1-dev libappindicator-dev librsvg-dev patchelf"
                   :brew nil})
      (unless (try-run "pkg-config --exists webkit2gtk-4.1 gtk+-3.0 libsoup-3.0")
        (eprint "recipe.janet: uwaga -- nadal nie widzę webkit2gtk-4.1/gtk+-3.0/libsoup-3.0 przez pkg-config; jeśli build `cargo` zaraz padnie, doinstaluj je ręcznie dla swojej dystrybucji")))

    (run (string "cd " repo-root " && npm install"))

    # Budujemy binarkę BEZPOŚREDNIO cargo, z pominięciem CLI Tauri
    # (`tauri build` / `npm run tauri build`) -- to CELOWA zmiana, nie
    # tylko kwestia flagi `--bundles`. `tauri build` po skompilowaniu
    # binarki (widocznej już wtedy w target/release/) przechodzi do
    # etapu bundlowania natywnych paczek (.deb/.rpm/.AppImage), który
    # potrafi WISIEĆ W NIESKOŃCZONOŚĆ (np. próba pobrania
    # linuxdeploy/AppImage tooling, brak odpowiedzi sieci w
    # danym środowisku) -- obserwowane w praniu. Pomijając CLI Tauri
    # całkowicie, ten etap w ogóle się nie odpala: nic tu nie może
    # zawiesić builda dłużej niż sama kompilacja Rustem.
    #
    # Musimy więc ręcznie odtworzyć to, co normalnie robi
    # `beforeBuildCommand` z tauri.conf.json (bo teraz nie ma go kto
    # wywołać): skopiować ikonę i zbudować frontend (`npm run build`
    # -- svelte-check + vite -> dist/, czytane przez src-tauri przez
    # `frontendDist: "../dist"`), a dopiero potem `cargo build`.
    (run (string "cd " repo-root " && mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png"))
    (run (string "cd " repo-root " && npm run build"))
    (run (string "cd " repo-root " && cargo build --release --manifest-path src-tauri/Cargo.toml"))))

(unless (os/stat bin-path :mode)
  (fail (string "nie znaleziono zbudowanej binarki: " bin-path)))

# ---------------------------------------------------------------------
# HackerOS-Comp -- kompozytor Wayland dla tej sesji. To OSOBNY projekt
# (osobne repo, osobny workspace Cargo) -- w odróżnieniu od
# blue-environment (powłoki, budowanej wyżej z repo-root tego samego
# repo), tutaj świeżo KLONUJEMY całe repo z GitHuba i budujemy je
# `cargo build --release` we własnym katalogu roboczym, poza `stage`
# (żeby do pakietu nie wpadło przypadkiem całe sklonowane źródło --
# do stage trafia POTEM wyłącznie sama zbudowana binarka).
# ---------------------------------------------------------------------

(def compositor-repo-url "https://github.com/HackerOS-Linux-System/HackerOS-Comp.git")
(def compositor-bin-name "hackeros-comp")

(def compositor-prebuilt (os/getenv "ZPK_PACKAGING_PREBUILT_COMP_BIN"))

(var compositor-bin-path nil)

(if (and compositor-prebuilt (> (length compositor-prebuilt) 0))
  # Analogicznie do ZPK_PACKAGING_PREBUILT_BIN wyżej -- operator/CI
  # zbudował już HackerOS-Comp wcześniej w tym samym biegu, więc nie
  # klonujemy i nie budujemy drugi raz.
  (set compositor-bin-path compositor-prebuilt)
  (do
    (ensure-git!)
    (ensure-cargo!)

    # -----------------------------------------------------------
    # Nagłówki wymagane przez Smithay (HackerOS-Comp ma
    # backend_udev/backend_gbm włączone w Cargo.toml bezwarunkowo,
    # nawet że domyślnym backendem w main.rs jest winit) -- bez nich
    # `cargo build` pada na etapie linkowania crate'ów *-sys
    # (libwayland/libxkbcommon/libinput/libgbm/libudev/EGL), analogicznie
    # do sekcji webkit2gtk/gtk3/libsoup3 dla blue-environment wyżej.
    # -----------------------------------------------------------
    (unless (try-run "pkg-config --exists wayland-server xkbcommon libinput gbm libudev egl")
      (eprint "recipe.janet: brak nagłówków Wayland/libinput/gbm/udev/EGL wymaganych przez HackerOS-Comp (Smithay) -- próbuję zainstalować (" (detect-pm) ")...")
      (pm-install {:apt "libwayland-dev libxkbcommon-dev libinput-dev libgbm-dev libudev-dev libegl1-mesa-dev libgles2-mesa-dev libseat-dev"
                   :dnf "wayland-devel libxkbcommon-devel libinput-devel mesa-libgbm-devel systemd-devel mesa-libEGL-devel mesa-libGLES-devel libseat-devel"
                   :pacman "wayland libxkbcommon libinput mesa systemd-libs libglvnd seatd"
                   :zypper "wayland-devel libxkbcommon-devel libinput-devel mesa-libgbm-devel libudev-devel Mesa-libEGL-devel libseat-devel"
                   :apk "wayland-dev libxkbcommon-dev libinput-dev mesa-dev eudev-dev libseat-dev"
                   :brew nil})
      (unless (try-run "pkg-config --exists wayland-server xkbcommon libinput gbm libudev egl")
        (eprint "recipe.janet: uwaga -- nadal nie widzę wayland-server/xkbcommon/libinput/gbm/libudev/egl przez pkg-config; jeśli build `cargo` HackerOS-Comp zaraz padnie, doinstaluj je ręcznie dla swojej dystrybucji")))

    # Katalog roboczy pod klon -- NIE `stage` (patrz komentarz wyżej) i
    # NIE `repo-root` (to inne repo, nie ma po co mieszać go z drzewem
    # Blue-Environment). ZPK_PACKAGING_WORKDIR pozwala go nadpisać (np.
    # w CI z ograniczonym /tmp); domyślnie TMPDIR, a w ostateczności /tmp.
    (def work-root (or (os/getenv "ZPK_PACKAGING_WORKDIR") (os/getenv "TMPDIR") "/tmp"))
    (def compositor-src-dir (string work-root "/zpk-build-hackeros-comp-src"))

    # Świeży klon za każdym razem -- prościej i pewniej niż `git pull`
    # w potencjalnie rozjechanym katalogu z poprzedniego, przerwanego builda.
    (try-run (string "rm -rf " compositor-src-dir))
    (run (string "git clone --depth 1 " compositor-repo-url " " compositor-src-dir))
    (run (string "cd " compositor-src-dir " && cargo build --release"))

    (set compositor-bin-path (string compositor-src-dir "/target/release/" compositor-bin-name))))

(unless (os/stat compositor-bin-path :mode)
  (fail (string "nie znaleziono zbudowanej binarki kompozytora: " compositor-bin-path)))

(def share-dir (string stage "/usr/share/Blue-Environment"))
(def apps-dir (string stage "/usr/share/applications"))
(ensure-dir-p share-dir)
(ensure-dir-p apps-dir)

(def dest-bin (string share-dir "/blue-environment"))
(spit dest-bin (slurp bin-path))
(run (string "chmod +x " dest-bin))

(def icon-src (string repo-root "/images/icon.png"))
(when (os/stat icon-src :mode)
  (spit (string share-dir "/icon.png") (slurp icon-src)))

# HackerOS-Comp -- w odróżnieniu od blue-environment (prywatnej binarki
# tej apki, w usr/share/Blue-Environment/) trafia do usr/bin/ jako
# zwykłe, systemowe polecenie.
(def bin-dir (string stage "/usr/bin"))
(ensure-dir-p bin-dir)
(def dest-compositor-bin (string bin-dir "/" compositor-bin-name))
(spit dest-compositor-bin (slurp compositor-bin-path))
(run (string "chmod +x " dest-compositor-bin))

# Wpis .desktop wskazujący bezpośrednio na zainstalowaną binarkę --
# menu/launcher aplikacji (usr/share/applications), NIE sesja logowania
# (ta jest osobno, niżej).
(spit (string apps-dir "/blue-environment.desktop")
      (string "[Desktop Entry]\n"
              "Name=Blue Environment\n"
              "Comment=Graphical environment for HackerOS.\n"
              "Exec=" share-dir "/blue-environment\n"
              "Icon=" share-dir "/icon.png\n"
              "Type=Application\n"
              "Categories=System;\n"))

# Sesja Wayland dla menedżera logowania -- ten pakiet ZAWIERA już
# kompozytor (hackeros-comp, wyżej), więc w odróżnieniu od wcześniejszej
# wersji tego recipe instalujemy też wpis sesji, kopiując GOTOWY plik z
# config/Blue-Environment.desktop (repo Blue-Environment), zamiast
# generować go ręcznie jak wpis dla usr/share/applications powyżej.
(def sessions-dir (string stage "/usr/share/wayland-sessions"))
(ensure-dir-p sessions-dir)
(def session-desktop-src (string repo-root "/config/Blue-Environment.desktop"))
(unless (os/stat session-desktop-src :mode)
  (fail (string "nie znaleziono pliku sesji: " session-desktop-src)))
(spit (string sessions-dir "/Blue-Environment.desktop") (slurp session-desktop-src))

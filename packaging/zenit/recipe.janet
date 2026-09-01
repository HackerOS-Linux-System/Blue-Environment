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
        :apt (try-run (string sudo "apt-get update && " sudo "env DEBIAN_FRONTEND=noninteractive apt-get install -y " pkgs))
        :dnf (try-run (string sudo "dnf install -y " pkgs))
        :pacman (try-run (string sudo "pacman -Sy --noconfirm " pkgs))
        :zypper (try-run (string sudo "zypper --non-interactive install " pkgs))
        :apk (try-run (string sudo "apk add --no-cache " pkgs))
        :brew (try-run (string "brew install " pkgs))
        false))))

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
  # `npm run tauri build` jako osobny krok) -- nie buduj drugi raz.
  # Pomijamy też poniższą logikę instalowania npm/cargo/gtk.
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
      (unless (pm-install {:apt "nodejs npm" :dnf "nodejs npm" :pacman "nodejs npm" :zypper "nodejs npm" :apk "nodejs npm" :brew "node"})
        (eprint "recipe.janet: menedżer pakietów nie dał Node.js -- próbuję NodeSource (setup_22.x)...")
        (when (try-run "curl -fsSL https://deb.nodesource.com/setup_22.x -o /tmp/zpk-nodesource.sh")
          (try-run (string (sudo-) "bash /tmp/zpk-nodesource.sh"))
          (pm-install {:apt "nodejs" :dnf "nodejs" :zypper "nodejs"}))))
    (unless (have? "npm")
      (fail "nie udało się zapewnić 'npm' -- zainstaluj Node.js >= 18 ręcznie i uruchom ponownie"))

    # -----------------------------------------------------------
    # cargo -- pakiet dystrybucyjny, w ostateczności rustup (działa
    # identycznie na każdej dystrybucji, nie wymaga roota).
    # -----------------------------------------------------------
    (unless (have? "cargo")
      (eprint "recipe.janet: brak 'cargo' -- próbuję zainstalować (" (detect-pm) ")...")
      (unless (pm-install {:apt "cargo" :dnf "cargo" :pacman "rust" :zypper "cargo" :apk "cargo" :brew "rust"})
        (eprint "recipe.janet: menedżer pakietów nie ma 'cargo' -- próbuję rustup (oficjalny instalator)...")
        (try-run "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable")
        (def cargo-bin-dir (string (os/getenv "HOME") "/.cargo/bin"))
        (when (os/stat (string cargo-bin-dir "/cargo") :mode)
          (os/setenv "PATH" (string cargo-bin-dir ":" (os/getenv "PATH"))))))
    (unless (have? "cargo")
      (fail "nie udało się zapewnić 'cargo' -- zainstaluj Rust ręcznie (rustup) i uruchom ponownie"))

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
        (eprint "recipe.janet: uwaga -- nadal nie widzę webkit2gtk-4.1/gtk+-3.0/libsoup-3.0 przez pkg-config; jeśli build `cargo`/`tauri` zaraz padnie, doinstaluj je ręcznie dla swojej dystrybucji")))

    (run (string "cd " repo-root " && npm install"))
    # tauri.conf.json's beforeBuildCommand kopiuje ikonę i woła
    # `npm run build` (svelte-check + vite) samo, ale `npm run tauri
    # build` i tak wywołuje ten hook -- jawne kopiowanie tu jest tylko
    # zabezpieczeniem, gdyby ktoś kiedyś odpiął beforeBuildCommand.
    (run (string "cd " repo-root " && mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png"))

    # Bez `--bundles none` -- ta flaga nie jest wspierana przez każdą
    # wersję Tauri CLI (na niektórych `[possible values: deb, rpm,
    # appimage]`, "none" nie istnieje). `npm run tauri build` to
    # dokładnie to samo polecenie co `cmd_build_shell` w build.hl --
    # samo zbuduje też natywne bundle (.deb/.rpm/.AppImage) jako efekt
    # uboczny, jeśli znajdzie odpowiednie narzędzia (dpkg-deb/rpmbuild/
    # linuxdeploy); nas to nie obchodzi, bierzemy tylko surową
    # binarkę. Kompilacja Rustem (cargo) dzieje się PRZED etapem
    # bundlowania, więc nawet jeśli bundlery zawiodą z braku narzędzi
    # (częste poza Debianem/Ubuntu), binarka zwykle i tak już powstała
    # -- dlatego `try-run` zamiast `run`: prawdziwą porażkę i tak
    # złapie poniższe sprawdzenie `os/stat bin-path`.
    (unless (try-run (string "cd " repo-root " && npm run tauri build"))
      (eprint "recipe.janet: uwaga -- 'npm run tauri build' zwróciło błąd (możliwe, że to tylko etap bundlowania .deb/.rpm/.AppImage, nie kompilacja) -- sprawdzam, czy binarka mimo to powstała..."))))

(unless (os/stat bin-path :mode)
  (fail (string "nie znaleziono zbudowanej binarki: " bin-path)))

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

# Wpis .desktop wskazujący bezpośrednio na zainstalowaną binarkę --
# ten pakiet nie zawiera kompozytora Wayland (compositor/) ani CLI
# `blue` (launcher/), więc nie instalujemy sesji w
# usr/share/wayland-sessions/ (patrz build_deb w build.hl dla pełnego
# wariantu, gdy oba te komponenty są obecne).
(spit (string apps-dir "/blue-environment.desktop")
      (string "[Desktop Entry]\n"
              "Name=Blue Environment\n"
              "Comment=Graphical environment for HackerOS.\n"
              "Exec=" share-dir "/blue-environment\n"
              "Icon=" share-dir "/icon.png\n"
              "Type=Application\n"
              "Categories=System;\n"))

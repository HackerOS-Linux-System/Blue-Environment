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
  (set bin-path prebuilt)
  (do
    (run (string "command -v npm >/dev/null 2>&1 || "
                 "{ echo \"recipe.janet: brak 'npm' w PATH -- zainstaluj Node.js\" >&2; exit 1; }"))
    (run (string "command -v cargo >/dev/null 2>&1 || "
                 "{ echo \"recipe.janet: brak 'cargo' w PATH -- zainstaluj Rust (rustup)\" >&2; exit 1; }"))
    (run (string "cd " repo-root " && npm install"))
    # tauri.conf.json's beforeBuildCommand kopiuje ikonę i woła
    # `npm run build` (svelte-check + vite) samo, ale `npm run tauri
    # build` i tak wywołuje ten hook -- jawne kopiowanie tu jest tylko
    # zabezpieczeniem, gdyby ktoś kiedyś odpiął beforeBuildCommand.
    (run (string "cd " repo-root " && mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png"))
    (run (string "cd " repo-root " && npm run tauri build -- --bundles none"))))

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

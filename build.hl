#!/usr/bin/env hl

using <gen 2>

# <main/colors>
# <main/fs>

;; Zależności systemowe

// npm
// cargo
// crystal
// python3
// rpmbuild
// sed
// printf
// mkdir
// cp
// chmod
// rm

;; ============================================
;; KONFIGURACJA
;; ============================================
% BLUE_SHARE: str  = "/usr/share/Blue-Environment"
% BLUE_LIBS: str   = "$(@BLUE_SHARE)/lib"
% BLUE_APPS: str   = "$(@BLUE_SHARE)/apps"
% BLUE_WALLS: str  = "$(@BLUE_SHARE)/wallpapers"
% VERSION: str     = "0.6.0"

;; ============================================
;; FUNKCJE POMOCNICZE
;; ============================================
: check_root def
    > id -u |> @uid
    ?~ @uid != 0
        ::red "✗ This command requires root privileges"
        exit 1
    done
done

: echo_info def
    ~> @COLOR_CYAN$1@COLOR_RESET
done

: echo_success def
    ::green "  ✓ $1"
done

: echo_error def
    ::red "  ✗ $1"
done

: echo_build def
    ::bold "  → $1"
done

;; ============================================
;; HELP — wyświetla dostępne komendy
;; ============================================
: cmd_help def
    ::hr 40
    ~> @COLOR_BOLDBlue Environment v@VERSION — Build System@COLOR_RESET
    ::hr 40
    ~> "  build                 Normal build"
    ~> "  build-hackeros        HackerOS variant"
    ~> "  build-legendaryos     LegendaryOS .rpm"
    ~> "  build-fedora          Fedora .rpm"
    ~> "  install               Install to system (sudo)"
    ~> "  clean                 Clean artifacts"
    ~> "  uninstall             Remove from system (sudo)"
    ::hr 40
done

;; ============================================
;; BUILD — kompilacja główna
;; ============================================
: cmd_build def
    echo_info "Building Blue Environment v@VERSION..."

    ;; npm build
    echo_build "npm build"
    > npm run build
    ? ok
        echo_success "npm build complete"
    ? err
        echo_error "npm build failed"
        exit 1
    done

    ;; Cargo build — blue-environment
    echo_build "cargo build — blue-environment"
    > cargo build --release --manifest-path src-tauri/Cargo.toml
    ? ok
        echo_success "blue-environment compiled"
    ? err
        echo_error "cargo build failed"
        exit 1
    done

    ;; Cargo build — blue-compositor
    echo_build "cargo build — blue-compositor"
    > cargo build --release --manifest-path src-tauri/lib/blue-compositor/Cargo.toml
    ? ok
        echo_success "blue-compositor compiled"
    ? err
        echo_error "compositor build failed"
        exit 1
    done

    ;; Crystal launcher
    echo_build "crystal launcher"
    ::which crystal
    ? ok
        > crystal build launcher/src/main.cr -o blue --release 2>/dev/null
        ? ok
            echo_success "blue launcher"
        ? err
            echo_error "crystal build failed"
        done
    ? err
        echo_error "crystal not found — skipping launcher"
    done

    ::green "✓ Build complete."
done

;; ============================================
;; BUILD-HACKEROS — wariant HackerOS
;; ============================================
: cmd_build_hackeros def
    echo_info "Building HackerOS variant..."

    ;; Patch
    > python3 scripts/patch-hackeros.py
    ? err
        echo_error "Patching failed"
        exit 1
    done
    echo_success "Patched for HackerOS"

    ;; Build
    -- cmd_build

    ;; Unpatch
    > python3 scripts/unpatch.py
    echo_success "Restored original files"
done

;; ============================================
;; BUILD-LEGENDARYOS — RPM dla LegendaryOS
;; ============================================
: cmd_build_legendaryos def
    echo_info "Building LegendaryOS RPM..."

    ;; Check rpmbuild
    ::which rpmbuild
    ? err
        echo_error "rpmbuild not found"
        exit 1
    done

    ;; Patch
    > python3 scripts/patch-legendaryos.py
    ? err
        echo_error "Patching failed"
        exit 1
    done

    ;; Build
    -- cmd_build

    ;; Unpatch
    > python3 scripts/unpatch.py

    ;; Prepare RPM structure
    > mkdir -p rpmbuild/BUILD rpmbuild/RPMS rpmbuild/SOURCES rpmbuild/SPECS rpmbuild/SRPMS

    ;; Generate spec file
    > sed 's/@VERSION@/@VERSION/g' packaging/blue-environment-legendaryos.spec > rpmbuild/SPECS/blue-environment.spec
    > sed -i "s/@VERSION/@VERSION/g" rpmbuild/SPECS/blue-environment.spec
    > rpmbuild -bb rpmbuild/SPECS/blue-environment.spec --define "_topdir $(pwd)/rpmbuild"

    ? ok
        echo_success "RPM built in rpmbuild/RPMS/"
    ? err
        echo_error "RPM build failed"
    done
done

;; ============================================
;; BUILD-FEDORA — RPM dla Fedory
;; ============================================
: cmd_build_fedora def
    echo_info "Building Fedora RPM..."

    ::which rpmbuild
    ? err
        echo_error "rpmbuild not found"
        exit 1
    done

    -- cmd_build

    > mkdir -p rpmbuild/BUILD rpmbuild/RPMS rpmbuild/SOURCES rpmbuild/SPECS rpmbuild/SRPMS
    > sed 's/@VERSION@/@VERSION/g' packaging/blue-environment-fedora.spec > rpmbuild/SPECS/blue-environment.spec
    > sed -i "s/@VERSION/@VERSION/g" rpmbuild/SPECS/blue-environment.spec
    > rpmbuild -bb rpmbuild/SPECS/blue-environment.spec --define "_topdir $(pwd)/rpmbuild"

    ? ok
        echo_success "RPM built"
    ? err
        echo_error "RPM build failed"
    done
done

;; ============================================
;; INSTALL — instalacja systemowa
;; ============================================
: cmd_install def
    -- check_root

    echo_info "Installing Blue Environment v@VERSION..."

    ;; Create directories
    > mkdir -p @BLUE_LIBS @BLUE_APPS @BLUE_WALLS
    > mkdir -p /usr/share/wayland-sessions /usr/share/applications

    ;; Install blue-environment binary
    ::exists src-tauri/target/release/blue-environment
    ? ok
        > cp src-tauri/target/release/blue-environment @BLUE_SHARE/blue-environment
        > chmod 755 @BLUE_SHARE/blue-environment
        echo_success "blue-environment"
    ? err
        echo_error "binary missing"
    done

    ;; Install blue-compositor
    ::exists src-tauri/lib/blue-compositor/target/release/blue-compositor
    ? ok
        > cp src-tauri/lib/blue-compositor/target/release/blue-compositor @BLUE_LIBS/blue-compositor
        > chmod 755 @BLUE_LIBS/blue-compositor
        echo_success "blue-compositor"
    ? err
        echo_error "compositor missing"
    done

    ;; Install CLI
    ::exists blue
    ? ok
        > cp blue /usr/local/bin/blue
        > chmod 755 /usr/local/bin/blue
        echo_success "blue CLI"
    done

    ;; Install wallpapers
    ::isdir wallpapers
    ? ok
        > cp -r wallpapers/. @BLUE_WALLS/
        echo_success "wallpapers"
    done

    ;; Create wayland session desktop file
    > printf '[Desktop Entry]\nName=Blue Environment\nExec=@BLUE_LIBS/blue-compositor\nType=Application\nDesktopNames=Blue\nVersion=@VERSION\n' > /usr/share/wayland-sessions/blue-environment.desktop

    ;; Create app desktop file
    > printf '[Desktop Entry]\nName=Blue Environment\nExec=@BLUE_SHARE/blue-environment\nIcon=@BLUE_SHARE/icon.png\nType=Application\nCategories=System;\n' > /usr/share/applications/blue-environment.desktop

    ;; Write version file
    > printf "%s" "@VERSION" > @BLUE_SHARE/.version

    ::green "✓ Installation complete! Run: blue start"
done

;; ============================================
;; UNINSTALL — odinstalacja
;; ============================================
: cmd_uninstall def
    -- check_root

    echo_info "Uninstalling Blue Environment..."

    > rm -rf @BLUE_SHARE
    > rm -f /usr/share/wayland-sessions/blue-environment.desktop
    > rm -f /usr/share/applications/blue-environment.desktop
    > rm -f /usr/local/bin/blue

    ::yellow "✓ Uninstalled. User configs in ~/.config/Blue-Environment kept."
done

;; ============================================
;; CLEAN — czyszczenie artifactów
;; ============================================
: cmd_clean def
    echo_info "Cleaning build artifacts..."

    > rm -rf src-tauri/target src-tauri/lib/blue-compositor/target dist blue rpmbuild

    ::green "✓ Clean complete."
done

;; ============================================
;; MAIN — parsowanie argumentów
;; ============================================
% action: str = @arg0

? switch @action
| help
    -- cmd_help
| build
    -- cmd_build
| build-hackeros
    -- cmd_build_hackeros
| build-legendaryos
    -- cmd_build_legendaryos
| build-fedora
    -- cmd_build_fedora
| install
    -- cmd_install
| uninstall
    -- cmd_uninstall
| clean
    -- cmd_clean
| *
    ?~ @action != ""
        ::red "✗ Unknown command: @action"
        ::yellow "Run: hl $0 help"
        exit 1
    | _
        -- cmd_help
    done
done

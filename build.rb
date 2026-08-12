#!/usr/bin/ruby
# frozen_string_literal: true

require 'fileutils'

# ============================================
# KOLORY I FORMATOWANIE ANSI
# ============================================
COLOR_RESET   = "\e[0m"
COLOR_BOLD    = "\e[1m"
COLOR_RED     = "\e[31m"
COLOR_GREEN   = "\e[32m"
COLOR_YELLOW  = "\e[33m"
COLOR_CYAN    = "\e[36m"

# ============================================
# KONFIGURACJA
# ============================================
BLUE_SHARE = "/usr/share/Blue-Environment"
BLUE_LIBS  = "#{BLUE_SHARE}/lib"
BLUE_APPS  = "#{BLUE_SHARE}/apps"
BLUE_WALLS = "#{BLUE_SHARE}/wallpapers"
VERSION    = "0.7.0"

SHELL_MANIFEST       = "src-tauri/Cargo.toml"
# Expects a `compositor/` subdirectory populated with the Blue-Compositor
# source tree (vendored copy or git submodule) at build time — this repo
# only builds it, it doesn't own that source. See compositor/ROADMAP.md
# (once vendored in) for what's implemented there, including this pass's
# additions: real dmabuf modifier negotiation, IME popup positioning
# (ibus/fcitx5 bridge), and HDR/color-management protocol scaffolding.
COMPOSITOR_MANIFEST  = "compositor/Cargo.toml"
SHELL_BIN_PATH       = "target/release/blue-environment"
COMPOSITOR_BIN_PATH  = "compositor/target/release/blue-compositor"

# ============================================
# FUNKCJE POMOCNICZE
# ============================================
def check_root
  uid = `id -u`.strip.to_i
  if uid != 0
    puts "#{COLOR_RED}✗ This command requires root privileges#{COLOR_RESET}"
    exit 1
  end
end

def echo_info(msg)
  puts "#{COLOR_CYAN}#{msg}#{COLOR_RESET}"
end

def echo_success(msg)
  puts "#{COLOR_GREEN}  ✓ #{msg}#{COLOR_RESET}"
end

def echo_error(msg)
  puts "#{COLOR_RED}  ✗ #{msg}#{COLOR_RESET}"
end

def echo_warn(msg)
  puts "#{COLOR_YELLOW}  ! #{msg}#{COLOR_RESET}"
end

def echo_build(msg)
  puts "#{COLOR_BOLD}  → #{msg}#{COLOR_RESET}"
end

def print_hr(length = 56)
  puts "—" * length
end

def run_cmd(cmd)
  system(cmd)
  $?.success?
end

def command_exists?(cmd)
  system("which #{cmd} > /dev/null 2>&1")
end

# ============================================
# HELP — wyświetla dostępne komendy
# ============================================
def cmd_help
  print_hr
  puts "#{COLOR_BOLD}Blue Environment v#{VERSION} — Build System#{COLOR_RESET}"
  print_hr
  puts "  #{COLOR_BOLD}ruby build.rb#{COLOR_RESET}                Show this list"
  puts "  #{COLOR_BOLD}ruby build.rb desktop#{COLOR_RESET}        Build shell + compositor, package .rpm + .deb + Arch"
  puts "                              (auto-detects rpmbuild / dpkg-deb / makepkg)"
  puts "  #{COLOR_BOLD}ruby build.rb shell#{COLOR_RESET}           npm install + npm run tauri build (fastest)"
  puts "  #{COLOR_BOLD}ruby build.rb shell debian#{COLOR_RESET}    Shell build — Blue Software uses APT backend"
  puts "  #{COLOR_BOLD}ruby build.rb shell arch#{COLOR_RESET}      Shell build — Blue Software uses Pacman backend"
  puts "  #{COLOR_BOLD}ruby build.rb shell fedora#{COLOR_RESET}    Shell build — Blue Software uses DNF backend"
  puts "  #{COLOR_BOLD}ruby build.rb shell opensuse#{COLOR_RESET}  Shell build — Blue Software uses Zypper backend"
  puts "  #{COLOR_BOLD}ruby build.rb shell rpm-ostree#{COLOR_RESET} Shell build — Blue Software uses rpm-ostree backend"
  puts "  #{COLOR_BOLD}ruby build.rb android#{COLOR_RESET}         Build the Android / mobile shell (.apk)"
  puts "                              Requires ANDROID_HOME or ~/Android/Sdk"
  puts "  #{COLOR_BOLD}ruby build.rb tv#{COLOR_RESET}              Build the TV / Big Picture shell"
  puts "                              10-foot UI, gamepad navigation"
  puts "  #{COLOR_BOLD}ruby build.rb BEDM#{COLOR_RESET}            Build BEDM (Blue Environment Display Manager)"
  puts "  #{COLOR_BOLD}ruby build.rb compositor#{COLOR_RESET}      Build only the Wayland compositor"
  puts "  #{COLOR_BOLD}ruby build.rb packages [fmt]#{COLOR_RESET}  Package already-built binaries only"
  puts "                              fmt = rpm | deb | arch | apk | opensuse  (default: all available)"
  puts "  #{COLOR_BOLD}ruby build.rb app <name>#{COLOR_RESET}      Build one Blue app standalone (no full shell)"
  puts "  #{COLOR_BOLD}ruby build.rb app#{COLOR_RESET}             List all standalone-buildable apps"
  puts "  #{COLOR_BOLD}ruby build.rb all#{COLOR_RESET}             Build compositor + desktop shell (no .rpm)"
  print_hr
  puts "  #{COLOR_BOLD}ruby build.rb check#{COLOR_RESET}           svelte-check only (fast — no vite build, no cargo, no packaging)"
  puts "  #{COLOR_BOLD}ruby build.rb doctor#{COLOR_RESET}          Check build-time deps + IME runtime (ibus/fcitx5)"
  print_hr
  puts "  #{COLOR_BOLD}Maintenance:#{COLOR_RESET}"
  puts "    install                  Install the desktop build to the system (sudo)"
  puts "    uninstall                Remove it from the system (sudo)"
  puts "    clean                    Remove build artifacts"
  print_hr
end

# ============================================
# WSPÓLNE KROKI BUDOWANIA
# ============================================

# npm install + tsc + vite build (the React/Tauri shell frontend)
def build_frontend
  unless File.directory?("node_modules")
    echo_build "npm install"
    unless run_cmd("npm install")
      echo_error "npm install failed"
      return false
    end
  end
  echo_build "npm run build (svelte-check + vite)"
  if run_cmd("npm run build")
    echo_success "frontend build complete"
    true
  else
    echo_error "frontend build failed"
    echo_warn  "Run 'ruby build.rb check' on its own for just the svelte-check step (faster to"
    echo_warn  "iterate on than a full build) — see its output for the specific file/line."
    false
  end
end

# ============================================
# FRONTEND CHECK — svelte-check only, no vite build, no cargo, no
# packaging. Exists because `npm run build`'s `svelte-check && vite build`
# only reports svelte-check's failure via a generic non-zero exit from
# Tauri's `beforeBuildCommand` unless you're watching the raw npm output —
# this surfaces it directly and, since translation files are plain .ts
# (no Svelte-specific tooling needed to catch a syntax error in them),
# catches exactly the class of bug that broke french.ts/italian.ts here:
# an apostrophe inside a single-quoted string value (`'l'HDR'`) that
# TypeScript parses as the string terminating early, not an unescaped
# accent or unusual char — the same mistake is easy to reintroduce
# whenever new translated strings get added by hand across many locale
# files at once.
# ============================================
def cmd_check_frontend
  echo_info "Checking Blue Environment frontend (svelte-check only — no vite build)"
  print_hr
  unless File.directory?("node_modules")
    echo_build "npm install (node_modules missing)"
    unless run_cmd("npm install")
      echo_error "npm install failed"
      return false
    end
  end
  echo_build "svelte-check --tsconfig ./tsconfig.json"
  if run_cmd("npx svelte-check --tsconfig ./tsconfig.json")
    echo_success "svelte-check passed"
    true
  else
    echo_error "svelte-check failed (see file:line above)."
    echo_warn  "If the error is in src/lib/translations/*.ts and mentions \"',' expected\"" \
               " / \"Expression expected\" / \"shorthand property\": check for an" \
               " apostrophe inside a single-quoted string value — use double quotes" \
               " for that value instead (e.g. \"l'HDR\" not 'l'HDR')."
    false
  end
end

def build_shell_binary
  echo_build "cargo build — blue-environment (shell)"
  if run_cmd("cargo build --release --manifest-path #{SHELL_MANIFEST}")
    echo_success "blue-environment compiled"
    true
  else
    echo_error "shell build failed"
    false
  end
end

# The compositor links directly against several native libraries
# (libinput, libgbm/mesa, libudev, libseat, libxkbcommon, libdrm,
# wayland) that Rust/cargo cannot install for you — they have to already
# be present as system *development* packages (the `-dev`/`-devel`
# variants with headers + linkable .so symlinks, not just the runtime
# libraries most systems ship by default). Missing ones surface as a
# late, cryptic linker error like:
#   cannot find -linput: No such file or directory
#   cannot find -lgbm: No such file or directory
# after several minutes of otherwise-successful compilation — which is
# confusing because it looks like a code/build-system problem when it's
# actually just a missing system package. This checks up front via
# pkg-config and fails fast with the exact install command for the
# detected distro instead.
COMPOSITOR_PKG_CONFIG_DEPS = {
  "libinput"    => "libinput",
  "gbm"         => "mesa/gbm (EGL/GBM)",
  "libudev"     => "udev",
  "libseat"     => "seatd/libseat",
  "xkbcommon"   => "libxkbcommon",
  "libdrm"      => "libdrm",
  "wayland-server" => "wayland",
}.freeze

def detect_distro_family
  return :unknown unless File.exist?("/etc/os-release")
  os_release = File.read("/etc/os-release")
  id_like = os_release[/^ID_LIKE=(.*)$/, 1].to_s.delete('"')
  id      = os_release[/^ID=(.*)$/, 1].to_s.delete('"')
  combined = "#{id} #{id_like}".downcase
  return :debian  if combined.include?("debian") || combined.include?("ubuntu")
  return :fedora  if combined.include?("fedora") || combined.include?("rhel")
  return :arch    if combined.include?("arch")
  return :suse    if combined.include?("suse")
  return :alpine  if combined.include?("alpine")
  :unknown
end

def compositor_install_hint
  case detect_distro_family
  when :debian
    "sudo apt install pkg-config libinput-dev libgbm-dev libudev-dev libseat-dev libxkbcommon-dev libdrm-dev libwayland-dev libegl1-mesa-dev libgles2-mesa-dev"
  when :fedora
    "sudo dnf install pkgconf-pkg-config libinput-devel mesa-libgbm-devel systemd-devel libseat-devel libxkbcommon-devel libdrm-devel wayland-devel mesa-libEGL-devel mesa-libGLES-devel"
  when :arch
    "sudo pacman -S --needed pkgconf libinput mesa systemd-libs seatd libxkbcommon libdrm wayland"
  when :suse
    "sudo zypper install pkg-config libinput-devel Mesa-libgbm-devel libudev-devel libseat-devel libxkbcommon-devel libdrm-devel wayland-devel"
  when :alpine
    "sudo apk add pkgconf libinput-dev mesa-dev eudev-dev seatd-dev libxkbcommon-dev libdrm-dev wayland-dev"
  else
    "install development packages (headers + .so symlinks, e.g. '-dev'/'-devel' variants) for: libinput, mesa/gbm, libudev, libseat, libxkbcommon, libdrm, wayland"
  end
end

def check_compositor_build_deps
  unless system("which pkg-config > /dev/null 2>&1")
    echo_error "pkg-config not found — needed to even check for the compositor's native dependencies."
    echo_warn  "Install it first, e.g.: #{compositor_install_hint}"
    return false
  end

  missing = COMPOSITOR_PKG_CONFIG_DEPS.keys.reject { |lib| system("pkg-config --exists #{lib} 2>/dev/null") }
  return true if missing.empty?

  echo_error "Missing native development libraries needed to build the compositor:"
  missing.each { |lib| echo_warn "  - #{lib} (#{COMPOSITOR_PKG_CONFIG_DEPS[lib]})" }
  echo_warn "Without these, cargo will compile for several minutes and then fail at the *linking* step"
  echo_warn "with an error like 'cannot find -linput' / 'cannot find -lgbm' — this check exists to catch"
  echo_warn "that up front instead. Install them with:"
  puts "#{COLOR_BOLD}    #{compositor_install_hint}#{COLOR_RESET}"
  false
end

def build_compositor_binary
  echo_build "cargo build — blue-compositor"
  unless check_compositor_build_deps
    echo_error "compositor build aborted — missing native dependencies (see above)"
    return false
  end
  if run_cmd("cargo build --release --manifest-path #{COMPOSITOR_MANIFEST}")
    echo_success "blue-compositor compiled"
    true
  else
    echo_error "compositor build failed"
    false
  end
end

# ============================================
# RUNTIME CHECK — things that aren't build-time deps but affect what
# actually works once the compositor is running: an IME engine for the
# ibus/fcitx5 bridge (protocols/input_method.rs on the compositor side),
# and the i18n locale coverage in this shell's own translations/.
# Advisory only — never blocks a build, since none of this is required to
# compile or even to run (the compositor works fine with no IME running,
# it just won't have a candidate window to show).
# ============================================
def check_ime_runtime
  ibus = command_exists?("ibus-daemon") || command_exists?("ibus")
  fcitx5 = command_exists?("fcitx5")
  if ibus || fcitx5
    echo_success "IME engine found (#{[("ibus" if ibus), ("fcitx5" if fcitx5)].compact.join(' + ')}) — candidate-window bridge in the compositor can attach to it"
  else
    echo_warn "No ibus or fcitx5 found on PATH. The compositor's IME candidate-window support " \
              "(compositor/src/protocols/input_method.rs) has nothing to bridge to without one — " \
              "install ibus (+ ibus-wayland, on distros that split it out) or fcitx5 to use CJK/other IME input."
  end
end

def cmd_doctor
  echo_info "Blue Environment — environment check"
  print_hr
  echo_build "Build-time native dependencies (compositor)"
  check_compositor_build_deps
  echo_build "Runtime: input method engine"
  check_ime_runtime
  print_hr
end

# The 'blue' CLI launcher is a self-contained Ruby script (launcher/main.rb)
# — there is no compiled launcher binary to build, it just needs to be
# staged as an executable named 'blue'.
def build_launcher_cli
  echo_build "staging blue CLI launcher (main.rb + src/)"
  unless File.exist?("launcher/main.rb")
    echo_error "launcher/main.rb missing — skipping CLI"
    return false
  end

  # The launcher is a Ruby source tree, not a compiled binary. We stage
  # it to BLUE_LIBS/blue/ where the wrapper script (launcher/blue →
  # /usr/local/bin/blue) will find it.
  FileUtils.mkdir_p("staging/blue")
  FileUtils.cp("launcher/main.rb", "staging/blue/main.rb")
  FileUtils.cp_r("launcher/src", "staging/blue/src")
  FileUtils.cp("launcher/blue", "blue")
  FileUtils.chmod(0755, "blue")
  echo_success "blue CLI staged (staging/blue/ + blue wrapper)"
  true
end

def stage_rpm_sources(topdir)
  FileUtils.mkdir_p("#{topdir}/SOURCES")
  FileUtils.cp(SHELL_BIN_PATH, "#{topdir}/SOURCES/blue-environment")
  FileUtils.cp(COMPOSITOR_BIN_PATH, "#{topdir}/SOURCES/blue-compositor")
  FileUtils.cp("blue", "#{topdir}/SOURCES/blue")
end

def build_rpm(spec_path, topdir, label)
  FileUtils.mkdir_p(["#{topdir}/BUILD", "#{topdir}/RPMS", "#{topdir}/SOURCES", "#{topdir}/SPECS", "#{topdir}/SRPMS"])
  stage_rpm_sources(topdir)

  spec_out = "#{topdir}/SPECS/blue-environment.spec"
  run_cmd("sed 's/@VERSION@/#{VERSION}/g' #{spec_path} > #{spec_out}")

  pwd = Dir.pwd
  if run_cmd("rpmbuild -bb #{spec_out} --define \"_topdir #{pwd}/#{topdir}\"")
    echo_success "#{label} RPM → #{topdir}/RPMS/"
    true
  else
    echo_error "#{label} RPM build failed"
    false
  end
end

# ============================================
# .DEB — Debian / Ubuntu package (dpkg-deb, no full debhelper needed)
# ============================================
DEB_CONTROL_DIR = "packaging/blue-environment-debian"

def stage_deb_tree(root)
  FileUtils.rm_rf(root)
  FileUtils.mkdir_p("#{root}/DEBIAN")
  FileUtils.mkdir_p("#{root}/usr/share/Blue-Environment/lib/blue/src")
  FileUtils.mkdir_p("#{root}/usr/share/wayland-sessions")
  FileUtils.mkdir_p("#{root}/usr/share/applications")
  FileUtils.mkdir_p("#{root}/usr/local/bin")

  FileUtils.install(SHELL_BIN_PATH, "#{root}/usr/share/Blue-Environment/blue-environment", mode: 0755)
  FileUtils.install(COMPOSITOR_BIN_PATH, "#{root}/usr/share/Blue-Environment/lib/blue-compositor", mode: 0755)
  FileUtils.install("blue", "#{root}/usr/local/bin/blue", mode: 0755)
  FileUtils.install("launcher/main.rb", "#{root}/usr/share/Blue-Environment/lib/blue/main.rb", mode: 0644)
  FileUtils.cp_r("launcher/src/.", "#{root}/usr/share/Blue-Environment/lib/blue/src")

  File.write("#{root}/usr/share/wayland-sessions/blue-environment.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/lib/blue-compositor\nType=Application\nDesktopNames=Blue\n")
  File.write("#{root}/usr/share/applications/blue-environment.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/blue-environment\nIcon=/usr/share/Blue-Environment/icon.png\nType=Application\nCategories=System;\n")
end

def build_deb(label = "Debian/Ubuntu")
  unless command_exists?("dpkg-deb")
    echo_error "dpkg-deb not found — install the 'dpkg-dev' package to produce .deb files"
    return false
  end

  root = "build/deb/blue-environment"
  stage_deb_tree(root)

  control_src = File.read("#{DEB_CONTROL_DIR}/control")
  # dpkg-deb wants a single-stanza DEBIAN/control — take the binary
  # "Package:" stanza (the second paragraph of the dpkg-source-style file
  # we keep in packaging/ for reference) and inject the real version.
  pkg_stanza = control_src.split(/\n\n/).last
  File.write("#{root}/DEBIAN/control", "#{pkg_stanza}\nVersion: #{VERSION}\n")
  FileUtils.install("#{DEB_CONTROL_DIR}/postinst", "#{root}/DEBIAN/postinst", mode: 0755) if File.exist?("#{DEB_CONTROL_DIR}/postinst")

  out = "dist/blue-environment_#{VERSION}_amd64.deb"
  FileUtils.mkdir_p("dist")
  if run_cmd("dpkg-deb --root-owner-group --build #{root} #{out}")
    echo_success "#{label} DEB → #{out}"
    true
  else
    echo_error "#{label} DEB build failed"
    false
  end
end

# ============================================
# .pkg.tar.zst — Arch Linux / LegendaryOS-Arch package (makepkg)
# ============================================
def build_arch_pkg(label = "Arch Linux")
  unless command_exists?("makepkg")
    echo_error "makepkg not found — this target only works on Arch-based systems"
    return false
  end

  pkgdir = "build/arch"
  FileUtils.mkdir_p(pkgdir)
  FileUtils.cp(SHELL_BIN_PATH, "#{pkgdir}/blue-environment")
  FileUtils.cp(COMPOSITOR_BIN_PATH, "#{pkgdir}/blue-compositor")
  FileUtils.cp("blue", "#{pkgdir}/blue")
  File.write("#{pkgdir}/blue-environment.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/blue-environment\nIcon=/usr/share/Blue-Environment/icon.png\nType=Application\nCategories=System;\n")
  File.write("#{pkgdir}/blue-environment-session.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/lib/blue-compositor\nType=Application\nDesktopNames=Blue\n")
  run_cmd("sed 's/@VERSION@/#{VERSION}/g' packaging/PKGBUILD > #{pkgdir}/PKGBUILD")

  pwd = Dir.pwd
  if run_cmd("cd #{pkgdir} && makepkg -f --skipinteg SRCDEST=#{pwd}/#{pkgdir}")
    echo_success "#{label} package → #{pkgdir}/*.pkg.tar.zst"
    true
  else
    echo_error "#{label} package build failed"
    false
  end
end

# ============================================
# .apk — Alpine Linux package (abuild)
# ============================================
def build_apk(label = "Alpine")
  unless command_exists?("abuild")
    echo_error "abuild not found — install the 'alpine-sdk' package (on Alpine) to produce .apk files"
    return false
  end

  root = "build/apk"
  FileUtils.rm_rf(root)
  FileUtils.mkdir_p(root)
  FileUtils.cp(SHELL_BIN_PATH, "#{root}/blue-environment")
  FileUtils.cp(COMPOSITOR_BIN_PATH, "#{root}/blue-compositor")
  FileUtils.cp("blue", "#{root}/blue")
  File.write("#{root}/blue-environment.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/blue-environment\nIcon=/usr/share/Blue-Environment/icon.png\nType=Application\nCategories=System;\n")
  File.write("#{root}/blue-environment-session.desktop",
    "[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/lib/blue-compositor\nType=Application\nDesktopNames=Blue\n")
  run_cmd("sed 's/@VERSION@/#{VERSION}/g' packaging/APKBUILD > #{root}/APKBUILD")

  pwd = Dir.pwd
  # abuild expects to run as a non-root build user with its own signing
  # key (`abuild-keygen`) already set up — that setup is environment
  # -specific (CI provisions it; see .github/workflows/build.yml's
  # `alpine-musl` job) so it's assumed to already exist here rather than
  # generated inline.
  if run_cmd("cd #{root} && abuild-keygen -n -a 2>/dev/null; SRCDEST=#{pwd}/#{root} abuild -r")
    echo_success "#{label} package → ~/packages/ (abuild's default output dir)"
    true
  else
    echo_error "#{label} package build failed"
    false
  end
end

# ============================================
# openSUSE / SLE — separate spec from Fedora (see packaging/opensuse/)
# ============================================
def build_opensuse_rpm(label = "openSUSE")
  build_rpm("packaging/opensuse/blue-environment.spec", "rpmbuild/opensuse", label)
end

# ============================================
# DESKTOP — shell + compositor, paczki .rpm (Fedora + LegendaryOS)
# ============================================
def cmd_build_desktop
  echo_info "Building Blue Environment v#{VERSION} — Desktop (all installable package formats)"

  exit 1 unless build_frontend
  exit 1 unless build_shell_binary
  exit 1 unless build_compositor_binary
  build_launcher_cli

  results = {}

  if command_exists?("rpmbuild")
    echo_build "packaging Fedora .rpm"
    results["Fedora .rpm"] = build_rpm("packaging/blue-environment-fedora.spec", "rpmbuild/fedora", "Fedora")
    echo_build "packaging LegendaryOS .rpm"
    results["LegendaryOS .rpm"] = build_rpm("packaging/blue-environment-legendaryos.spec", "rpmbuild/legendaryos", "LegendaryOS")
  else
    echo_warn "rpmbuild not found — skipping .rpm (install the 'rpm-build' package to enable)"
  end

  if command_exists?("dpkg-deb")
    echo_build "packaging .deb"
    results["Debian .deb"] = build_deb
  else
    echo_warn "dpkg-deb not found — skipping .deb (install the 'dpkg-dev' package to enable)"
  end

  if command_exists?("makepkg")
    echo_build "packaging Arch .pkg.tar.zst"
    results["Arch package"] = build_arch_pkg
  else
    echo_warn "makepkg not found — skipping Arch package (only available on Arch-based systems)"
  end

  if command_exists?("abuild")
    echo_build "packaging Alpine .apk"
    results["Alpine .apk"] = build_apk
  else
    echo_warn "abuild not found — skipping .apk (only available on Alpine, needs the 'alpine-sdk' package)"
  end

  if command_exists?("rpmbuild") && File.exist?("/etc/os-release") && File.read("/etc/os-release").match?(/suse/i)
    echo_build "packaging openSUSE .rpm"
    results["openSUSE .rpm"] = build_opensuse_rpm
  end

  if results.empty?
    echo_error "No packaging tool found (need at least one of: rpmbuild, dpkg-deb, makepkg)"
    exit 1
  end

  print_hr
  results.each { |label, ok| ok ? echo_success(label) : echo_error(label) }
  print_hr

  if results.values.all?
    puts "#{COLOR_GREEN}✓ Desktop build complete — #{results.size} package format(s) produced.#{COLOR_RESET}"
  else
    echo_warn "Desktop build finished with errors on some formats — see above"
    exit 1
  end
end

# ============================================
# PACKAGES — build only the installer packages (assumes binaries already built)
# ============================================
def cmd_build_packages(format = nil)
  case format
  when "rpm"
    exit 1 unless command_exists?("rpmbuild")
    build_rpm("packaging/blue-environment-fedora.spec", "rpmbuild/fedora", "Fedora") &&
      build_rpm("packaging/blue-environment-legendaryos.spec", "rpmbuild/legendaryos", "LegendaryOS")
  when "deb"
    exit 1 unless command_exists?("dpkg-deb")
    build_deb
  when "arch", "pacman"
    exit 1 unless command_exists?("makepkg")
    build_arch_pkg
  when "apk", "alpine"
    exit 1 unless command_exists?("abuild")
    build_apk
  when "opensuse", "suse"
    exit 1 unless command_exists?("rpmbuild")
    build_opensuse_rpm
  when nil, "all"
    cmd_build_desktop
  else
    echo_error "Unknown package format '#{format}'. Use: rpm, deb, arch, apk, opensuse"
    exit 1
  end
end

# ============================================
# TV — jak 'desktop', ale z interfejsem Big Picture
# ============================================
def cmd_build_tv
  echo_info "Building Blue Environment v#{VERSION} — TV / Big Picture Shell"
  ENV['VITE_BLUE_UI_MODE']     = 'bigpicture'
  ENV['VITE_BLUE_ALTERNATIVE'] = 'tv'
  echo_build "UI mode: VITE_BLUE_UI_MODE=bigpicture, VITE_BLUE_ALTERNATIVE=tv"

  unless File.exist?('src/alternatives/tv/main.tsx')
    echo_error "TV shell not found — src/alternatives/tv/ was removed with the React source and has not been rebuilt in Svelte yet (see STATUS.md)"
    exit 1
  end

  # The TV shell uses the standard Tauri build pipeline but with env vars
  # that swap the entry point via src/main.tsx conditional import.
  echo_build "npm install (if needed)"
  run_cmd("npm install --prefer-offline 2>/dev/null || npm install")

  echo_build "Copying icon assets"
  run_cmd("mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png 2>/dev/null || true")

  echo_build "npm run build — TV shell"
  unless run_cmd("npm run build")
    echo_error "TV frontend build failed"
    exit 1
  end
  echo_success "TV frontend built"

  echo_build "cargo build — Tauri shell binary (TV)"
  shell_ok = Dir.chdir('src-tauri') { run_cmd("cargo build --release") }
  unless shell_ok
    echo_error "Shell binary build failed"
    exit 1
  end
  echo_success "Shell binary built"

  echo_success "TV / Big Picture build complete"
  echo_info "Binary: target/release/blue-environment"
end

# ============================================
# ANDROID — alternatywna powłoka + .apk
# ============================================
def cmd_build_android
  echo_info "Building Blue Environment v#{VERSION} — Android / Mobile Shell"
  ENV['VITE_BLUE_UI_MODE']     = 'mobile'
  ENV['VITE_BLUE_ALTERNATIVE'] = 'android'

  unless command_exists?("npx")
    echo_error "npx not found — install Node.js + npm first"
    exit 1
  end

  # Auto-detect Android SDK in standard locations
  sdk_candidates = [
    ENV['ANDROID_HOME'],
    ENV['ANDROID_SDK_ROOT'],
    File.expand_path('~/Android/Sdk'),
    File.expand_path('~/android/sdk'),
    '/opt/android-sdk',
    '/usr/lib/android-sdk',
  ].compact.select { |p| p && File.directory?(p) }

  if sdk_candidates.empty?
    echo_error "Android SDK not found. Searched: ~/Android/Sdk, /opt/android-sdk, etc."
    echo_info  "Install via Android Studio or: https://developer.android.com/studio"
    exit 1
  end

  sdk_path = sdk_candidates.first
  ENV['ANDROID_HOME']     ||= sdk_path
  ENV['ANDROID_SDK_ROOT'] ||= sdk_path
  echo_success "Android SDK: #{sdk_path}"

  # Find NDK
  ndk_path = Dir.glob("#{sdk_path}/ndk/*").select { |p| File.directory?(p) }.sort.last ||
             Dir.glob("#{sdk_path}/ndk-bundle").first
  if ndk_path
    ENV['NDK_HOME'] ||= ndk_path
    echo_success "Android NDK: #{ndk_path}"
  else
    echo_warn "NDK not found in #{sdk_path}/ndk/ — install via SDK Manager"
  end

  # Find Java
  java_home = ENV['JAVA_HOME'] ||
    Dir.glob('/usr/lib/jvm/java-*').select { |p| File.directory?(p) }.sort.last ||
    `which java 2>/dev/null`.then { |j| j.strip.empty? ? nil : File.dirname(File.dirname(j)) }
  if java_home && File.directory?(java_home.to_s)
    ENV['JAVA_HOME'] ||= java_home
    echo_success "Java: #{java_home}"
  else
    echo_warn "JAVA_HOME not set — Android build may fail. Install JDK 17+"
  end

  unless File.exist?('src/alternatives/android/main.tsx')
    echo_error "Android shell not found — src/alternatives/android/ was removed with the React source and has not been rebuilt in Svelte yet (see STATUS.md)"
    exit 1
  end

  echo_build "npm install (if needed)"
  run_cmd("npm install --prefer-offline 2>/dev/null || npm install")

  echo_build "Copying icon assets"
  run_cmd("mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png 2>/dev/null || true")

  echo_build "npm run build — Android/mobile shell"
  unless run_cmd("npm run build")
    echo_error "Android frontend build failed"
    exit 1
  end
  echo_success "Android frontend built"

  unless File.directory?("src-tauri/gen/android")
    echo_build "tauri android init"
    run_cmd("npx tauri android init")
  end

  echo_build "tauri android build → release .apk"
  if run_cmd("npx tauri android build --apk --release")
    apk = Dir.glob("src-tauri/gen/android/**/outputs/**/*.apk").first ||
          Dir.glob("src-tauri/gen/android/**/*.apk").first
    if apk && File.exist?(apk)
      FileUtils.cp(apk, "BlueEnvironment-mobile.apk")
      echo_success "APK ready: BlueEnvironment-mobile.apk (#{File.size('BlueEnvironment-mobile.apk') / 1024}KB)"
    else
      echo_success "Build finished — check src-tauri/gen/android for the APK"
    end
  else
    echo_error "Android build failed — ensure NDK and JDK 17+ are installed"
    exit 1
  end
end

# ============================================
# BEDM — Blue Environment Display Manager
# ============================================
def cmd_build_bedm
  echo_info "Building BEDM — Blue Environment Display Manager"

  echo_build "cargo build — bedm-daemon"
  daemon_ok = Dir.chdir("BEDM/bedm-daemon") { run_cmd("cargo build --release") }
  daemon_ok ? echo_success("bedm-daemon compiled") : echo_error("bedm-daemon build failed")

  echo_build "npm build — bedm-ui"
  ui_ok = Dir.chdir("BEDM/bedm-ui") { run_cmd("npm install") && run_cmd("npm run build") }
  ui_ok ? echo_success("bedm-ui compiled") : echo_error("bedm-ui build failed")

  echo_build "tauri build — bedm-greeter"
  greeter_ok = Dir.chdir("BEDM/bedm-greeter") { run_cmd("npm install") && run_cmd("npx tauri build") }
  greeter_ok ? echo_success("bedm-greeter compiled") : echo_error("bedm-greeter build failed")

  if daemon_ok && ui_ok && greeter_ok
    puts "#{COLOR_GREEN}✓ BEDM build complete.#{COLOR_RESET}"
  else
    echo_warn "BEDM build finished with errors — see above"
    exit 1
  end
end

# ============================================
# COMPOSITOR — sam kompozytor
# ============================================
def cmd_build_shell(pkg_manager = nil)
  variant_label = pkg_manager ? " [pkg: #{pkg_manager}]" : ""
  echo_info "Building Blue Environment v#{VERSION} — Shell only#{variant_label}"
  echo_info "Runs: npm install && npm run tauri build"

  # Pass package manager to Vite/Tauri via env var so Blue Software
  # uses the right backend at runtime.
  if pkg_manager
    ENV['BLUE_PKG_MANAGER'] = pkg_manager
    echo_build "Package manager backend: #{pkg_manager}"
  end

  echo_build "npm install"
  unless run_cmd("npm install")
    echo_error "npm install failed"
    exit 1
  end
  echo_success "npm install done"

  echo_build "Copying icon assets"
  run_cmd("mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png 2>/dev/null || true")

  echo_build "npm run tauri build"
  unless run_cmd("npm run tauri build")
    echo_error "tauri build failed"
    exit 1
  end
  echo_success "Shell build complete"
  echo_info "Binary: target/release/blue-environment"
end

BLUE_APPS = {
  # app_id => { label, description, feature_flag (optional cargo feature) }
  "notepad"        => { label: "Notepad",          desc: "Plain-text and Markdown editor" },
  "blue_docs"      => { label: "Blue Docs",         desc: "Rich document editor (DOCX/PDF/Markdown)" },
  "blue_code"      => { label: "Blue Code",         desc: "Monaco-based code editor with LSP hints" },
  "terminal"       => { label: "Terminal",           desc: "PTY-backed terminal emulator" },
  "mail"           => { label: "Blue Mail",          desc: "IMAP/SMTP email client" },
  "blue_ai"        => { label: "Blue AI",            desc: "AI chat assistant (multi-provider)" },
  "blue_web"       => { label: "Blue Browser",       desc: "WebView-based web browser" },
  "blue_images"    => { label: "Blue Images",        desc: "Image viewer and editor" },
  "blue_video"     => { label: "Blue Video",         desc: "Video player (MPV backend)" },
  "blue_screenshot"=> { label: "Blue Screenshot",    desc: "Screenshot tool with region select" },
  "blue_archive"   => { label: "Blue Archive",       desc: "Archive manager (zip/tar/7z)" },
  "exploler"       => { label: "Blue Files",         desc: "File manager / Explorer" },
  "calculator"     => { label: "Calculator",         desc: "Scientific calculator" },
  "system_monitor" => { label: "System Monitor",     desc: "CPU/RAM/GPU/network metrics" },
  "blue_software"  => { label: "Blue Software",      desc: "Package manager frontend" },
  "camera"         => { label: "Camera",             desc: "Webcam capture application" },
}.freeze

def cmd_build_app(app_name)
  if app_name.nil? || app_name.empty?
    echo_info "Blue Environment v#{VERSION} — Standalone App Builder"
    echo_info "Usage: ruby build.rb app <app_name>"
    echo_info ""
    echo_info "Available apps:"
    BLUE_APPS.each do |id, info|
      puts "  #{COLOR_BOLD}#{id.ljust(18)}#{COLOR_RESET}  #{info[:label].ljust(20)}  #{COLOR_YELLOW}#{info[:desc]}#{COLOR_RESET}"
    end
    echo_info ""
    echo_info "Example: ruby build.rb app blue_docs"
    return
  end

  unless BLUE_APPS.key?(app_name)
    echo_error "Unknown app: '#{app_name}'"
    echo_info  "Run 'ruby build.rb app' to see available apps."
    exit 1
  end

  app_info = BLUE_APPS[app_name]
  echo_info "Building Blue Environment v#{VERSION} — #{app_info[:label]} (standalone)"
  echo_info "#{app_info[:desc]}"

  # Set env so the Vite build only bundles this app
  ENV['VITE_BLUE_STANDALONE_APP'] = app_name
  ENV['VITE_BLUE_UI_MODE']        = 'app'

  echo_build "npm install (if needed)"
  unless run_cmd("npm install --prefer-offline 2>/dev/null || npm install")
    echo_error "npm install failed"
    exit 1
  end

  echo_build "Copying icon assets"
  run_cmd("mkdir -p src-tauri/icons && cp images/icon.png src-tauri/icons/icon.png 2>/dev/null || true")

  echo_build "npm run build — standalone #{app_info[:label]}"
  unless run_cmd("npm run build")
    echo_error "Frontend build failed"
    exit 1
  end
  echo_success "Frontend built"

  echo_build "cargo build --release — Tauri wrapper for #{app_info[:label]}"
  Dir.chdir('src-tauri') do
    unless run_cmd("cargo build --release")
      echo_error "Tauri build failed"
      exit 1
    end
  end

  bin = "target/release/blue-environment"
  out = "target/release/blue-#{app_name.gsub('_', '-')}"
  FileUtils.cp(bin, out) if File.exist?(bin)

  echo_success "#{app_info[:label]} built"
  echo_info   "Binary: #{out}"
  echo_info   "Launch: #{out}  (opens only #{app_info[:label]}, no desktop shell)"
end

def cmd_build_compositor
  echo_info "Building Blue Compositor (standalone)"
  if build_compositor_binary
    puts "#{COLOR_GREEN}✓ Compositor build complete → #{COMPOSITOR_BIN_PATH}#{COLOR_RESET}"
  else
    exit 1
  end
end

# ============================================
# ALL — kompozytor + powłoka (bez pakowania .rpm)
# ============================================
def cmd_build_all
  echo_info "Building Blue Environment v#{VERSION} — compositor + desktop shell"
  exit 1 unless build_compositor_binary
  exit 1 unless build_frontend
  exit 1 unless build_shell_binary
  build_launcher_cli
  puts "#{COLOR_GREEN}✓ Compositor + shell build complete.#{COLOR_RESET}"
end

# ============================================
# INSTALL — instalacja systemowa (build desktop)
# ============================================
def cmd_install
  check_root

  echo_info "Installing Blue Environment v#{VERSION}..."

  FileUtils.mkdir_p([BLUE_LIBS, BLUE_APPS, BLUE_WALLS])
  FileUtils.mkdir_p(["/usr/share/wayland-sessions", "/usr/share/applications"])

  if File.exist?(SHELL_BIN_PATH)
    FileUtils.cp(SHELL_BIN_PATH, "#{BLUE_SHARE}/blue-environment")
    FileUtils.chmod(0755, "#{BLUE_SHARE}/blue-environment")
    echo_success "blue-environment"
  else
    echo_error "binary missing — run 'ruby build.rb desktop' (or 'all') first"
  end

  if File.exist?(COMPOSITOR_BIN_PATH)
    FileUtils.cp(COMPOSITOR_BIN_PATH, "#{BLUE_LIBS}/blue-compositor")
    FileUtils.chmod(0755, "#{BLUE_LIBS}/blue-compositor")
    echo_success "blue-compositor"
  else
    echo_error "compositor missing — run 'ruby build.rb compositor' (or 'all') first"
  end

  if File.exist?("launcher/blue")
    # Install the wrapper to PATH
    FileUtils.cp("launcher/blue", "/usr/local/bin/blue")
    FileUtils.chmod(0755, "/usr/local/bin/blue")
    # Install the full launcher source tree
    launcher_dest = "#{BLUE_LIBS}/blue"
    FileUtils.mkdir_p(launcher_dest)
    FileUtils.cp("launcher/main.rb", "#{launcher_dest}/main.rb")
    FileUtils.cp_r("launcher/src", "#{launcher_dest}/src")
    echo_success "blue CLI"
  end

  if File.directory?("wallpapers")
    FileUtils.cp_r(Dir.glob("wallpapers/*"), "#{BLUE_WALLS}/")
    echo_success "wallpapers"
  end

  wayland_desktop = <<~TEXT
    [Desktop Entry]
    Name=Blue Environment
    Exec=#{BLUE_LIBS}/blue-compositor
    Type=Application
    DesktopNames=Blue
    Version=#{VERSION}
  TEXT
  File.write("/usr/share/wayland-sessions/blue-environment.desktop", wayland_desktop)

  app_desktop = <<~TEXT
    [Desktop Entry]
    Name=Blue Environment
    Exec=#{BLUE_SHARE}/blue-environment
    Icon=#{BLUE_SHARE}/icon.png
    Type=Application
    Categories=System;
  TEXT
  File.write("/usr/share/applications/blue-environment.desktop", app_desktop)

  File.write("#{BLUE_SHARE}/.version", VERSION)

  puts "#{COLOR_GREEN}✓ Installation complete! Run: blue start#{COLOR_RESET}"
end

# ============================================
# UNINSTALL — odinstalacja
# ============================================
def cmd_uninstall
  check_root

  echo_info "Uninstalling Blue Environment..."

  FileUtils.rm_rf(BLUE_SHARE)
  FileUtils.rm_f("/usr/share/wayland-sessions/blue-environment.desktop")
  FileUtils.rm_f("/usr/share/applications/blue-environment.desktop")
  FileUtils.rm_f("/usr/local/bin/blue")

  puts "#{COLOR_YELLOW}✓ Uninstalled. User configs in ~/.config/Blue-Environment kept.#{COLOR_RESET}"
end

# ============================================
# CLEAN — czyszczenie artefaktów
# ============================================
def cmd_clean
  echo_info "Cleaning build artifacts..."

  FileUtils.rm_rf([
    "src-tauri/target", "compositor/target", "dist", "blue", "rpmbuild", "staging",
    "BEDM/bedm-daemon/target", "BEDM/bedm-ui/dist", "BEDM/bedm-greeter/target",
    "BEDM/bedm-greeter/dist", "src-tauri/gen",
  ])

  puts "#{COLOR_GREEN}✓ Clean complete.#{COLOR_RESET}"
end

# ============================================
# MAIN — parsowanie argumentów
# ============================================
action = ARGV[0]

case action
when nil, ""
  cmd_help
when "desktop"
  cmd_build_desktop
when "android"
  cmd_build_android
when "tv"
  cmd_build_tv
when "BEDM", "bedm"
  cmd_build_bedm
when "compositor"
  cmd_build_compositor
when "packages", "pkg"
  # ruby build.rb packages [rpm|deb|arch|apk|opensuse]   (binaries must
  # already be built — e.g. after `ruby build.rb shell` — this only
  # (re)packages them)
  cmd_build_packages(ARGV[1])
when "app"
  # ruby build.rb app <app_name>
  # ruby build.rb app             (lists all available apps)
  app_name = ARGV[1]
  cmd_build_app(app_name)
when "shell"
  # ruby build.rb shell [debian|arch|fedora|opensuse|rpm-ostree|dnf]
  pkg_manager = ARGV[1]
  cmd_build_shell(pkg_manager)
when "all"
  cmd_build_all
when "install"
  cmd_install
when "uninstall"
  cmd_uninstall
when "clean"
  cmd_clean
when "check"
  exit(1) unless cmd_check_frontend
when "doctor"
  cmd_doctor
when "help", "-h", "--help"
  cmd_help
else
  puts "#{COLOR_RED}✗ Unknown command: #{action}#{COLOR_RESET}"
  puts "#{COLOR_YELLOW}Run: ruby #{$PROGRAM_NAME}#{COLOR_RESET}"
  exit 1
end

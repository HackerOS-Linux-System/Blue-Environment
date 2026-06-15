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
VERSION    = "0.6.0"

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

def echo_build(msg)
  puts "#{COLOR_BOLD}  → #{msg}#{COLOR_RESET}"
end

def print_hr(length = 40)
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
  puts "  build                Normal build"
  puts "  build-hackeros       HackerOS variant"
  puts "  build-legendaryos    LegendaryOS .rpm"
  puts "  build-fedora         Fedora .rpm"
  puts "  install              Install to system (sudo)"
  puts "  clean                Clean artifacts"
  puts "  uninstall            Remove from system (sudo)"
  print_hr
end

# ============================================
# BUILD — kompilacja główna
# ============================================
def cmd_build
  echo_info "Building Blue Environment v#{VERSION}..."

  # npm build
  echo_build "npm build"
  if run_cmd("npm run build")
    echo_success "npm build complete"
  else
    echo_error "npm build failed"
    exit 1
  end

  # Cargo build — blue-environment
  echo_build "cargo build — blue-environment"
  if run_cmd("cargo build --release --manifest-path src-tauri/Cargo.toml")
    echo_success "blue-environment compiled"
  else
    echo_error "cargo build failed"
    exit 1
  end

  # Cargo build — blue-compositor
  echo_build "cargo build — blue-compositor"
  if run_cmd("cargo build --release --manifest-path src-tauri/lib/blue-compositor/Cargo.toml")
    echo_success "blue-compositor compiled"
  else
    echo_error "compositor build failed"
    exit 1
  end

  # Crystal launcher
  echo_build "crystal launcher"
  if command_exists?("crystal")
    if run_cmd("crystal build launcher/src/main.cr -o blue --release 2>/dev/null")
      echo_success "blue launcher"
    else
      echo_error "crystal build failed"
    end
  else
    echo_error "crystal not found — skipping launcher"
  end

  puts "#{COLOR_GREEN}✓ Build complete.#{COLOR_RESET}"
end

# ============================================
# BUILD-HACKEROS — wariant HackerOS
# ============================================
def cmd_build_hackeros
  echo_info "Building HackerOS variant..."

  # Patch
  if run_cmd("python3 scripts/patch-hackeros.py")
    echo_success "Patched for HackerOS"
  else
    echo_error "Patching failed"
    exit 1
  end

  # Build
  cmd_build

  # Unpatch
  run_cmd("python3 scripts/unpatch.py")
  echo_success "Restored original files"
end

# ============================================
# BUILD-LEGENDARYOS — RPM dla LegendaryOS
# ============================================
def cmd_build_legendaryos
  echo_info "Building LegendaryOS RPM..."

  # Check rpmbuild
  unless command_exists?("rpmbuild")
    echo_error "rpmbuild not found"
    exit 1
  end

  # Patch
  if run_cmd("python3 scripts/patch-legendaryos.py")
    echo_success "Patching failed" # Zachowane z oryginału (prawdopodobnie mały bug w logice komunikatów oryginału, ale przepisany 1:1)
  else
    echo_error "Patching failed"
    exit 1
  end

  # Build
  cmd_build

  # Unpatch
  run_cmd("python3 scripts/unpatch.py")

  # Prepare RPM structure
  FileUtils.mkdir_p(["rpmbuild/BUILD", "rpmbuild/RPMS", "rpmbuild/SOURCES", "rpmbuild/SPECS", "rpmbuild/SRPMS"])

  # Generate spec file
  pwd = Dir.pwd
  run_cmd("sed 's/@VERSION@/#{VERSION}/g' packaging/blue-environment-legendaryos.spec > rpmbuild/SPECS/blue-environment.spec")
  run_cmd("sed -i 's/@VERSION/#{VERSION}/g' rpmbuild/SPECS/blue-environment.spec")
  
  if run_cmd("rpmbuild -bb rpmbuild/SPECS/blue-environment.spec --define \"_topdir #{pwd}/rpmbuild\"")
    echo_success "RPM built in rpmbuild/RPMS/"
  else
    echo_error "RPM build failed"
  end
end

# ============================================
# BUILD-FEDORA — RPM dla Fedory
# ============================================
def cmd_build_fedora
  echo_info "Building Fedora RPM..."

  unless command_exists?("rpmbuild")
    echo_error "rpmbuild not found"
    exit 1
  end

  cmd_build

  FileUtils.mkdir_p(["rpmbuild/BUILD", "rpmbuild/RPMS", "rpmbuild/SOURCES", "rpmbuild/SPECS", "rpmbuild/SRPMS"])
  
  pwd = Dir.pwd
  run_cmd("sed 's/@VERSION@/#{VERSION}/g' packaging/blue-environment-fedora.spec > rpmbuild/SPECS/blue-environment.spec")
  run_cmd("sed -i 's/@VERSION/#{VERSION}/g' rpmbuild/SPECS/blue-environment.spec")
  
  if run_cmd("rpmbuild -bb rpmbuild/SPECS/blue-environment.spec --define \"_topdir #{pwd}/rpmbuild\"")
    echo_success "RPM built"
  else
    echo_error "RPM build failed"
  end
end

# ============================================
# INSTALL — instalacja systemowa
# ============================================
def cmd_install
  check_root

  echo_info "Installing Blue Environment v#{VERSION}..."

  # Create directories
  FileUtils.mkdir_p([BLUE_LIBS, BLUE_APPS, BLUE_WALLS])
  FileUtils.mkdir_p(["/usr/share/wayland-sessions", "/usr/share/applications"])

  # Install blue-environment binary
  if File.exist?("src-tauri/target/release/blue-environment")
    FileUtils.cp("src-tauri/target/release/blue-environment", "#{BLUE_SHARE}/blue-environment")
    FileUtils.chmod(0755, "#{BLUE_SHARE}/blue-environment")
    echo_success "blue-environment"
  else
    echo_error "binary missing"
  end

  # Install blue-compositor
  if File.exist?("src-tauri/lib/blue-compositor/target/release/blue-compositor")
    FileUtils.cp("src-tauri/lib/blue-compositor/target/release/blue-compositor", "#{BLUE_LIBS}/blue-compositor")
    FileUtils.chmod(0755, "#{BLUE_LIBS}/blue-compositor")
    echo_success "blue-compositor"
  else
    echo_error "compositor missing"
  end

  # Install CLI
  if File.exist?("blue")
    FileUtils.cp("blue", "/usr/local/bin/blue")
    FileUtils.chmod(0755, "/usr/local/bin/blue")
    echo_success "blue CLI"
  end

  # Install wallpapers
  if File.directory?("wallpapers")
    # Kopiowanie zawartości katalogu (odpowiednik wallpapers/.)
    FileUtils.cp_r(Dir.glob("wallpapers/*"), "#{BLUE_WALLS}/")
    echo_success "wallpapers"
  end

  # Create wayland session desktop file
  wayland_desktop = <<~TEXT
    [Desktop Entry]
    Name=Blue Environment
    Exec=#{BLUE_LIBS}/blue-compositor
    Type=Application
    DesktopNames=Blue
    Version=#{VERSION}
  TEXT
  File.write("/usr/share/wayland-sessions/blue-environment.desktop", wayland_desktop)

  # Create app desktop file
  app_desktop = <<~TEXT
    [Desktop Entry]
    Name=Blue Environment
    Exec=#{BLUE_SHARE}/blue-environment
    Icon=#{BLUE_SHARE}/icon.png
    Type=Application
    Categories=System;
  TEXT
  File.write("/usr/share/applications/blue-environment.desktop", app_desktop)

  # Write version file
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
# CLEAN — czyszczenie artifactów
# ============================================
def cmd_clean
  echo_info "Cleaning build artifacts..."

  FileUtils.rm_rf(["src-tauri/target", "src-tauri/lib/blue-compositor/target", "dist", "blue", "rpmbuild"])

  puts "#{COLOR_GREEN}✓ Clean complete.#{COLOR_RESET}"
end

# ============================================
# MAIN — parsowanie argumentów
# ============================================
action = ARGV[0]

case action
when "help"
  cmd_help
when "build"
  cmd_build
when "build-hackeros"
  cmd_build_hackeros
when "build-legendaryos"
  cmd_build_legendaryos
when "build-fedora"
  cmd_build_fedora
when "install"
  cmd_install
when "uninstall"
  cmd_uninstall
when "clean"
  cmd_clean
else
  if action && action != ""
    puts "#{COLOR_RED}✗ Unknown command: #{action}#{COLOR_RESET}"
    puts "#{COLOR_YELLOW}Run: ruby #{$PROGRAM_NAME} help#{COLOR_RESET}"
    exit 1
  else
    cmd_help
  end
end

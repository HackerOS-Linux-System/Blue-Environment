#!/usr/bin/env ruby
# frozen_string_literal: true

# BEDM — Blue Environment Display Manager
# One-command installer script translated to Ruby
# Usage: sudo ruby install.rb [--autologin username]

require 'fileutils'
require 'open3'

##############################################################################
# Config
##############################################################################

BEDM_VERSION = "1.0.0"
BEDM_BINDIR = "/usr/bin"
BEDM_SYSCONFDIR = "/etc/bedm"
BEDM_SYSTEMD = "/usr/lib/systemd/system"
BEDM_PAMDIR = "/etc/pam.d"
BEDM_VARDIR = "/var/lib/bedm"
BEDM_LOGDIR = "/var/log/bedm"
BEDM_RUNDIR = "/run/bedm"

# Colors
R = "\033[0;31m"  # Red
G = "\033[0;32m"  # Green
Y = "\033[1;33m"  # Yellow
C = "\033[0;36m"  # Cyan
B = "\033[1;34m"  # Blue
N = "\033[0m"     # Reset

##############################################################################
# Helpers
##############################################################################

def info(msg)    puts "#{C}  →#{N} #{msg}" end
def ok(msg)      puts "#{G}  ✓#{N} #{msg}" end
def warn(msg)    puts "#{Y}  !#{N} #{msg}" end
def err(msg)     warn_stderr "#{R}  ✗#{N} #{msg}" end
def warn_stderr(msg) $stderr.puts msg end

def die(msg)
  err(msg)
  exit 1
end

def section(title)
  puts "\n#{B}══ #{title} ══#{N}"
end

def require_root
  die "This script must be run as root" unless Process.euid == 0
end

def command_exists?(cmd)
  system("command -v #{cmd} >/dev/null 2>&1")
end

def detect_package_manager
  if command_exists?('apt-get') then 'apt'
  elsif command_exists?('dnf') then 'dnf'
  elsif command_exists?('pacman') then 'pacman'
  elsif command_exists?('zypper') then 'zypper'
  else 'unknown'
  end
end

##############################################################################
# Main
##############################################################################

require_root

autologin_user = ""
args = ARGV.dup

while args.any?
  arg = args.shift
  case arg
  when '--autologin'
    autologin_user = args.shift || ""
  when '--help', '-h'
    puts "Usage: sudo ruby install.rb [--autologin USERNAME]"
    exit 0
  else
    warn "Unknown argument: #{arg}"
  end
end

puts ""
puts "#{B}  ██████╗ ███████╗██████╗ ███╗   ███╗#{N}"
puts "#{B}  ██╔══██╗██╔════╝██╔══██╗████╗ ████║#{N}"
puts "#{B}  ██████╔╝█████╗  ██║  ██║██╔████╔██║#{N}"
puts "#{B}  ██╔══██╗██╔══╝  ██║  ██║██║╚██╔╝██║#{N}"
puts "#{B}  ██████╔╝███████╗██████╔╝██║ ╚═╝ ██║#{N}"
puts "#{B}  ╚═════╝ ╚══════╝╚═════╝ ╚═╝     ╚═╝#{N}"
puts "#{C}  Blue Environment Display Manager v#{BEDM_VERSION}${N}"
puts ""

##############################################################################
section "Pre-flight checks"
##############################################################################

# Check we're not replacing a running session wrongly
if system('systemctl is-active --quiet bedm >/dev/null 2>&1')
  warn "BEDM is already running — will reinstall"
end

# Detect existing display managers
existing_dms = []
%w[gdm gdm3 sddm lightdm lxdm xdm slim].each do |dm|
  if system("systemctl is-enabled --quiet #{dm} >/dev/null 2>&1")
    existing_dms << dm
  end
end

if existing_dms.any?
  warn "Found existing display manager(s): #{existing_dms.join(' ')}"
  warn "These will be disabled in favor of BEDM"
  print "  Continue? [y/N] "
  ans = $stdin.gets&.strip&.downcase
  unless ans == 'y'
    info "Aborted"
    exit 0
  end
end

# Build check — either pre-built binaries or source
script_dir = File.expand_path(__dir__)
daemon_bin = File.join(script_dir, 'bedm-daemon/target/release/bedm')
greeter_bin = File.join(script_dir, 'bedm-greeter/target/release/bedm-greeter')

if !File.exist?(daemon_bin)
  info "Binary not found — attempting source build"

  # Check build deps
  die "Rust/cargo not found — install from https://rustup.rs" unless command_exists?('cargo')
  die "Node.js/npm not found — install Node.js first" unless command_exists?('npm')

  info "Building BEDM (this may take a few minutes)..."
  unless system("make -C '#{script_dir}' build")
    die "Build failed — check error output above"
  end
end

die "Daemon binary missing after build" unless File.exist?(daemon_bin)
die "Greeter binary missing after build" unless File.exist?(greeter_bin)
ok "Binaries ready"

##############################################################################
section "Creating directories"
##############################################################################

dirs = [
  BEDM_SYSCONFDIR, BEDM_VARDIR, BEDM_LOGDIR, BEDM_RUNDIR,
  File.join(BEDM_RUNDIR, 'sessions'), File.join(BEDM_VARDIR, 'users')
]

dirs.each do |dir|
  FileUtils.mkdir_p(dir)
  FileUtils.chmod(0755, dir)
  ok "Created #{dir}"
end

##############################################################################
section "Installing binaries"
##############################################################################

FileUtils.mkdir_p(BEDM_BINDIR)
FileUtils.cp(daemon_bin, File.join(BEDM_BINDIR, 'bedm'))
FileUtils.chmod(0755, File.join(BEDM_BINDIR, 'bedm'))

FileUtils.cp(greeter_bin, File.join(BEDM_BINDIR, 'bedm-greeter'))
FileUtils.chmod(0755, File.join(BEDM_BINDIR, 'bedm-greeter'))

ok "Installed /usr/bin/bedm"
ok "Installed /usr/bin/bedm-greeter"

##############################################################################
section "Installing PAM configuration"
##############################################################################

FileUtils.mkdir_p(BEDM_PAMDIR)
FileUtils.cp(File.join(script_dir, 'config/pam-bedm'), File.join(BEDM_PAMDIR, 'bedm'))
FileUtils.chmod(0644, File.join(BEDM_PAMDIR, 'bedm'))
ok "PAM service: /etc/pam.d/bedm"

##############################################################################
section "Installing systemd service"
##############################################################################

FileUtils.mkdir_p(BEDM_SYSTEMD)
FileUtils.cp(File.join(script_dir, 'systemd/bedm.service'), File.join(BEDM_SYSTEMD, 'bedm.service'))
FileUtils.chmod(0644, File.join(BEDM_SYSTEMD, 'bedm.service'))

system('systemctl daemon-reload')
ok "systemd service installed"

##############################################################################
section "Installing configuration"
##############################################################################

toml_path = File.join(BEDM_SYSCONFDIR, 'bedm.toml')

if !File.exist?(toml_path)
  FileUtils.cp(File.join(script_dir, 'config/bedm.toml'), toml_path)
  FileUtils.chmod(0644, toml_path)
  ok "Default config: /etc/bedm/bedm.toml"
else
  warn "Config exists — preserved: /etc/bedm/bedm.toml"
end

# Handle autologin
if !autologin_user.empty?
  user_exists = system("id '#{autologin_user}' >/dev/null 2>&1")
  
  if user_exists
    # Patch config
    config_content = File.read(toml_path)
    
    if config_content.match?(/^#\s*autologin_user.*/)
      config_content.gsub!(/^#\s*autologin_user.*/, "autologin_user = \"#{autologin_user}\"")
    elsif !config_content.match?(/^autologin_user/)
      config_content += "\nautologin_user = \"#{autologin_user}\"\n"
    end
    
    File.write(toml_path, config_content)
    ok "Autologin configured for: #{autologin_user}"
  else
    warn "User '#{autologin_user}' not found — autologin not configured"
  end
end

##############################################################################
section "Disabling conflicting display managers"
##############################################################################

existing_dms.each do |dm|
  if system("systemctl disable --now #{dm} >/dev/null 2>&1")
    ok "Disabled: #{dm}"
  else
    warn "Could not disable: #{dm}"
  end
end

##############################################################################
section "Enabling BEDM"
##############################################################################

# Link as display-manager.service
begin
  target_link = File.join(BEDM_SYSTEMD, 'display-manager.service')
  File.delete(target_link) if File.exist?(target_link) || File.symlink?(target_link)
  File.symlink(File.join(BEDM_SYSTEMD, 'bedm.service'), target_link)
rescue StandardError
  # Equivalent to || true in bash
end

system('systemctl enable bedm')
ok "BEDM enabled as display manager"

##############################################################################
# Summary
##############################################################################

puts ""
puts "#{G}══════════════════════════════════════════════════#{N}"
puts "#{G}  BEDM v#{BEDM_VERSION} installed successfully!#{N}"
puts "#{G}══════════════════════════════════════════════════#{N}"
puts ""
puts "  Daemon:    #{C}#{BEDM_BINDIR}/bedm#{N}"
puts "  Greeter:   #{C}#{BEDM_BINDIR}/bedm-greeter#{N}"
puts "  Config:    #{C}#{BEDM_SYSCONFDIR}/bedm.toml#{N}"
puts "  Logs:      #{C}journalctl -u bedm#{N}"
puts ""
puts "#{Y}  Start now: #{N}systemctl start bedm"
puts "#{Y}  Or reboot: #{N}reboot"
puts ""

# Offer to start immediately
print "  Start BEDM now? [Y/n] "
ans = $stdin.gets&.strip&.downcase
if ans != 'n'
  info "Starting BEDM..."
  # Run in background using Process.spawn equivalent to '&' in Bash
  Process.spawn('systemctl start bedm')
  ok "BEDM started — switching to display manager"
end

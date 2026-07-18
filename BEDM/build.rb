require 'fileutils'

# ── Konfiguracja Zmiennych Środowiskowych ─────────────────────────────────────
PREFIX      = ENV['PREFIX'] || '/usr'
SYSCONFDIR  = ENV['SYSCONFDIR'] || '/etc'
VARDIR      = ENV['VARDIR'] || '/var'
RUNDIR      = ENV['RUNDIR'] || '/run'
SYSTEMDDIR  = ENV['SYSTEMDDIR'] || '/usr/lib/systemd/system'
DESTDIR     = ENV['DESTDIR'] || ''
VERSION     = ENV['VERSION'] || '0.6.0'

BINDIR      = File.join(PREFIX, 'bin')
SHAREDIR    = File.join(PREFIX, 'share/bedm')
PAMDIR      = File.join(SYSCONFDIR, 'pam.d')

CARGO       = 'cargo'
NPM         = 'npm'
TAURI       = 'npx tauri'

# ── Kolory ANSI ───────────────────────────────────────────────────────────────
GREEN  = "\033[0;32m"
YELLOW = "\033[1;33m"
CYAN   = "\033[0;36m"
RESET  = "\033[0m"

# Helper do formatowania logów
def log(color, message)
  puts "#{color}#{message}#{RESET}"
end

# Helper odzwierciedlający zachowanie komendy `install -D`
def install_file(src, dest, mode)
  FileUtils.mkdir_p(File.dirname(dest))
  FileUtils.cp(src, dest)
  FileUtils.chmod(mode, dest)
end

# Zadanie domyślne (default) odpowiadające "all"
task default: :build

## ── Build ──────────────────────────────────────────────────────────────────

desc 'Build daemon + greeter (release)'
task :build => [: 'build-daemon', 'build-greeter'] do
  log(GREEN, '✓ BEDM build complete')
end

desc 'Build only the daemon'
task 'build-daemon' do
  log(CYAN, '→ Building bedm daemon...')
  Dir.chdir('bedm-daemon') do
    sh "#{CARGO} build --release"
  end
  log(GREEN, '✓ bedm daemon built')
end

desc 'Build only the greeter'
task 'build-greeter' do
  log(CYAN, '→ Building bedm-ui...')
  Dir.chdir('bedm-ui') do
    sh "#{NPM} install"
    sh "#{NPM} run build"
  end
  log(CYAN, '→ Building bedm-greeter (Tauri)...')
  Dir.chdir('bedm-greeter') do
    sh "#{TAURI} build"
  end
  log(GREEN, '✓ bedm-greeter built')
end

## ── Development ────────────────────────────────────────────────────────────

desc 'Start greeter in Tauri dev mode'
task 'dev-greeter' do
  log(CYAN, '→ Starting greeter in dev mode...')
  Dir.chdir('bedm-greeter') do
    sh "#{TAURI} dev"
  end
end

desc 'Start React UI dev server'
task 'dev-ui' do
  log(CYAN, '→ Starting UI dev server...')
  Dir.chdir('bedm-ui') do
    sh "#{NPM} run dev"
  end
end

## ── Install ────────────────────────────────────────────────────────────────

desc 'Full install (run as root)'
task :install => [:build, 'install-systemd', 'install-pam', 'install-config'] do
  log(CYAN, '→ Installing binaries...')
  install_file('bedm-daemon/target/release/bedm', File.join(DESTDIR, BINDIR, 'bedm'), 0755)
  install_file('bedm-greeter/target/release/bedm-greeter', File.join(DESTDIR, BINDIR, 'bedm-greeter'), 0755)

  log(CYAN, '→ Creating runtime dirs...')
  FileUtils.mkdir_p(File.join(DESTDIR, RUNDIR, 'bedm'), mode: 0755)
  FileUtils.mkdir_p(File.join(DESTDIR, RUNDIR, 'bedm/sessions'), mode: 0755)
  FileUtils.mkdir_p(File.join(DESTDIR, VARDIR, 'lib/bedm'), mode: 0755)
  FileUtils.mkdir_p(File.join(DESTDIR, VARDIR, 'log/bedm'), mode: 0755)

  log(GREEN, "✓ BEDM installed to #{DESTDIR}#{BINDIR}")
  puts ''
  log(YELLOW, 'Next steps:')
  puts '  1. Disable your current DM:  systemctl disable gdm sddm lightdm 2>/dev/null || true'
  puts '  2. Enable BEDM:              systemctl enable bedm'
  puts '  3. Start BEDM:               systemctl start bedm'
  puts '  OR reboot to apply.'
end

task 'install-systemd' do
  log(CYAN, '→ Installing systemd service...')
  install_file('systemd/bedm.service', File.join(DESTDIR, SYSTEMDDIR, 'bedm.service'), 0644)
  if DESTDIR.empty?
    sh 'systemctl daemon-reload' rescue nil
  end
end

task 'install-pam' do
  log(CYAN, '→ Installing PAM config...')
  install_file('config/pam-bedm', File.join(DESTDIR, PAMDIR, 'bedm'), 0644)
end

task 'install-config' do
  log(CYAN, '→ Installing default config...')
  config_path = File.join(DESTDIR, SYSCONFDIR, 'bedm/bedm.toml')
  if !File.exist?(config_path)
    install_file('config/bedm.toml', config_path, 0644)
    log(GREEN, "✓ Config installed to #{config_path}")
  else
    log(YELLOW, "! Existing config preserved (#{config_path})")
  end
end

## ── Uninstall ──────────────────────────────────────────────────────────────

desc 'Remove BEDM'
task :uninstall do
  log(CYAN, '→ Uninstalling BEDM...')
  
  sh 'systemctl stop bedm 2>/dev/null || true' rescue nil
  sh 'systemctl disable bedm 2>/dev/null || true' rescue nil
  
  FileUtils.rm_f(File.join(BINDIR, 'bedm'))
  FileUtils.rm_f(File.join(BINDIR, 'bedm-greeter'))
  FileUtils.rm_f(File.join(SYSTEMDDIR, 'bedm.service'))
  FileUtils.rm_f(File.join(PAMDIR, 'bedm'))
  
  sh 'systemctl daemon-reload' rescue nil
  
  log(GREEN, '✓ BEDM uninstalled')
  log(YELLOW, 'Note: config at /etc/bedm/ and logs at /var/log/bedm/ kept')
end

## ── Quality ────────────────────────────────────────────────────────────────

desc 'Run cargo check for daemon and greeter'
task :check do
  Dir.chdir('bedm-daemon') { sh "#{CARGO} check" }
  Dir.chdir('bedm-greeter') { sh "#{CARGO} check" }
end

desc 'Lint the codebase'
task :lint do
  Dir.chdir('bedm-daemon') { sh "#{CARGO} clippy -- -D warnings" }
  Dir.chdir('bedm-greeter') { sh "#{CARGO} clippy -- -D warnings" }
  Dir.chdir('bedm-ui') { sh "#{NPM} run lint 2>/dev/null || true" rescue nil }
end

desc 'Clean build artifacts'
task :clean do
  Dir.chdir('bedm-daemon') { sh "#{CARGO} clean" }
  Dir.chdir('bedm-greeter') { sh "#{CARGO} clean" }
  FileUtils.rm_rf(['bedm-ui/dist', 'bedm-ui/node_modules'])
end

## ── Package ────────────────────────────────────────────────────────────────

desc 'Create distribution tarball'
task :dist => :build do
  log(CYAN, '→ Creating distribution tarball...')
  tmp_dir = "dist-tmp/bedm-#{VERSION}"
  FileUtils.mkdir_p(tmp_dir)
  
  FileUtils.cp_r('bedm-daemon/target/release/bedm', tmp_dir)
  FileUtils.cp_r('bedm-greeter/target/release/bedm-greeter', tmp_dir)
  FileUtils.cp_r(['config/', 'systemd/', 'Makefile', 'README.md'], tmp_dir)
  
  sh "tar -czf bedm-#{VERSION}.tar.gz -C dist-tmp bedm-#{VERSION}"
  FileUtils.rm_rf('dist-tmp')
  
  log(GREEN, "✓ bedm-#{VERSION}.tar.gz")
end

## ── Package (installer formats) ───────────────────────────────────────────

desc 'Build a .rpm installer package (needs rpmbuild)'
task :rpm => :build do
  log(CYAN, '→ Building BEDM .rpm...')
  unless system('which rpmbuild > /dev/null 2>&1')
    log(YELLOW, '! rpmbuild not found — install the "rpm-build" package')
    exit 1
  end

  topdir = File.expand_path('rpmbuild')
  %w[BUILD RPMS SOURCES SPECS SRPMS].each { |d| FileUtils.mkdir_p("#{topdir}/#{d}") }

  FileUtils.cp('bedm-daemon/target/release/bedm', "#{topdir}/SOURCES/bedm")
  FileUtils.cp('bedm-greeter/target/release/bedm-greeter', "#{topdir}/SOURCES/bedm-greeter")

  spec_out = "#{topdir}/SPECS/bedm.spec"
  sh "sed 's/@VERSION@/#{VERSION}/g' ../packaging/BEDM/bedm.spec > #{spec_out}" rescue \
    FileUtils.cp('../packaging/BEDM/bedm.spec', spec_out)

  sh "rpmbuild -bb #{spec_out} --define \"_topdir #{topdir}\""
  log(GREEN, "✓ BEDM RPM → #{topdir}/RPMS/")
end

desc 'Build a .deb installer package (needs dpkg-deb)'
task :deb => :build do
  log(CYAN, '→ Building BEDM .deb...')
  unless system('which dpkg-deb > /dev/null 2>&1')
    log(YELLOW, '! dpkg-deb not found — install the "dpkg-dev" package')
    exit 1
  end

  root = 'build/deb/bedm'
  FileUtils.rm_rf(root)
  FileUtils.mkdir_p("#{root}/DEBIAN")
  FileUtils.mkdir_p("#{root}#{BINDIR}")
  FileUtils.mkdir_p("#{root}/usr/lib/systemd/system")
  FileUtils.mkdir_p("#{root}#{PAMDIR}")
  FileUtils.mkdir_p("#{root}#{SYSCONFDIR}/bedm")

  install_file('bedm-daemon/target/release/bedm', "#{root}#{BINDIR}/bedm", 0755)
  install_file('bedm-greeter/target/release/bedm-greeter', "#{root}#{BINDIR}/bedm-greeter", 0755)
  install_file('systemd/bedm.service', "#{root}/usr/lib/systemd/system/bedm.service", 0644)
  install_file('config/pam-bedm', "#{root}#{PAMDIR}/bedm", 0644)
  install_file('config/bedm.toml', "#{root}#{SYSCONFDIR}/bedm/bedm.toml", 0644)

  control_src = File.read('../packaging/BEDM/control')
  pkg_stanza = control_src.split(/\n\n/).last
  File.write("#{root}/DEBIAN/control", "#{pkg_stanza}\nVersion: #{VERSION}\n")

  FileUtils.mkdir_p('dist')
  out = "dist/bedm_#{VERSION}_amd64.deb"
  sh "dpkg-deb --root-owner-group --build #{root} #{out}"
  log(GREEN, "✓ BEDM DEB → #{out}")
end

desc 'Build every installer format available on this machine (.rpm + .deb)'
task :packages => :build do
  Rake::Task[:rpm].invoke if system('which rpmbuild > /dev/null 2>&1')
  Rake::Task[:deb].invoke if system('which dpkg-deb > /dev/null 2>&1')
end

## ── Help ───────────────────────────────────────────────────────────────────

# Rake automatycznie generuje pomoc dla zadań opisanych przez `desc` za pomocą komendy `rake -T`.
# Poniższe zadanie zachowuje oryginalny wygląd Twojego menu help z Makefile.
desc 'List available tasks'
task :help do
  log(CYAN, "\nBEDM — Blue Environment Display Manager\n")
  log(YELLOW, 'Build:')
  puts '  rake build          — Build daemon + greeter (release)'
  puts '  rake build-daemon   — Build only the daemon'
  puts '  rake build-greeter  — Build only the greeter'
  puts ''
  log(YELLOW, 'Development:')
  puts '  rake dev-greeter    — Start greeter in Tauri dev mode'
  puts '  rake dev-ui         — Start React UI dev server'
  puts ''
  log(YELLOW, 'Install (run as root):')
  puts '  rake install        — Full install'
  puts '  rake uninstall      — Remove BEDM'
  puts ''
  log(YELLOW, 'Packaging:')
  puts '  rake rpm            — Build a .rpm installer (needs rpmbuild)'
  puts '  rake deb            — Build a .deb installer (needs dpkg-deb)'
  puts '  rake packages       — Build every installer format available here'
  puts '  rake dist           — Build a plain .tar.gz distribution archive'
  puts ''
  log(YELLOW, 'Environment:')
  puts "  PREFIX=#{PREFIX}  SYSCONFDIR=#{SYSCONFDIR}"
  puts ''
end

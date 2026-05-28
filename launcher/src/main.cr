require "http/client"
require "json"
require "file_utils"

VERSION     = "0.7"
GITHUB_RAW  = "https://raw.githubusercontent.com/HackerOS-Linux-System/Blue-Environment/main"
GITHUB_REL  = "https://github.com/HackerOS-Linux-System/Blue-Environment/releases/download"
BLUE_SHARE  = "/usr/share/Blue-Environment"
BLUE_LIBS   = "#{BLUE_SHARE}/lib"
BLUE_APPS   = "#{BLUE_SHARE}/apps"
BLUE_WALLS  = "#{BLUE_SHARE}/wallpapers"
COMPOSITOR  = "#{BLUE_LIBS}/blue-compositor"
SHELL_BIN   = "#{BLUE_SHARE}/blue-environment"
VERSION_FILE = "#{BLUE_SHARE}/.version"

def user_config : String
  File.join(ENV.fetch("HOME", "/root"), ".config", "Blue-Environment")
end

def bold(s)
  "\e[1m#{s}\e[0m"
end
def green(s)
  "\e[32m#{s}\e[0m"
end
def yellow(s)
  "\e[33m#{s}\e[0m"
end
def red(s)
  "\e[31m#{s}\e[0m"
end
def cyan(s)
  "\e[36m#{s}\e[0m"
end
def dim(s)
  "\e[2m#{s}\e[0m"
end

def ok(m)
  puts "  #{green("✓")} #{m}"
end
def err(m)
  puts "  #{red("✗")} #{m}"
end
def info(m)
  puts "  #{cyan("→")} #{m}"
end
def warn(m)
  puts "  #{yellow("!")} #{m}"
end

def die(m) : NoReturn
  err(m)
  exit(1)
end

def http_get(url : String) : String?
  HTTP::Client.get(url) { |r| r.status.success? ? r.body_io.gets_to_end : nil }
rescue
  nil
end

def download_file(url : String, dest : String) : Bool
  info "Downloading #{File.basename(dest)}..."
  r = HTTP::Client.get(url)
  if r.status.success?
    File.write(dest, r.body)
    ok "Saved"
    true
  else
    err "HTTP #{r.status_code}: #{url}"
    false
  end
rescue ex
  err "Download failed: #{ex.message}"
  false
end

def local_version : String
  return "0.0" unless File.exists?(VERSION_FILE)
  File.read(VERSION_FILE).strip
rescue
  "0.0"
end

def remote_version : String?
  body = http_get("#{GITHUB_RAW}/launcher/version")
  return nil unless body
  body.strip
end

def newer?(rv : String, lv : String) : Bool
  r = rv.split(".").map(&.to_i)
  l = lv.split(".").map(&.to_i)
  max = [r.size, l.size].max
  r.concat(Array.new(max - r.size, 0))
  l.concat(Array.new(max - l.size, 0))
  r.zip(l).each { |a, b| return true if a > b; return false if a < b }
  false
end

def blue_share_writable? : Bool
  if File.exists?(BLUE_SHARE)
    File::Info.writable?(BLUE_SHARE)
  else
    parent = File.dirname(BLUE_SHARE)
    File.exists?(parent) && File::Info.writable?(parent)
  end
end

def fetch_app_list : Hash(String, String)?
  body = http_get("#{GITHUB_RAW}/apps/list.hacker")
  return nil unless body
  result = {} of String => String
  current = nil
  body.each_line do |line|
    line = line.strip
    next if line.empty? || line.starts_with?('#')
    if line.ends_with?(':')
      current = line.chomp(':')
    elsif line.starts_with?("= ") && current
      result[current] = line[2..]
      current = nil
    end
  end
  result
end

def cmd_info
  puts ""
  puts "  #{bold("Blue Environment")} — Launcher v#{VERSION}"
  puts "  #{dim("──────────────────────────────────────────────")}"
  puts "  Local version    : #{cyan(local_version)}"
  puts "  Compositor       : #{File.exists?(COMPOSITOR) ? green("installed") : red("missing")}"
  puts "  Shell binary     : #{File.exists?(SHELL_BIN) ? green("installed") : red("missing")}"
  puts "  System apps dir  : #{cyan(BLUE_APPS)}"
  puts "  Wallpapers dir   : #{cyan(BLUE_WALLS)}"
  puts "  User config      : #{cyan(user_config)}"
  if Dir.exists?(BLUE_APPS)
    apps = Dir.children(BLUE_APPS).select { |d| File.directory?(File.join(BLUE_APPS, d)) }
    if apps.empty?
      puts "  Installed apps   : #{dim("none")}"
    else
      puts "  Installed apps   :"
      apps.each { |a| puts "    #{green("•")} #{a}" }
    end
  end
  puts ""
end

def cmd_start(bigpicture = false)
  if bigpicture
    puts yellow("  Big Picture mode — coming in v0.7!")
    exit 0
  end
  if ENV.has_key?("WAYLAND_DISPLAY") || ENV.has_key?("DISPLAY")
    cmd_dev
  else
    die "blue-compositor not found at #{COMPOSITOR}. Run: sudo blue update" unless File.exists?(COMPOSITOR)
    info "Starting Blue Compositor (TTY/DRM mode)..."
    Process.exec(COMPOSITOR, [] of String)
  end
end

def cmd_dev
  die "blue-environment not found at #{SHELL_BIN}. Run: sudo blue update" unless File.exists?(SHELL_BIN)
  info "Starting Blue Environment (nested mode)..."
  Process.exec(SHELL_BIN, [] of String)
end

def cmd_update
  puts ""
  info "Checking for updates..."
  lv = local_version
  rv = remote_version
  die "Cannot reach GitHub. Check internet connection." if rv.nil?
  puts "  Local  : #{cyan(lv)}"
  puts "  Remote : #{cyan(rv)}"
  unless newer?(rv, lv)
    ok "Already up to date (#{lv})"
    return
  end
  puts "\n  #{yellow("Update available: #{lv} → #{rv}")}"
  unless blue_share_writable?
    warn "#{BLUE_SHARE} is not writable. Try: sudo blue update"
    exit 1
  end
  FileUtils.mkdir_p(BLUE_LIBS)
  [
    {url: "#{GITHUB_REL}/v#{rv}/blue-environment", dest: SHELL_BIN},
    {url: "#{GITHUB_REL}/v#{rv}/blue-compositor",  dest: COMPOSITOR},
  ].each do |pair|
    File.delete(pair[:dest]) if File.exists?(pair[:dest])
    die "Failed to download #{File.basename(pair[:dest])}" unless download_file(pair[:url], pair[:dest])
    File.chmod(pair[:dest], 0o755)
  end
  File.write(VERSION_FILE, rv)
  ok "Updated to v#{rv}!"
end

def cmd_install(name : String)
  puts ""
  info "Fetching package list..."
  list = fetch_app_list
  die "Cannot fetch package list from GitHub." if list.nil?
  unless list.has_key?(name)
    err "Package '#{name}' not found."
    puts "\n  Available packages:"
    list.keys.each { |k| puts "    #{green("•")} #{k}" }
    exit 1
  end
  dest = File.join(BLUE_APPS, name)
  unless blue_share_writable?
    warn "#{BLUE_APPS} is not writable. Try: sudo blue install #{name}"
    exit 1
  end
  if Dir.exists?(dest)
    warn "#{name} is already installed."
    print "  Reinstall? [y/N] "
    ans = gets
    unless ans && ans.strip.downcase == "y"
      info "Aborted."
      exit 0
    end
    FileUtils.rm_rf(dest)
  end
  tmp = "/tmp/blue-install-#{name}.tar.gz"
  die "Download failed" unless download_file(list[name], tmp)
  FileUtils.mkdir_p(dest)
  result = Process.run("tar", args: ["-xzf", tmp, "-C", dest], output: Process::Redirect::Inherit, error: Process::Redirect::Inherit)
  File.delete(tmp) rescue nil
  if result.success?
    ok "#{name} installed to #{dest}"
  else
    FileUtils.rm_rf(dest)
    die "Extraction failed"
  end
end

def cmd_remove(name : String)
  dest = File.join(BLUE_APPS, name)
  die "'#{name}' is not installed" unless Dir.exists?(dest)
  unless File::Info.writable?(dest)
    warn "Cannot remove #{dest}. Try: sudo blue remove #{name}"
    exit 1
  end
  print "  Remove #{bold(name)}? [y/N] "
  ans = gets
  unless ans && ans.strip.downcase == "y"
    info "Aborted."
    exit 0
  end
  FileUtils.rm_rf(dest)
  ok "#{name} removed."
end

def cmd_search(query : String)
  puts ""
  info "Fetching package list..."
  list = fetch_app_list
  die "Cannot fetch package list." if list.nil?
  matches = list.select { |k, _| k.downcase.includes?(query.downcase) }
  if matches.empty?
    warn "No packages matching '#{query}'"
  else
    puts "  Matching #{cyan(query)}:"
    matches.each do |name, _|
      installed = Dir.exists?(File.join(BLUE_APPS, name))
      puts "    #{green("•")} #{bold(name)} #{installed ? green("[installed]") : dim("[available]")}"
    end
  end
  puts ""
end

def cmd_wallpaper(action : String, arg : String?)
  case action
  when "list"
    puts "  Wallpapers in #{cyan(BLUE_WALLS)}:"
    if Dir.exists?(BLUE_WALLS)
      Dir.glob(File.join(BLUE_WALLS, "*.{png,jpg,jpeg}")).each do |f|
        puts "    #{green("•")} #{File.basename(f)}"
      end
    end
    Dir.glob("/usr/share/wallpapers/*.{png,jpg}").each do |f|
      puts "    #{green("•")} [system] #{File.basename(f)}"
    end
  when "set"
    die "Usage: blue wallpaper set <filename>" if arg.nil?
    path = File.exists?(arg) ? arg : File.join(BLUE_WALLS, arg)
    die "Wallpaper not found: #{path}" unless File.exists?(path)
    cfg_file = File.join(user_config, "config.json")
    if File.exists?(cfg_file)
      begin
        cfg = JSON.parse(File.read(cfg_file)).as_h
        cfg["wallpaper"] = JSON::Any.new("file://#{path}")
        File.write(cfg_file, cfg.to_json)
        ok "Wallpaper set to #{File.basename(path)}"
      rescue ex
        warn "Could not update config: #{ex.message}"
      end
    else
      warn "Blue Environment config not found — launch BE first"
    end
  else
    err "Usage: blue wallpaper [list|set <file>]"
  end
end

def cmd_compositor(action : String)
  case action
  when "start"
    die "Compositor not found" unless File.exists?(COMPOSITOR)
    info "Starting compositor..."
    Process.exec(COMPOSITOR, [] of String)
  when "stop"
    Process.run("pkill", args: ["-f", "blue-compositor"])
    ok "Compositor stopped"
  when "restart"
    Process.run("pkill", args: ["-f", "blue-compositor"])
    sleep(1.second)
    die "Compositor not found" unless File.exists?(COMPOSITOR)
    Process.exec(COMPOSITOR, [] of String)
  when "status"
    r = Process.run("pgrep", args: ["-f", "blue-compositor"], output: Process::Redirect::Pipe)
    r.exit_code == 0 ? ok("Compositor is running") : warn("Compositor is not running")
  else
    err "Usage: blue compositor [start|stop|restart|status]"
  end
end

def print_help
  puts ""
  puts "  #{bold("blue")} — Blue Environment launcher v#{VERSION}"
  puts "  #{dim("──────────────────────────────────────────────────────")}"
  puts ""
  puts "  #{bold("Session:")}"
  puts "    #{cyan("blue start")}                       Start compositor + shell"
  puts "    #{cyan("blue dev")}                         Launch shell (nested mode)"
  puts ""
  puts "  #{bold("Packages:")}"
  puts "    #{cyan("blue install")} #{dim("<name>")}              Install addon app"
  puts "    #{cyan("blue remove")}  #{dim("<name>")}              Remove installed app"
  puts "    #{cyan("blue search")}  #{dim("<query>")}             Search available packages"
  puts ""
  puts "  #{bold("System:")}"
  puts "    #{cyan("blue update")}                      Download and install updates"
  puts "    #{cyan("blue compositor")} #{dim("[start|stop|status]")} Manage compositor"
  puts "    #{cyan("blue wallpaper list")}              List available wallpapers"
  puts "    #{cyan("blue wallpaper set")} #{dim("<file>")}        Set wallpaper"
  puts "    #{cyan("blue info")}                        Show version + installed apps"
  puts "    #{cyan("blue help")}                        This help"
  puts ""
  puts "  #{bold("Paths:")}"
  puts "    Binaries   : #{dim(BLUE_SHARE)}"
  puts "    Apps       : #{dim(BLUE_APPS)}"
  puts "    Wallpapers : #{dim(BLUE_WALLS)}"
  puts "    User config: #{dim("~/.config/Blue-Environment")}"
  puts ""
end

args = ARGV.to_a
cmd  = args.shift? || "help"

case cmd
when "start"       then cmd_start(args.includes?("--bigpicture"))
when "dev"         then cmd_dev
when "update"      then cmd_update
when "install"
  n = args.shift?
  n ? cmd_install(n) : (err "Usage: blue install <name>"; exit 1)
when "remove"
  n = args.shift?
  n ? cmd_remove(n) : (err "Usage: blue remove <name>"; exit 1)
when "search"
  q = args.shift?
  q ? cmd_search(q) : (err "Usage: blue search <query>"; exit 1)
when "wallpaper"   then cmd_wallpaper(args.shift? || "list", args.shift?)
when "compositor"  then cmd_compositor(args.shift? || "status")
when "info"        then cmd_info
when "help", "--help", "-h" then print_help
else
  err "Unknown command: #{cmd}"; print_help; exit 1
end

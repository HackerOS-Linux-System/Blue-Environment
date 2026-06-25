# frozen_string_literal: true
# Blue Launcher — CLI command implementations

def blue_share_writable?
  File.writable?(BLUE_SHARE) || Process.uid == 0
end

# ── info ──────────────────────────────────────────────────────────────────
def cmd_info
  lv = local_version
  rv = remote_version
  print_hr
  puts "#{bold('Blue Environment')}  v#{lv}"
  puts "  Compositor : #{File.exist?(COMPOSITOR) ? green('installed') : red('not found')}"
  puts "  Shell      : #{File.exist?(SHELL_BIN)  ? green('installed') : red('not found')}"
  puts "  Config     : #{user_config}"
  puts "  Apps dir   : #{BLUE_APPS}"
  if rv
    if newer?(rv, lv)
      puts "  Update     : #{yellow("v#{rv} available — run: blue update")}"
    else
      puts "  Update     : #{green('up to date')}"
    end
  end
  print_hr

  # Installed add-on apps
  if Dir.exist?(BLUE_APPS)
    apps = Dir.glob("#{BLUE_APPS}/*").select { |d| File.directory?(d) }
    if apps.any?
      puts "Installed add-ons:"
      apps.each { |a| puts "  • #{File.basename(a)}" }
    else
      puts dim("No add-on apps installed (#{BLUE_APPS} is empty)")
    end
  end
end

# ── start ─────────────────────────────────────────────────────────────────
def cmd_start(bigpicture = false)
  env = bigpicture ? { 'VITE_BLUE_UI_MODE' => 'bigpicture' } : {}
  die "Shell binary not found: #{SHELL_BIN}" unless File.exist?(SHELL_BIN)
  info "Starting Blue Environment#{bigpicture ? ' (Big Picture)' : ''}..."
  exec(env, SHELL_BIN)
end

# ── dev ───────────────────────────────────────────────────────────────────
def cmd_dev
  info "Starting dev server (Vite + Tauri)..."
  exec('npm run tauri dev')
end

# ── update ────────────────────────────────────────────────────────────────
def cmd_update
  lv = local_version
  rv = remote_version

  if rv.nil?
    err "Cannot reach GitHub — check your internet connection"
    exit 1
  end

  unless newer?(rv, lv)
    ok "Already up to date (v#{lv})"
    return
  end

  info "Updating v#{lv} → v#{rv}..."
  die "Root required to update (run: sudo blue update)" unless blue_share_writable?

  pkg_url = "#{GITHUB_REL}/v#{rv}/blue-environment-#{rv}.tar.gz"
  tmp = "/tmp/blue-update-#{rv}.tar.gz"

  unless download_file(pkg_url, tmp)
    die "Failed to download update package"
  end

  info "Extracting..."
  unless system("tar -xzf '#{tmp}' -C '#{BLUE_SHARE}' --strip-components=1 2>/dev/null")
    die "Extraction failed"
  end

  FileUtils.rm_f(tmp)
  File.write(VERSION_FILE, rv)
  ok "Updated to v#{rv}"
end

# ── install ───────────────────────────────────────────────────────────────
def cmd_install(name)
  apps = fetch_app_list
  if apps.empty?
    die "Could not fetch app list from #{APPS_LIST_URL}"
  end

  url = apps[name]
  unless url
    err "Unknown app: '#{name}'"
    puts "Available: #{apps.keys.join(', ')}"
    exit 1
  end

  die "Root required to install (run: sudo blue install #{name})" unless blue_share_writable?

  dest_dir = File.join(BLUE_APPS, name)
  if File.exist?(dest_dir)
    warn "'#{name}' is already installed (#{dest_dir})"
    return
  end

  tmp = "/tmp/blue-app-#{name}.tar.gz"
  unless download_file(url, tmp)
    die "Download failed for #{name}"
  end

  FileUtils.mkdir_p(dest_dir)
  unless system("tar -xzf '#{tmp}' -C '#{dest_dir}' --strip-components=1 2>/dev/null || tar -xzf '#{tmp}' -C '#{dest_dir}' 2>/dev/null")
    FileUtils.rm_rf(dest_dir)
    die "Extraction failed"
  end

  FileUtils.rm_f(tmp)

  # Make the main binary executable (common convention: same name as directory)
  main_bin = File.join(dest_dir, name)
  FileUtils.chmod(0o755, main_bin) if File.exist?(main_bin)

  ok "Installed '#{name}' → #{dest_dir}"
end

# ── remove ────────────────────────────────────────────────────────────────
def cmd_remove(name)
  dest_dir = File.join(BLUE_APPS, name)
  unless File.exist?(dest_dir)
    err "'#{name}' is not installed"
    exit 1
  end

  die "Root required to remove (run: sudo blue remove #{name})" unless blue_share_writable?

  FileUtils.rm_rf(dest_dir)
  ok "Removed '#{name}'"
end

# ── search ────────────────────────────────────────────────────────────────
def cmd_search(query)
  apps = fetch_app_list
  if apps.empty?
    die "Could not fetch app list"
  end

  hits = apps.select { |k, _| k.downcase.include?(query.downcase) }
  if hits.empty?
    puts "No apps found matching '#{query}'"
  else
    puts "Results for '#{query}':"
    hits.each do |k, url|
      installed = File.exist?(File.join(BLUE_APPS, k)) ? green(' [installed]') : ''
      puts "  #{bold(k)}#{installed}"
      puts "  #{dim(url)}"
    end
  end
end

# ── wallpaper ─────────────────────────────────────────────────────────────
def cmd_wallpaper(action, arg)
  config_path = File.join(user_config, 'config.json')

  case action
  when 'set'
    arg = File.expand_path(arg)
    unless File.exist?(arg)
      die "File not found: #{arg}"
    end
    config = File.exist?(config_path) ? (JSON.parse(File.read(config_path)) rescue {}) : {}
    config['wallpaper'] = "file://#{arg}"
    FileUtils.mkdir_p(user_config)
    File.write(config_path, JSON.pretty_generate(config))
    ok "Wallpaper set to #{arg}"
    info "Restart Blue Environment to apply changes"

  when 'list'
    patterns = [
      "#{BLUE_WALLS}/*.png", "#{BLUE_WALLS}/*.jpg",
      "/usr/share/wallpapers/*.png", "/usr/share/wallpapers/*.jpg",
      "/usr/share/backgrounds/*.png", "/usr/share/backgrounds/*.jpg",
    ]
    walls = patterns.flat_map { |p| Dir.glob(p) }.uniq
    if walls.empty?
      puts dim("No wallpapers found")
    else
      puts "Available wallpapers:"
      walls.each { |w| puts "  #{w}" }
    end

  when 'reset'
    config = File.exist?(config_path) ? (JSON.parse(File.read(config_path)) rescue {}) : {}
    config['wallpaper'] = "file://#{BLUE_WALLS}/default.png"
    File.write(config_path, JSON.pretty_generate(config))
    ok "Wallpaper reset to default"

  else
    err "Unknown wallpaper action: #{action}"
    puts "Usage: blue wallpaper set <path>"
    puts "       blue wallpaper list"
    puts "       blue wallpaper reset"
    exit 1
  end
end

# ── compositor ────────────────────────────────────────────────────────────
def cmd_compositor(action)
  case action
  when 'start'
    die "Compositor not found: #{COMPOSITOR}" unless File.exist?(COMPOSITOR)
    info "Starting Blue Compositor..."
    exec(COMPOSITOR)
  when 'stop'
    system("pkill -x blue-compositor")
    ok "Compositor stopped"
  when 'restart'
    system("pkill -x blue-compositor")
    sleep 0.5
    die "Compositor not found: #{COMPOSITOR}" unless File.exist?(COMPOSITOR)
    Process.spawn(COMPOSITOR)
    ok "Compositor restarted"
  when 'status'
    pid = `pgrep -x blue-compositor 2>/dev/null`.strip
    if pid.empty?
      puts "  #{red('●')} blue-compositor is not running"
    else
      puts "  #{green('●')} blue-compositor running (PID #{pid})"
    end
  else
    err "Unknown compositor action: #{action}"
    puts "Usage: blue compositor start|stop|restart|status"
    exit 1
  end
end

# ── help ──────────────────────────────────────────────────────────────────
def print_help
  lv = local_version
  print_hr
  puts "#{bold('Blue Environment')} v#{lv} — launcher"
  print_hr
  cmds = [
    ["start",              "Start the desktop shell"],
    ["start bigpicture",   "Start in Big Picture (TV) mode"],
    ["dev",                "Start development server (Vite + Tauri)"],
    ["info",               "Show version and installed components"],
    ["update",             "Update Blue Environment"],
    ["install <name>",     "Install an add-on app"],
    ["remove <name>",      "Remove an add-on app"],
    ["search <query>",     "Search available add-on apps"],
    ["wallpaper set <f>",  "Set desktop wallpaper"],
    ["wallpaper list",     "List available wallpapers"],
    ["wallpaper reset",    "Reset wallpaper to default"],
    ["compositor start",   "Start the Wayland compositor"],
    ["compositor stop",    "Stop the compositor"],
    ["compositor restart", "Restart the compositor"],
    ["compositor status",  "Show compositor status"],
  ]
  max = cmds.map { |c, _| c.length }.max
  cmds.each { |cmd, desc| puts "  #{bold('blue')} #{cmd.ljust(max)}  #{dim(desc)}" }
  print_hr
end

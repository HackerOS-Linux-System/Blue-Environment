#!/usr/bin/env ruby
# frozen_string_literal: true

# blue-installer-apply.rb — Blue Installer's privileged backend step.
#
# Invoked via `pkexec` by BlueInstallerApp/mod.rs::installer_run(), which
# means this script runs AS ROOT. Treat every argument as hostile until
# validated — this is the one place in the whole app that can destroy data.

require 'optparse'
require 'fileutils'
require 'io/console'

# Domyślne wartości
options = {
  disk: nil,
  confirm_erase: nil,
  locale: 'en_US.UTF-8',
  keyboard: 'us',
  hostname: 'blue-pc',
  username: nil,
  fullname: '',
  autologin: false
}

# Parsowanie argumentów CLI
OptionParser.new do |opts|
  opts.on('--disk DISK') { |v| options[:disk] = v }
  opts.on('--confirm-erase DISK') { |v| options[:confirm_erase] = v }
  opts.on('--locale LOCALE') { |v| options[:locale] = v }
  opts.on('--keyboard KEYBOARD') { |v| options[:keyboard] = v }
  opts.on('--hostname HOSTNAME') { |v| options[:hostname] = v }
  opts.on('--username USERNAME') { |v| options[:username] = v }
  opts.on('--fullname FULLNAME') { |v| options[:fullname] = v }
  opts.on('--autologin') { options[:autologin] = true }
end.parse!

def progress(percent, message)
  puts "PROGRESS #{percent} #{message}"
  $stdout.flush
end

def run_cmd(cmd)
  system(cmd)
  unless $?.success?
    warn "Błąd podczas wykonywania: #{cmd}"
    exit 1
  end
end

def run_cmd_silent(cmd)
  system("#{cmd} >/dev/null 2>&1")
end

# ── Kontrole bezpieczeństwa ───────────────────────────────────────────────
if options[:disk].nil? || options[:disk].empty?
  warn "Missing --disk"
  exit 1
end

if options[:confirm_erase] != options[:disk]
  warn "Safety check failed: --confirm-erase (#{options[:confirm_erase]}) does not match --disk (#{options[:disk]})"
  exit 1
end

unless File.blockdev?(options[:disk])
  warn "#{options[:disk]} is not a block device"
  exit 1
end

if options[:username].nil? || options[:username].empty?
  warn "Missing --username"
  exit 1
end

# Odczyt hasła ze stdin bez echa (odpowiednik read -s)
password = $stdin.noecho(&:gets)&.chomp
password ||= ""

progress(2, "Reading password")

# ── 1. Partycjonowanie dysku ──────────────────────────────────────────────
disk = options[:disk]
progress(5, "Partitioning #{disk}")

run_cmd("parted -s \"#{disk}\" -- mklabel gpt")
run_cmd("parted -s \"#{disk}\" -- mkpart ESP fat32 1MiB 513MiB")
run_cmd("parted -s \"#{disk}\" -- set 1 esp on")
run_cmd("parted -s \"#{disk}\" -- mkpart root ext4 513MiB 100%")
run_cmd("partprobe \"#{disk}\"")
sleep 1

if disk =~ /nvme|mmcblk/
  efi_part = "#{disk}p1"
  root_part = "#{disk}p2"
else
  efi_part = "#{disk}1"
  root_part = "#{disk}2"
end

progress(12, "Creating filesystems")
run_cmd("mkfs.fat -F32 \"#{efi_part}\"")
run_cmd("mkfs.ext4 -F \"#{root_part}\"")

progress(16, "Mounting target")
target = '/mnt/blue-install'
FileUtils.mkdir_p(target)
run_cmd("mount \"#{root_part}\" \"#{target}\"")
FileUtils.mkdir_p("#{target}/boot/efi")
run_cmd("mount \"#{efi_part}\" \"#{target}/boot/efi\"")

# Funkcja czyszcząca (odpowiednik trap w Bashu)
define_method(:cleanup) do
  system("umount -R \"#{target}\" >/dev/null 2>&1")
end

Kernel.at_exit do
  cleanup
end

# ── 2. Kopiowanie systemu na dysk ─────────────────────────────────────────
progress(20, "Copying system files (this takes a while)…")
source_root = '/run/initramfs/live/LiveOS/squashfs.img'

excludes = [
  '--exclude=/proc/*', '--exclude=/sys/*', '--exclude=/dev/*',
  '--exclude=/run/*', '--exclude=/tmp/*', '--exclude=/mnt/*'
]

if File.exist?(source_root)
  mount_src = '/mnt/blue-live-src'
  FileUtils.mkdir_p(mount_src)
  run_cmd("mount -o loop,ro \"#{source_root}\" \"#{mount_src}\"")
  
  rsync_cmd = ["rsync -aHAX --info=progress2", *excludes, "\"#{mount_src}/\"", "\"#{target}/\""].join(' ')
  run_cmd(rsync_cmd)
  run_cmd("umount \"#{mount_src}\"")
else
  # Fallback dla czystego rootfs
  rsync_cmd = ["rsync -aHAX --info=progress2", *excludes, '--exclude=/mnt/blue-install/*', '/', "\"#{target}/\""].join(' ')
  run_cmd(rsync_cmd)
end
progress(60, "System files copied")

['proc', 'sys', 'dev', 'run'].each do |dir|
  FileUtils.mkdir_p("#{target}/#{dir}")
  run_cmd("mount --bind /#{dir} \"#{target}/#{dir}\"")
end

# ── 3. Konfiguracja zainstalowanego systemu (chroot) ──────────────────────
progress(65, "Configuring system")

File.write("#{target}/etc/hostname", "#{options[:hostname]}\n")

uuid_root = `blkid -s UUID -o value "#{root_part}"`.strip
uuid_efi  = `blkid -s UUID -o value "#{efi_part}"`.strip

fstab_content = <<~FSTAB
  UUID=#{uuid_root}  /          ext4  defaults  0 1
  UUID=#{uuid_efi}   /boot/efi  vfat  umask=0077  0 2
FSTAB
File.write("#{target}/etc/fstab", fstab_content)

progress(72, "Setting locale and keyboard")
unless run_cmd_silent("chroot \"#{target}\" localectl set-locale \"LANG=#{options[:locale]}\"")
  File.write("#{target}/etc/locale.conf", "LANG=#{options[:locale]}\n")
end

unless run_cmd_silent("chroot \"#{target}\" localectl set-keymap \"#{options[:keyboard]}\"")
  File.write("#{target}/etc/vconsole.conf", "KEYMAP=#{options[:keyboard]}\n")
end

progress(78, "Creating user account")
run_cmd("chroot \"#{target}\" useradd -m -c \"#{options[:fullname]}\" -G wheel,audio,video,input \"#{options[:username]}\"")

# Bezpieczne przekazanie hasła do chpasswd przez IO.popen
IO.popen("chroot \"#{target}\" chpasswd", "w") do |io|
  io.puts("#{options[:username]}:#{password}")
end
unless $?.success?
  warn "Nie udało się ustawić hasła użytkownika."
  exit 1
end

# Czyszczenie zmiennej z hasłem ASAP
password = nil

if options[:autologin]
  progress(80, "Configuring auto-login")
  begin
    FileUtils.mkdir_p("#{target}/etc/bedm/bedm.toml.d")
    autologin_content = <<~TOML
      autologin_user = "#{options[:username]}"
      autologin_session = "blue-environment"
    TOML
    File.write("#{target}/etc/bedm/bedm.toml.d/autologin.toml", autologin_content)
  rescue => e
    warn "Nieudana konfiguracja autologinu: #{e.message}"
  end
end

progress(85, "Installing bootloader")
grub_install_success = run_cmd_silent("chroot \"#{target}\" grub2-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=BlueEnvironment \"#{disk}\"")
unless grub_install_success
  run_cmd("chroot \"#{target}\" grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=BlueEnvironment \"#{disk}\"")
end

grub_config_success = run_cmd_silent("chroot \"#{target}\" grub2-mkconfig -o /boot/grub2/grub.cfg")
unless grub_config_success
  run_cmd("chroot \"#{target}\" grub-mkconfig -o /boot/grub/grub.cfg")
end

progress(95, "Finishing up")
run_cmd("sync")

progress(100, "Installation complete")

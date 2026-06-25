# frozen_string_literal: true
# Blue Launcher — configuration constants
# Installed to: /usr/share/Blue-Environment/lib/blue/src/config.rb

VERSION       = "0.7"
GITHUB_RAW    = "https://raw.githubusercontent.com/LegendaryOS-Linux-System/Blue-Environment/main"
GITHUB_REL    = "https://github.com/LegendaryOS-Linux-System/Blue-Environment/releases/download"
BLUE_SHARE    = "/usr/share/Blue-Environment"
BLUE_LIBS     = "#{BLUE_SHARE}/lib"
BLUE_APPS     = "#{BLUE_SHARE}/apps"
BLUE_WALLS    = "#{BLUE_SHARE}/wallpapers"
COMPOSITOR    = "#{BLUE_LIBS}/blue-compositor"
SHELL_BIN     = "#{BLUE_SHARE}/blue-environment"
VERSION_FILE  = "#{BLUE_SHARE}/.version"

APPS_LIST_URL    = "#{GITHUB_RAW}/config/list.json"
VERSION_JSON_URL = "#{GITHUB_RAW}/config/version.json"

def user_config
  File.join(ENV.fetch("HOME", "/root"), ".config", "Blue-Environment")
end

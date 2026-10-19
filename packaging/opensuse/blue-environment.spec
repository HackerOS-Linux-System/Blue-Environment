Name:           blue-environment
Version:        @VERSION@
Release:        1%{?dist}
Summary:        Blue Environment Wayland Desktop Shell
License:        GPL-3.0
Group:          System/GUI/Other
URL:            https://github.com/LegendaryOS-Linux-System/Blue-Environment
Requires:       ruby
Recommends:     bedm

%description
Blue Environment — production Wayland desktop shell built with
Tauri and Svelte. openSUSE/SUSE packaging variant.

%install
mkdir -p %{buildroot}/usr/share/Blue-Environment/lib
mkdir -p %{buildroot}/usr/share/wayland-sessions
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/bin
install -m755 %{_sourcedir}/blue-environment %{buildroot}/usr/share/Blue-Environment/blue-environment
install -m755 %{_sourcedir}/blue-compositor %{buildroot}/usr/share/Blue-Environment/lib/blue-compositor
install -m755 %{_sourcedir}/blue %{buildroot}/usr/bin/blue
printf '[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/lib/blue-compositor\nType=Application\nDesktopNames=Blue\n' > %{buildroot}/usr/share/wayland-sessions/blue-environment.desktop
printf '[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/blue-environment\nIcon=/usr/share/Blue-Environment/icon.png\nType=Application\nCategories=System;\n' > %{buildroot}/usr/share/applications/blue-environment.desktop

%files
/usr/share/Blue-Environment/
/usr/share/wayland-sessions/blue-environment.desktop
/usr/share/applications/blue-environment.desktop
/usr/bin/blue

%changelog
* Mon Jul 20 2026 Blue Environment packaging - @VERSION@-1
- openSUSE/SUSE packaging variant added

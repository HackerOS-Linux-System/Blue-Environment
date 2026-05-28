Name: blue-environment
Version: @VERSION@
Release: 1%{?dist}
Summary: Blue Environment Wayland Desktop Shell
License: GPL-3.0
%description
Blue Environment — Production Wayland desktop shell built with Smithay, Tauri and React.
%install
mkdir -p %{buildroot}/usr/share/Blue-Environment/lib
mkdir -p %{buildroot}/usr/share/wayland-sessions
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/local/bin
install -m755 %{_sourcedir}/blue-environment %{buildroot}/usr/share/Blue-Environment/blue-environment
install -m755 %{_sourcedir}/blue-compositor %{buildroot}/usr/share/Blue-Environment/lib/blue-compositor
install -m755 %{_sourcedir}/blue %{buildroot}/usr/local/bin/blue
printf '[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/lib/blue-compositor\nType=Application\nDesktopNames=Blue\n' > %{buildroot}/usr/share/wayland-sessions/blue-environment.desktop
printf '[Desktop Entry]\nName=Blue Environment\nExec=/usr/share/Blue-Environment/blue-environment\nIcon=/usr/share/Blue-Environment/icon.png\nType=Application\nCategories=System;\n' > %{buildroot}/usr/share/applications/blue-environment.desktop
%files
/usr/share/Blue-Environment/
/usr/share/wayland-sessions/blue-environment.desktop
/usr/share/applications/blue-environment.desktop
/usr/local/bin/blue

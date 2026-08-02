Name:           bedm
Version:        1.0.0
Release:        1%{?dist}
Summary:        Blue Environment Display Manager
License:        GPL-3.0
URL:            https://github.com/HackerOS-Linux-System/Blue-Environment
BuildArch:      x86_64 aarch64

BuildRequires:  cargo
BuildRequires:  nodejs >= 18
BuildRequires:  npm
BuildRequires:  pam-devel
BuildRequires:  openssl-devel
BuildRequires:  systemd-rpm-macros

Requires:       pam
Requires:       dbus
Requires:       systemd

Conflicts:      gdm sddm lightdm

%description
BEDM is a production display manager for Linux built with Rust + Tauri.
It provides a glassmorphism login screen with PAM authentication, Wayland
and X11 session support, user avatars, and power management.

%build
make build

%install
make install DESTDIR=%{buildroot}

%post
%systemd_post bedm.service

%preun
%systemd_preun bedm.service

%postun
%systemd_postun_with_restart bedm.service

%files
%license LICENSE
%doc README.md
%{_bindir}/bedm
%{_bindir}/bedm-greeter
%{_unitdir}/bedm.service
%config(noreplace) %{_sysconfdir}/bedm/bedm.toml
%{_sysconfdir}/pam.d/bedm
%dir %{_localstatedir}/lib/bedm
%dir %{_localstatedir}/log/bedm

%changelog
* 2026 LegendaryOS Team <legendaryos.linux.system@gmail.com> - 1.0.0-1
- Initial release

PREFIX       ?= /usr
SYSCONFDIR   ?= /etc
VARDIR       ?= /var
RUNDIR       ?= /run
SYSTEMDDIR   ?= /usr/lib/systemd/system
BINDIR       := $(PREFIX)/bin
SHAREDIR     := $(PREFIX)/share/bedm
PAMDIR       := $(SYSCONFDIR)/pam.d

CARGO        := cargo
NPM          := npm
TAURI        := npx tauri

# Colors
GREEN  := \033[0;32m
YELLOW := \033[1;33m
CYAN   := \033[0;36m
RESET  := \033[0m

.PHONY: all build build-daemon build-greeter install uninstall \
        install-systemd install-pam install-config clean \
        dev-daemon dev-greeter check lint help

all: build

## ── Build ──────────────────────────────────────────────────────────────────

build: build-daemon build-greeter
	@echo "$(GREEN)✓ BEDM build complete$(RESET)"

build-daemon:
	@echo "$(CYAN)→ Building bedm daemon...$(RESET)"
	cd bedm-daemon && $(CARGO) build --release
	@echo "$(GREEN)✓ bedm daemon built$(RESET)"

build-greeter:
	@echo "$(CYAN)→ Building bedm-ui...$(RESET)"
	cd bedm-ui && $(NPM) install && $(NPM) run build
	@echo "$(CYAN)→ Building bedm-greeter (Tauri)...$(RESET)"
	cd bedm-greeter && $(TAURI) build
	@echo "$(GREEN)✓ bedm-greeter built$(RESET)"

## ── Development ────────────────────────────────────────────────────────────

dev-greeter:
	@echo "$(CYAN)→ Starting greeter in dev mode...$(RESET)"
	cd bedm-greeter && $(TAURI) dev

dev-ui:
	@echo "$(CYAN)→ Starting UI dev server...$(RESET)"
	cd bedm-ui && $(NPM) run dev

## ── Install ────────────────────────────────────────────────────────────────

install: build install-systemd install-pam install-config
	@echo "$(CYAN)→ Installing binaries...$(RESET)"
	install -Dm755 bedm-daemon/target/release/bedm     $(DESTDIR)$(BINDIR)/bedm
	install -Dm755 bedm-greeter/target/release/bedm-greeter $(DESTDIR)$(BINDIR)/bedm-greeter

	@echo "$(CYAN)→ Creating runtime dirs...$(RESET)"
	install -dm755 $(DESTDIR)$(RUNDIR)/bedm
	install -dm755 $(DESTDIR)$(RUNDIR)/bedm/sessions
	install -dm755 $(DESTDIR)$(VARDIR)/lib/bedm
	install -dm755 $(DESTDIR)$(VARDIR)/log/bedm

	@echo "$(GREEN)✓ BEDM installed to $(DESTDIR)$(BINDIR)$(RESET)"
	@echo ""
	@echo "$(YELLOW)Next steps:$(RESET)"
	@echo "  1. Disable your current DM:  systemctl disable gdm sddm lightdm 2>/dev/null || true"
	@echo "  2. Enable BEDM:              systemctl enable bedm"
	@echo "  3. Start BEDM:               systemctl start bedm"
	@echo "  OR reboot to apply."

install-systemd:
	@echo "$(CYAN)→ Installing systemd service...$(RESET)"
	install -Dm644 systemd/bedm.service $(DESTDIR)$(SYSTEMDDIR)/bedm.service
	@if [ -z "$(DESTDIR)" ]; then systemctl daemon-reload; fi

install-pam:
	@echo "$(CYAN)→ Installing PAM config...$(RESET)"
	install -Dm644 config/pam-bedm $(DESTDIR)$(PAMDIR)/bedm

install-config:
	@echo "$(CYAN)→ Installing default config...$(RESET)"
	@if [ ! -f "$(DESTDIR)$(SYSCONFDIR)/bedm/bedm.toml" ]; then \
		install -Dm644 config/bedm.toml $(DESTDIR)$(SYSCONFDIR)/bedm/bedm.toml; \
		echo "$(GREEN)✓ Config installed to $(DESTDIR)$(SYSCONFDIR)/bedm/bedm.toml$(RESET)"; \
	else \
		echo "$(YELLOW)! Existing config preserved ($(DESTDIR)$(SYSCONFDIR)/bedm/bedm.toml)$(RESET)"; \
	fi

## ── Uninstall ──────────────────────────────────────────────────────────────

uninstall:
	@echo "$(CYAN)→ Uninstalling BEDM...$(RESET)"
	-systemctl stop bedm 2>/dev/null || true
	-systemctl disable bedm 2>/dev/null || true
	rm -f $(BINDIR)/bedm
	rm -f $(BINDIR)/bedm-greeter
	rm -f $(SYSTEMDDIR)/bedm.service
	rm -f $(PAMDIR)/bedm
	-systemctl daemon-reload
	@echo "$(GREEN)✓ BEDM uninstalled$(RESET)"
	@echo "$(YELLOW)Note: config at /etc/bedm/ and logs at /var/log/bedm/ kept$(RESET)"

## ── Quality ────────────────────────────────────────────────────────────────

check:
	cd bedm-daemon && $(CARGO) check
	cd bedm-greeter && $(CARGO) check

lint:
	cd bedm-daemon && $(CARGO) clippy -- -D warnings
	cd bedm-greeter && $(CARGO) clippy -- -D warnings
	cd bedm-ui && $(NPM) run lint 2>/dev/null || true

clean:
	cd bedm-daemon && $(CARGO) clean
	cd bedm-greeter && $(CARGO) clean
	rm -rf bedm-ui/dist bedm-ui/node_modules

## ── Package ────────────────────────────────────────────────────────────────

dist: build
	@echo "$(CYAN)→ Creating distribution tarball...$(RESET)"
	mkdir -p dist-tmp/bedm-$(VERSION)
	cp -r bedm-daemon/target/release/bedm           dist-tmp/bedm-$(VERSION)/
	cp -r bedm-greeter/target/release/bedm-greeter  dist-tmp/bedm-$(VERSION)/
	cp -r config/ systemd/ Makefile README.md       dist-tmp/bedm-$(VERSION)/
	tar -czf bedm-$(VERSION).tar.gz -C dist-tmp bedm-$(VERSION)
	rm -rf dist-tmp
	@echo "$(GREEN)✓ bedm-$(VERSION).tar.gz$(RESET)"

## ── Help ───────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "$(CYAN)BEDM — Blue Environment Display Manager$(RESET)"
	@echo ""
	@echo "$(YELLOW)Build:$(RESET)"
	@echo "  make build          — Build daemon + greeter (release)"
	@echo "  make build-daemon   — Build only the daemon"
	@echo "  make build-greeter  — Build only the greeter"
	@echo ""
	@echo "$(YELLOW)Development:$(RESET)"
	@echo "  make dev-greeter    — Start greeter in Tauri dev mode"
	@echo "  make dev-ui         — Start React UI dev server"
	@echo ""
	@echo "$(YELLOW)Install (run as root):$(RESET)"
	@echo "  make install        — Full install"
	@echo "  make uninstall      — Remove BEDM"
	@echo ""
	@echo "$(YELLOW)Environment:$(RESET)"
	@echo "  PREFIX=$(PREFIX)  SYSCONFDIR=$(SYSCONFDIR)"
	@echo ""

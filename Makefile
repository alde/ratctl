PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
UDEVDIR ?= /etc/udev/rules.d

.PHONY: build install uninstall

build:
	cargo build --release

install: target/release/ratctl
	install -Dm755 target/release/ratctl $(DESTDIR)$(BINDIR)/ratctl
	install -Dm644 udev/99-ratctl.rules $(DESTDIR)$(UDEVDIR)/99-ratctl.rules
	@echo "Reload udev rules with: sudo udevadm control --reload-rules && sudo udevadm trigger"

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/ratctl
	rm -f $(DESTDIR)$(UDEVDIR)/99-ratctl.rules

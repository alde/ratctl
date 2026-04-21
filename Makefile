PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
UDEVDIR ?= /etc/udev/rules.d

.PHONY: build install uninstall

build:
	cargo build --release

install: target/release/ratctl
	install -Dm755 target/release/ratctl $(DESTDIR)$(BINDIR)/ratctl
	install -Dm644 udev/99-ratctl.rules $(DESTDIR)$(UDEVDIR)/99-ratctl.rules
	udevadm control --reload-rules
	udevadm trigger

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/ratctl
	rm -f $(DESTDIR)$(UDEVDIR)/99-ratctl.rules

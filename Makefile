PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share

.PHONY: all build install uninstall test clean

all: build

build:
	cargo build --release

test:
	cargo test

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/cloudpulse $(DESTDIR)$(BINDIR)/soundcloud
	ln -sf soundcloud $(DESTDIR)$(BINDIR)/cloudpulse
	install -d $(DESTDIR)$(DATADIR)/applications
	install -m 644 soundcloud.desktop $(DESTDIR)$(DATADIR)/applications/soundcloud.desktop
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 assets/soundcloud_app_icon.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/soundcloud.svg

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/soundcloud
	rm -f $(DESTDIR)$(BINDIR)/cloudpulse
	rm -f $(DESTDIR)$(DATADIR)/applications/soundcloud.desktop
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/soundcloud.svg

clean:
	cargo clean

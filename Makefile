PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
APP_ID = com.github.tilix_rust

.PHONY: all build check test install uninstall clean

all: build

build:
	cargo build --release

check: test

test:
	cargo test

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/tilix $(DESTDIR)$(BINDIR)/tilix
	ln -sf tilix $(DESTDIR)$(BINDIR)/tilix-rust
	install -d $(DESTDIR)$(DATADIR)/applications
	install -m 644 data/$(APP_ID).desktop $(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop
	install -d $(DESTDIR)$(DATADIR)/metainfo
	install -m 644 data/$(APP_ID).metainfo.xml $(DESTDIR)$(DATADIR)/metainfo/$(APP_ID).metainfo.xml
	install -d $(DESTDIR)$(DATADIR)/dbus-1/services
	install -m 644 data/$(APP_ID).service $(DESTDIR)$(DATADIR)/dbus-1/services/$(APP_ID).service
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 data/icons/scalable/apps/$(APP_ID).svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
	install -d $(DESTDIR)$(DATADIR)/bash-completion/completions
	install -m 644 data/completions/bash/tilix $(DESTDIR)$(DATADIR)/bash-completion/completions/tilix
	install -d $(DESTDIR)$(DATADIR)/zsh/site-functions
	install -m 644 data/completions/zsh/_tilix $(DESTDIR)$(DATADIR)/zsh/site-functions/_tilix
	install -d $(DESTDIR)$(DATADIR)/fish/vendor_completions.d
	install -m 644 data/completions/fish/tilix.fish $(DESTDIR)$(DATADIR)/fish/vendor_completions.d/tilix.fish

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/tilix
	rm -f $(DESTDIR)$(BINDIR)/tilix-rust
	rm -f $(DESTDIR)$(DATADIR)/applications/$(APP_ID).desktop
	rm -f $(DESTDIR)$(DATADIR)/metainfo/$(APP_ID).metainfo.xml
	rm -f $(DESTDIR)$(DATADIR)/dbus-1/services/$(APP_ID).service
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/$(APP_ID).svg
	rm -f $(DESTDIR)$(DATADIR)/bash-completion/completions/tilix
	rm -f $(DESTDIR)$(DATADIR)/zsh/site-functions/_tilix
	rm -f $(DESTDIR)$(DATADIR)/fish/vendor_completions.d/tilix.fish

clean:
	cargo clean

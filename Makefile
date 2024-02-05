OUT_DIR = bin
VERSION = 0.1.3
BIN_NAME = tiny-portal-gui-$(VERSION)
WIN_BIN = $(OUT_DIR)/$(BIN_NAME)-x86_64-pc-windows-gnu.exe

build:
	cargo build --release -p gui

build-all:
	rm -rf $(OUT_DIR)
	mkdir -p $(OUT_DIR)
	rm -f $(WIN_BIN)
	cross build --target x86_64-pc-windows-gnu --release -p gui
	mv -f target/x86_64-pc-windows-gnu/release/gui.exe $(WIN_BIN)

gh-release:
	gh release create $(VERSION) $(OUT_DIR)/*

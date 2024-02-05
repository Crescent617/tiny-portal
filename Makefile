OUT_DIR = bin
VERSION = 0.1.1
BIN_NAME = tiny-portal-gui
WIN_BIN = $(OUT_DIR)/$(BIN_NAME)-x86_64-pc-windows-gnu.exe

build:
	cargo build --release -p gui

build-win:
	mkdir -p $(OUT_DIR)
	rm -f $(WIN_BIN)
	cross build --target x86_64-pc-windows-gnu --release -p gui
	mv -f target/x86_64-pc-windows-gnu/release/gui.exe $(WIN_BIN)

gh-release:
	gh release create $(VERSION) $(OUT_DIR)/*

all:
	cross build --release --target x86_64-unknown-linux-musl
	cross build --release --target arm-unknown-linux-musleabihf
	cross build --release --target aarch64-unknown-linux-musl

clean:
	cargo clean

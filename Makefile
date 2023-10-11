all:
	cross build --release --target aarch64-unknown-linux-musl
	cross build --release --target x86_64-unknown-linux-musl

clean:
	find . -name \*~ -delete
	cargo clean

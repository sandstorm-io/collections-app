SPK_DEPS=spk/server spk/script.js.gz spk/style.css.gz

.PHONY: dev clean

collections.spk: $(SPK_DEPS)
	spk pack collections.spk

clean:
	rm -rf spk tmp bin lib feature-key-vendor.spk

dev-deps: $(SPK_DEPS)

dev: $(SPK_DEPS)
	spk dev

spk/script.js.gz: package.json *.jsx
	@mkdir -p spk tmp
	npm run-script bundle
	npm run-script uglify
	gzip -c tmp/script-min.js > spk/script.js.gz

spk/style.css.gz: package.json style.scss
	@mkdir -p spk tmp
	npm run-script sass
	npm run-script postcss
	gzip -c tmp/style.css > spk/style.css.gz

target/release/server: src/
	cargo build --release

spk/server: target/release/server
	@mkdir -p spk
	cp target/release/server spk/server


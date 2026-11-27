SIM_SEED ?= 20261112
PROPTEST_CASES ?= 1000

.PHONY: ci fmt lint fuzz-check test doc bench deny python-test examples book clean

ci: fmt lint deny fuzz-check test doc bench python-test examples

fmt:
	cargo fmt --all -- --check
	cargo fmt --manifest-path fuzz/Cargo.toml --all -- --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

fuzz-check:
	CARGO_TARGET_DIR=$(CURDIR)/target cargo check --manifest-path fuzz/Cargo.toml --locked --bins

test:
	SIM_SEED=$(SIM_SEED) PROPTEST_CASES=$(PROPTEST_CASES) cargo test --workspace --all-features --locked --no-fail-fast

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

bench:
	cargo bench --workspace --locked -- --test

deny:
	cargo deny check

python-test:
	cargo build --locked -p verirun-cli --bin verirun
	python3 -m unittest discover -s python/tests -v

examples:
	cargo build --locked -p verirun-cli --bin verirun
	python3 examples/reproducible_eval_pipeline.py
	sh examples/run_and_verify.sh

book:
	bash book/sync-docs.sh
	mdbook build book
	rm -rf book/src/docs

clean:
	cargo clean
	rm -rf book/build book/src/docs

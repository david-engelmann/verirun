SIM_SEED ?= 20261112
PROPTEST_CASES ?= 1000

.PHONY: ci fmt lint test doc bench deny python-test clean

ci: fmt lint deny test doc bench python-test

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

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

clean:
	cargo clean

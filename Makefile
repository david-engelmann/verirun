SIM_SEED ?= 20261112
PROPTEST_CASES ?= 1000

.PHONY: ci fmt lint fuzz-check test doc bench deny audit hygiene python-test examples book clean

ci: hygiene fmt lint deny audit fuzz-check test doc bench python-test examples

hygiene:
	@for f in Cargo.toml Cargo.lock deny.toml CITATION.cff CODE_OF_CONDUCT.md Makefile .editorconfig install.sh README.md AGENTS.md CONTRIBUTING.md CHANGELOG.md SECURITY.md DECISIONS.md docs/ARCHITECTURE.md docs/INVARIANTS.md docs/FAQ.md docs/COMPARISON.md book/book.toml book/sync-docs.sh examples/README.md examples/reproducible_eval_pipeline.py examples/run_and_verify.sh fuzz/Cargo.toml fuzz/Cargo.lock crates/verirun-core/README.md crates/verirun-platform/README.md crates/verirun-store/README.md crates/verirun-cli/README.md rust-toolchain.toml .github/workflows/ci.yml .github/pull_request_template.md .github/ISSUE_TEMPLATE/bug_report.md .github/ISSUE_TEMPLATE/feature_request.md .github/ISSUE_TEMPLATE/config.yml; do \
		test -f "$$f" || { echo "missing required file: $$f"; exit 1; }; \
	done
	@python3 -c "import pathlib, sys; bad = [str(p) for p in pathlib.Path('.').rglob('*.md') if '.git' not in p.parts and (chr(0x2014) in p.read_text(encoding='utf-8') or chr(0x2013) in p.read_text(encoding='utf-8'))]; print('em dash or en dash found in:', bad) if bad else None; sys.exit(1 if bad else 0)"
	grep -E '^channel = "[0-9]+\.[0-9]+\.[0-9]+"' rust-toolchain.toml
	test "$$(grep -c '^channel' rust-toolchain.toml)" = "1"
	@python3 -c "import pathlib, re, sys, tomllib; e = tomllib.loads(pathlib.Path('pyproject.toml').read_text(encoding='utf-8'))['project']['version']; m = re.search(r'^__version__ = \"([^\"]+)\"', pathlib.Path('python/verirun/__init__.py').read_text(encoding='utf-8'), re.M); sys.exit('SDK __version__ does not match pyproject.toml') if not m or m.group(1) != e else None"
	@python3 -c "import pathlib, re, sys, tomllib; e = tomllib.loads(pathlib.Path('pyproject.toml').read_text(encoding='utf-8'))['project']['version']; m = re.search(r'^version: (\S+)$$', pathlib.Path('CITATION.cff').read_text(encoding='utf-8'), re.M); sys.exit('CITATION.cff version does not match pyproject.toml') if not m or m.group(1) != e else None"
	@python3 -c "import pathlib, sys, tomllib; e = tomllib.loads(pathlib.Path('pyproject.toml').read_text(encoding='utf-8'))['project']['version']; ms = ['crates/verirun-platform/Cargo.toml', 'crates/verirun-core/Cargo.toml', 'crates/verirun-store/Cargo.toml', 'crates/verirun-cli/Cargo.toml']; ds = [tomllib.loads(pathlib.Path(x).read_text(encoding='utf-8')) for x in ms]; ok = all(d['package']['version'] == e for d in ds) and all(v.get('version') == e for d in ds for k, v in d.get('dependencies', {}).items() if k.startswith('verirun-') and isinstance(v, dict)); print(f'hygiene ok, version parity at {e}') if ok else sys.exit('crate version parity check failed')"

audit:
	cargo audit

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
	cargo run --locked -p verirun-core --example canonical_and_digests
	cargo run --locked -p verirun-store --example store_roundtrip
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

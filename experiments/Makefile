PYTHON ?= python3

.PHONY: all verify-data artifacts

all: artifacts

verify-data:
	cd data && sha256sum -c SHA256SUMS
	sha256sum -c RAW_SHA256SUMS >/dev/null

artifacts: verify-data
	$(PYTHON) scripts/generate_all.py

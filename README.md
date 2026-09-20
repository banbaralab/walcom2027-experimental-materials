# CyBER experimental materials

This repository reproduces the experiments for the WALCOM 2027 paper on exact
BDD-based cycle-free-basin computation. It contains the evaluated CyBER source,
the BBM inputs and fixed-point targets, one master CSV, generation scripts, and
the raw logs for every reported run.

The repository was initialized from the final experimental snapshot. It does
not contain obsolete Basin12 configurations or unreported mechanisms. The six
public CyBER configurations correspond one-to-one to the six CyBER curves in
the paper; PyBoolNet 3.0.16 is the seventh curve.

## Quick check

```sh
cargo build --release --locked --manifest-path cyber/Cargo.toml
./cyber/target/release/cyber \
  -cyber benchmarks/BBM/model_003_01.csv \
  --cyber-config full \
  --cyber-order dependency \
  benchmarks/BBM/model_003.bnet
make -C experiments
```

The solver should report basin cardinality `337920`. The experiment target
verifies the master CSV checksum, regenerates the cactus plot and tables, and
checks the complete 7-by-218 matrix and all common cardinalities.

## Layout

- `cyber/`: minimal Rust source for source revision `0f8d662`
- `benchmarks/BBM/`: 230 BBM networks and 218 matching `_01.csv` targets
- `experiments/data/master_results.csv`: all 1,526 normalized observations
- `experiments/raw_logs/`: per-task solver and runsolver logs
- `experiments/execution_records/`: immutable imported JSONL records
- `experiments/scripts/`: figure, table, validation, and export scripts

All solver runs use one thread, 1,800-second CPU and wall limits, 64,000 MB,
and Laurel allocation `p=1:t=14:c=14:m=64000M`. See
[`experiments/README.md`](experiments/README.md) for execution identifiers and
outcome counts.

This artifact is released under the MIT License. Third-party licenses remain
with their respective projects; see `THIRD_PARTY.md`.

# Experimental master data

`master_results.csv` is the single source for every experimental figure and table
in the paper. It contains 1,526 observations: 218 BBM instances under the six
named CyBER configurations and PyBoolNet 3.0.16. `SHA256SUMS` records its
committed content hash.

## Common setup

- Laurel 3 at Kyoto University; one solver/instance per Slurm job
- allocation `p=1:t=14:c=14:m=64000M`; solver restricted to one thread
- CPU and wall limits of 1,800 seconds; memory limit of 64,000 MB
- 218 BBM Boolean networks and their `model_NNN_01.csv` fixed-point attractors

All CyBER configurations use source revision `0f8d662` and the same container
image. PyBoolNet uses the existing pinned 3.0.16 run. The CSV records execution
UUIDs, import digests, build fingerprints, scheduler jobs, partitions,
allocations, resource limits, and solver statistics. In particular, Full and
Complement GFP+Sink-SCC record the residual variable counts immediately before
and after sink-SCC elimination.

## Configurations

| `solver_config` | role |
|---|---|
| `cyber/full` | Complement GFP plus all three structural/BDD techniques |
| `cyber/forward-baseline` | forward fixed-point baseline |
| `cyber/complement-gfp` | complement-based greatest fixed point only |
| `cyber/complement-gfp-plus-immutable` | Complement GFP plus immutable propagation |
| `cyber/complement-gfp-plus-sink-scc` | Complement GFP plus sink-SCC elimination |
| `cyber/complement-gfp-plus-dependency-order` | Complement GFP plus dependency ordering |
| `pyboolnet/cycle-free-3.0.16` | external cycle-free-basin baseline |

The public CyBER command line exposes exactly these six configurations; Full
does not contain unreported component decomposition or target grouping.

## Generation and validation

The named scripts under `scripts/` read this CSV directly. Cactus curves sort
completed runs by CPU time. PAR-2 charges 3,600 seconds for timeout or OOM.
CPU and RSS factors are paired geometric means on common completions. Basin-size
breakdowns use the 199 instances completed by Full.

`make artifacts` verifies the hash and requires a complete 7-by-218 matrix. It
also rejects unexpected configurations, solved-set containment violations, and
any exact basin-cardinality disagreement on a common completion.

Maintainers can recreate the master file from the Bench Platform DuckDB import,
BBM corpus, and collected raw logs with `scripts/export_master_results.py`.
After intentional replacement, run `make checksums`, review the regenerated
artifacts, and commit the CSV, figures, and tables together.

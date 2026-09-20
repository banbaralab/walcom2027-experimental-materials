# CyBER

This is the minimal standalone source for the CyBER implementation evaluated in
the paper, extracted from bn4rust commit
`0f8d662e996ac2639263a37b1b5d434b4737eea4`. Legacy basin solvers and their
CUDD, SAPPOROBDD, and SAT dependencies are omitted. CyBER depends only on
Biodivine BDD and `num-bigint`.

Build with:

```sh
cargo build --release --locked
```

The public interface exposes exactly six fixed configurations:

```text
full
forward-baseline
complement-gfp
complement-gfp-plus-immutable
complement-gfp-plus-sink-scc
complement-gfp-plus-dependency-order
```

For example:

```sh
./target/release/cyber \
  -cyber ../benchmarks/BBM/model_003_01.csv \
  --cyber-config full \
  --cyber-order dependency \
  ../benchmarks/BBM/model_003.bnet
```

The two configurations that apply sink-SCC elimination report the residual
variable counts immediately before and after that reduction.

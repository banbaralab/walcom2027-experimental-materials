# Experiments

`data/master_results.csv` contains one row for each of 218 instances under the
following seven configurations:

| Configuration | Execution UUID | Completed | Timeout | OOM |
|---|---|---:|---:|---:|
| CyBER Full | `d2445532-9f87-471f-90a2-8fd5507132d8` | 199 | 19 | 0 |
| Forward Baseline | `ab16a8cf-7b73-4a8d-9bf5-4436049f7552` | 139 | 68 | 11 |
| Complement GFP | `36da421c-1044-4268-8546-4bb3791a2361` | 150 | 58 | 10 |
| Complement GFP+Immutable | `565847fc-58d5-4b2d-96ec-a17ffb69e644` | 159 | 58 | 1 |
| Complement GFP+Sink-SCC | `448595f2-3bca-485e-b63a-352e03687ede` | 182 | 33 | 3 |
| Complement GFP+Dependency Order | `2d332995-5da0-482e-9d34-18160428df39` | 169 | 49 | 0 |
| PyBoolNet 3.0.16 | `b18957d3-1af1-4ef1-9588-7e4ec8f121e6` | 151 | 67 | 0 |

The six CyBER runs share source revision `0f8d662`, image digest
`sha256:f7032f0331ea743d369d0f159211a1635d9b6d3ce7dc6e749199a796424965ad`,
and identical limits. PyBoolNet is the previously collected pinned 3.0.16 run
under the same limits and instance set.

Full contains exactly Complement GFP, immutable-variable propagation,
sink-SCC elimination, and dependency-aware ordering. It does not include
component decomposition or same-target grouping.

Full completes every instance completed by another configuration. Exact basin
cardinalities agree on all common completions. The raw `solver-output.log` for
Full and Complement GFP+Sink-SCC also contains
`sink_scc_variables_before` and `sink_scc_variables_after`.

Run `make` here to verify and regenerate the paper artifacts. Python 3.10 or
newer and Matplotlib are required. `RAW_SHA256SUMS` verifies all collected raw
files and immutable execution records.

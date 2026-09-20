#!/usr/bin/env python3
"""Generate node-count and basin-cardinality breakdown tables from the master CSV."""

import math

from result_data import GENERATED, load_runs


def completion_count(paths, observations) -> int:
    return sum(observations[p]["outcome"] == "completed" for p in paths)


def emphasize_best(value: int, best: int) -> str:
    return rf"\textbf{{{value}}}" if value == best else str(value)


def table_rows(paths, value, cuts, labels, all_on, pyboolnet) -> str:
    rows, lower = [], -1
    for upper, label in zip(cuts + [math.inf], labels):
        bucket = [path for path in paths if lower < value(path) <= upper]
        all_on_count = completion_count(bucket, all_on)
        pyboolnet_count = completion_count(bucket, pyboolnet)
        best = max(all_on_count, pyboolnet_count)
        rows.append(
            f"{label} & {len(bucket)} & {emphasize_best(all_on_count, best)} "
            f"& {emphasize_best(pyboolnet_count, best)} \\\\"
        )
        lower = upper
    rows.append(r"\bottomrule")
    return "\n".join(rows) + "\n"


def main() -> None:
    runs = load_runs()
    full, pyboolnet = runs["full"], runs["pyboolnet"]
    paths = sorted(full)
    nodes = table_rows(
        paths, lambda p: int(full[p]["node_count"]), [20, 40, 80],
        [r"$n\leq20$", r"$21\leq n\leq40$", r"$41\leq n\leq80$", r"$n\geq81$"],
        full, pyboolnet,
    )
    known = [p for p in paths if full[p]["basin"] is not None]
    basin = table_rows(
        known, lambda p: int(full[p]["basin"]).bit_length(), [10, 25, 50],
        [r"$|\mathcal{B}|<2^{10}$", r"$2^{10}\leq|\mathcal{B}|<2^{25}$",
         r"$2^{25}\leq|\mathcal{B}|<2^{50}$", r"$|\mathcal{B}|\geq2^{50}$"],
        full, pyboolnet,
    )
    GENERATED.mkdir(parents=True, exist_ok=True)
    (GENERATED / "breakdown_nodes.tex").write_text(nodes, encoding="utf-8")
    (GENERATED / "breakdown_basin.tex").write_text(basin, encoding="utf-8")


if __name__ == "__main__":
    main()

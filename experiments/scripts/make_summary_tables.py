#!/usr/bin/env python3
"""Generate ablation, solver-comparison, and representation summary TeX."""

from result_data import (
    GENERATED, completed, count, load_runs, paired_factor, par2,
    validate_cardinalities,
)


def main() -> None:
    runs = load_runs()
    full = runs["full"]
    full_solved = completed(full)
    for name, observations in runs.items():
        if not completed(observations) <= full_solved:
            raise RuntimeError(f"{name}: solved set is not contained in Full")
        validate_cardinalities(full, observations, name)

    labels = [
        ("full", "Full"), ("forward_baseline", "Forward Baseline"),
        ("complement_gfp", "Complement GFP"),
        ("gfp_plus_immutable", "GFP + immutable"),
        ("gfp_plus_sink_scc", "GFP + Sink-SCC"),
        ("gfp_plus_dependency_order", "GFP + dependency order"),
    ]
    heavy = {p for p in completed(full) if float(full[p]["cpu"]) >= 10.0}
    rows = []
    for name, label in labels:
        obs = runs[name]
        cpu = 1.0 if name == "full" else paired_factor(obs, full, "cpu", heavy)
        rss = 1.0 if name == "full" else paired_factor(obs, full, "rss")
        rows.append(
            f"{label} & {count(obs, 'completed')} & {count(obs, 'timeout')} "
            f"& {count(obs, 'oom')} & {par2(obs):.2f} & {cpu:.3f} & {rss:.3f} \\\\"
        )
    rows.append(r"\bottomrule")

    pyboolnet = runs["pyboolnet"]
    comparison = [
        f"\\cyber{{}} & {count(full, 'completed')} & {count(full, 'timeout')} "
        f"& {count(full, 'oom')} & {par2(full):.2f} & 1.000 & 1.000 " + r"\\",
        f"\\pbn{{}} & {count(pyboolnet, 'completed')} & {count(pyboolnet, 'timeout')} "
        f"& {count(pyboolnet, 'oom')} & {par2(pyboolnet):.2f} "
        f"& {paired_factor(pyboolnet, full, 'cpu'):.3f} "
        f"& {paired_factor(pyboolnet, full, 'rss'):.3f} " + r"\\",
        r"\bottomrule",
    ]

    GENERATED.mkdir(parents=True, exist_ok=True)
    (GENERATED / "ablation_rows.tex").write_text("\n".join(rows) + "\n", encoding="utf-8")
    (GENERATED / "comparison_rows.tex").write_text("\n".join(comparison) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

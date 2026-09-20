"""Shared loader and statistics for figures and tables derived from master_results.csv."""

from __future__ import annotations

import csv
import math
from pathlib import Path


PAPER = Path(__file__).resolve().parent.parent
MASTER = PAPER / "data" / "master_results.csv"
GENERATED = PAPER / "generated"
FIGURES = PAPER / "figures" / "generated"
TIME_LIMIT = 1800.0
PENALTY = 2.0 * TIME_LIMIT

ALIASES = {
    "cyber/full": "full",
    "pyboolnet/cycle-free-3.0.16": "pyboolnet",
    "cyber/forward-baseline": "forward_baseline",
    "cyber/complement-gfp": "complement_gfp",
    "cyber/complement-gfp-plus-immutable": "gfp_plus_immutable",
    "cyber/complement-gfp-plus-sink-scc": "gfp_plus_sink_scc",
    "cyber/complement-gfp-plus-dependency-order": "gfp_plus_dependency_order",
}


def _number(value: str, conversion):
    return conversion(value) if value else None


def load_runs() -> dict[str, dict[str, dict[str, object]]]:
    """Load and validate the complete 7-configuration by 218-instance matrix."""
    runs: dict[str, dict[str, dict[str, object]]] = {}
    with MASTER.open(encoding="utf-8", newline="") as handle:
        for raw in csv.DictReader(handle):
            config = ALIASES.get(raw["solver_config"])
            if config is None:
                raise RuntimeError(f"unknown solver_config: {raw['solver_config']}")
            if raw["result"] not in {"completed", "timeout", "oom"}:
                raise RuntimeError(f"invalid result: {raw['result']}")
            observations = runs.setdefault(config, {})
            if raw["problem"] in observations:
                raise RuntimeError(f"duplicate observation: {config}/{raw['problem']}")
            observations[raw["problem"]] = {
                "node_count": int(raw["node_count"]),
                "outcome": raw["result"],
                "cpu": _number(raw["cpu_time_sec"], float),
                "wall": _number(raw["wall_time_sec"], float),
                "rss": _number(raw["max_rss_kib"], int),
                "basin": _number(raw["basin_size"], int),
                "sink_before": _number(raw["sink_scc_variables_before"], int),
                "sink_after": _number(raw["sink_scc_variables_after"], int),
            }
    if set(runs) != set(ALIASES.values()):
        raise RuntimeError("master CSV does not contain exactly the documented configurations")
    reference = set(next(iter(runs.values())))
    if len(reference) != 218:
        raise RuntimeError(f"expected 218 instances, got {len(reference)}")
    reference_name = next(iter(runs))
    reference_rows = runs[reference_name]
    for config, observations in runs.items():
        if set(observations) != reference:
            raise RuntimeError(f"{config}: instance set differs from the reference")
        for problem, row in observations.items():
            if row["node_count"] != reference_rows[problem]["node_count"]:
                raise RuntimeError(f"{config}/{problem}: inconsistent node count")
            required = ("cpu", "rss", "basin")
            if row["outcome"] == "completed" and any(
                row[field] is None for field in required
            ):
                raise RuntimeError(f"{config}/{problem}: incomplete completed result")
    return runs


def completed(observations):
    return {path for path, row in observations.items() if row["outcome"] == "completed"}


def count(observations, status: str) -> int:
    return sum(row["outcome"] == status for row in observations.values())


def par2(observations) -> float:
    return sum(
        float(row["cpu"]) if row["outcome"] == "completed" else PENALTY
        for row in observations.values()
    ) / len(observations)


def geometric_mean(values: list[float]) -> float:
    if not values or any(value <= 0 for value in values):
        raise RuntimeError("geometric mean requires positive observations")
    return math.exp(sum(math.log(value) for value in values) / len(values))


def paired_factor(numerator, denominator, field: str, paths=None) -> float:
    common = completed(numerator) & completed(denominator)
    if paths is not None:
        common &= paths
    ratios = [
        float(numerator[path][field]) / float(denominator[path][field])
        for path in common
        if numerator[path][field] is not None and denominator[path][field] is not None
    ]
    return geometric_mean(ratios)


def validate_cardinalities(reference, candidate, label: str) -> None:
    for path in completed(reference) & completed(candidate):
        if reference[path]["basin"] != candidate[path]["basin"]:
            raise RuntimeError(f"{label}/{path}: basin cardinality mismatch")


def cactus_times(observations) -> list[float]:
    return sorted(
        max(float(row["cpu"]), 0.001)
        for row in observations.values()
        if row["outcome"] == "completed" and row["cpu"] is not None
    )


def configure_matplotlib() -> None:
    import matplotlib as mpl

    mpl.rcParams.update({
        "font.family": "serif",
        "font.size": 8,
        "axes.labelsize": 8,
        "legend.fontsize": 7,
        "xtick.labelsize": 7,
        "ytick.labelsize": 7,
        "pdf.fonttype": 42,
    })


def save_pdf(figure, path: Path) -> None:
    """Write a reproducible PDF without clock-dependent metadata."""
    figure.savefig(
        path,
        bbox_inches="tight",
        metadata={"CreationDate": None, "ModDate": None},
    )

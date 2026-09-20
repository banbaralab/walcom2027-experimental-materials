#!/usr/bin/env python3
"""Export all paper experiments into one wide, immutable master CSV.

This maintainer-only script reads the original bench-platform DuckDB imports and
Kat execution JSONL files.  Plot/table scripts never need those private paths;
they consume only ``data/master_results.csv``.
"""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path
from typing import Any

import duckdb


FINAL_RUNS = {
    # solver_config: (execution UUID, import SHA-256, source revision)
    "cyber/full": (
        "d2445532-9f87-471f-90a2-8fd5507132d8",
        "f17ce4d8669280a7d5e535ef8777072cb75d85ea54f84a5e1ac35a687afd80ca",
        "0f8d662",
    ),
    "pyboolnet/cycle-free-3.0.16": (
        "b18957d3-1af1-4ef1-9588-7e4ec8f121e6",
        "b95930007c6d9ce7a4c660ceaac387a2e353d06eb9d4a7bb7f31e7fee5fed38f",
        "3.0.16",
    ),
    "cyber/forward-baseline": (
        "ab16a8cf-7b73-4a8d-9bf5-4436049f7552",
        "0d624a202f6e2ea15cd4ff4fca03a26924e7bf5685c404a0cc892c08223dd699",
        "0f8d662",
    ),
    "cyber/complement-gfp": (
        "36da421c-1044-4268-8546-4bb3791a2361",
        "5ecb911901f39d46f9f74128a82a651fb871eef5f9235aec32947c952c1f681e",
        "0f8d662",
    ),
    "cyber/complement-gfp-plus-immutable": (
        "565847fc-58d5-4b2d-96ec-a17ffb69e644",
        "bceed70b9d3ec67ed89cb5c35b49b90624fd66315ab7d564fdbfcdcc9c3c820c",
        "0f8d662",
    ),
    "cyber/complement-gfp-plus-sink-scc": (
        "448595f2-3bca-485e-b63a-352e03687ede",
        "73f3bf88e9e5d9b2e1d17aee903c42eb9f1185789321f0491a17382462775d34",
        "0f8d662",
    ),
    "cyber/complement-gfp-plus-dependency-order": (
        "2d332995-5da0-482e-9d34-18160428df39",
        "82ddc6bcb701372e009e32471d3d97e1913c5201cc586ce3b5ec9da8cfd0d784",
        "0f8d662",
    ),
}

FIELDS = [
    # The first four columns are the stable, human-facing core requested for analysis.
    "problem",
    "solver_config",
    "result",
    "cpu_time_sec",
    # Timing and runsolver resource statistics.
    "wall_time_sec",
    "user_time_sec",
    "system_time_sec",
    "cpu_usage_percent",
    "max_rss_kib",
    "max_vm_kib",
    "max_mm_kib",
    "exit_status",
    "container_exit_code",
    "timeout",
    "memout",
    # Problem and solver result statistics.
    "node_count",
    "basin_size",
    "bdd_iterations",
    "bdd_basin_nodes",
    "bdd_peak_nodes",
    "bdd_gc_runs",
    "active_rules",
    "skipped_rules",
    "retained_variables",
    "pruned_variables",
    "sink_scc_variables_before",
    "sink_scc_variables_after",
    "ternary_recursive_calls",
    "ternary_cache_hits",
    "ternary_cache_misses",
    "variable_order",
    "algorithm",
    # Experimental provenance.
    "experiment",
    "source_revision",
    "execution_uuid",
    "import_sha256",
    "benchmark_sha256",
    "build_fingerprint",
    "observed_at",
    "scheduler_job_id",
    "cluster",
    "partition",
    "allocation",
    "cpu_limit_sec",
    "wall_limit_sec",
    "memory_limit_mb",
]


def count_nodes(path: Path) -> int:
    return sum(
        bool(line.strip())
        and not line.lstrip().startswith("#")
        and not line.lstrip().startswith("targets,")
        for line in path.read_text(encoding="utf-8").splitlines()
    )


def as_object(value: Any) -> dict[str, Any]:
    if isinstance(value, dict):
        return value
    if not value:
        return {}
    return json.loads(value)


def normalized_row(
    raw: dict[str, Any],
    *,
    solver_config: str,
    experiment: str,
    source_revision: str,
    execution_uuid: str,
    import_sha256: str,
    node_count: int,
    solver_output: dict[str, Any] | None = None,
) -> dict[str, Any]:
    message = as_object(raw.get("row_message"))
    runsolver = as_object(message.get("runsolver"))
    metrics = as_object(raw.get("metrics"))
    solver_output = solver_output or {}
    metrics.update(solver_output)
    row = {field: "" for field in FIELDS}
    row.update(
        {
            "problem": raw["instance_relpath"],
            "solver_config": solver_config,
            "result": raw["result_status"],
            "cpu_time_sec": raw.get("cpu_sec", ""),
            "wall_time_sec": raw.get("wall_sec", ""),
            "user_time_sec": runsolver.get("USERTIME", ""),
            "system_time_sec": runsolver.get("SYSTEMTIME", ""),
            "cpu_usage_percent": runsolver.get("CPUUSAGE", ""),
            "max_rss_kib": raw.get("max_rss_kib", ""),
            "max_vm_kib": runsolver.get("MAXVM", ""),
            "max_mm_kib": runsolver.get("MAXMM", ""),
            "exit_status": runsolver.get("EXITSTATUS", ""),
            "container_exit_code": message.get("container_exit_code", ""),
            "timeout": str(bool(raw.get("timeout", False))).lower(),
            "memout": str(bool(raw.get("memout", False))).lower(),
            "node_count": node_count,
            "basin_size": metrics.get("basin_size", ""),
            "bdd_iterations": metrics.get("bdd_iterations", ""),
            "bdd_basin_nodes": metrics.get("bdd_basin_nodes", ""),
            "bdd_peak_nodes": metrics.get("bdd_peak_nodes", ""),
            "bdd_gc_runs": metrics.get("bdd_gc_runs", ""),
            "active_rules": metrics.get("active_rules", ""),
            "skipped_rules": metrics.get("skipped_rules", ""),
            "retained_variables": metrics.get("retained_variables", ""),
            "pruned_variables": metrics.get("pruned_variables", ""),
            "sink_scc_variables_before": metrics.get("sink_scc_variables_before", ""),
            "sink_scc_variables_after": metrics.get("sink_scc_variables_after", ""),
            "ternary_recursive_calls": metrics.get("ternary_recursive_calls", ""),
            "ternary_cache_hits": metrics.get("ternary_cache_hits", ""),
            "ternary_cache_misses": metrics.get("ternary_cache_misses", ""),
            "variable_order": metrics.get("variable_order", message.get("variable_order", "")),
            "algorithm": metrics.get("algorithm", message.get("algorithm", "")),
            "experiment": experiment,
            "source_revision": source_revision,
            "execution_uuid": execution_uuid,
            "import_sha256": import_sha256,
            "benchmark_sha256": raw.get("benchmark_sha256", ""),
            "build_fingerprint": raw.get("build_fingerprint", ""),
            "observed_at": raw.get("observed_at", ""),
            "scheduler_job_id": raw.get("scheduler_job_id", ""),
            "cluster": raw.get("cluster", ""),
            "partition": raw.get("partition", ""),
            "allocation": raw.get("allocation", ""),
            "cpu_limit_sec": raw.get("cpu_limit_sec", ""),
            "wall_limit_sec": raw.get("wall_limit_sec", ""),
            "memory_limit_mb": raw.get("memory_limit_mb", ""),
        }
    )
    return row


def load_solver_outputs(raw_log_root: Path, execution_uuid: str) -> dict[str, dict[str, Any]]:
    execution = raw_log_root / f"execution:uuid:{execution_uuid}"
    task_root = execution / "laurel-partial"
    if not task_root.is_dir():
        task_root = execution / "tasks"
    outputs: dict[str, dict[str, Any]] = {}
    for task_dir in task_root.glob("task-*"):
        task_file = task_dir / "task.json"
        output_file = task_dir / "solver-output.log"
        if not task_file.is_file() or not output_file.is_file():
            continue
        task = json.loads(task_file.read_text(encoding="utf-8"))
        problem = f"BBM/{Path(task['instance_path']).name}"
        lines = output_file.read_text(encoding="utf-8", errors="replace").splitlines()
        for line in reversed(lines):
            try:
                outputs[problem] = json.loads(line.split("\t", 1)[-1])
                break
            except json.JSONDecodeError:
                continue
    return outputs


def export_final_runs(
    database: Path, benchmark_root: Path, raw_log_root: Path
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with duckdb.connect(str(database), read_only=True) as connection:
        for solver_config, (execution_uuid, digest, revision) in FINAL_RUNS.items():
            solver_outputs = load_solver_outputs(raw_log_root, execution_uuid)
            import_id = f"import:sha256:{digest}"
            observations = connection.execute(
                """
                SELECT raw_json
                FROM run_observations
                WHERE import_id = ?
                ORDER BY json_extract_string(raw_json, '$.instance_relpath')
                """,
                [import_id],
            ).fetchall()
            if len(observations) != 218:
                raise RuntimeError(
                    f"{solver_config}: expected 218 rows, got {len(observations)}"
                )
            for (payload,) in observations:
                raw = as_object(payload)
                problem = raw["instance_relpath"]
                rows.append(
                    normalized_row(
                        raw,
                        solver_config=solver_config,
                        experiment="final-comparison-and-ablation",
                        source_revision=revision,
                        execution_uuid=execution_uuid,
                        import_sha256=digest,
                        node_count=count_nodes(benchmark_root / problem),
                        solver_output=solver_outputs.get(problem),
                    )
                )
    return rows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--benchmark-root", type=Path, required=True)
    parser.add_argument("--raw-log-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("data/master_results.csv"))
    args = parser.parse_args()
    rows = export_final_runs(args.database, args.benchmark_root, args.raw_log_root)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=FIELDS, lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    if len(rows) != 7 * 218:
        raise RuntimeError(f"expected 1,526 data rows, got {len(rows)}")


if __name__ == "__main__":
    main()

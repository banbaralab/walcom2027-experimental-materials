#!/usr/bin/env python3
"""Plot the forward and complement baselines, GFP-plus runs, PyBoolNet, and full CyBER."""

from result_data import (
    FIGURES, TIME_LIMIT, cactus_times, configure_matplotlib, load_runs, save_pdf,
)


def main() -> None:
    configure_matplotlib()
    import matplotlib.pyplot as plt

    plt.rcParams.update({
        "font.size": 10,
        "axes.labelsize": 10,
        "legend.fontsize": 10,
        "xtick.labelsize": 10,
        "ytick.labelsize": 10,
    })

    runs = load_runs()
    series = [
        ("full", "CyBER Full", "black", "-", 1.8),
        ("gfp_plus_sink_scc", "Complement GFP+Sink-SCC", "#d55e00", "-.", 1.2),
        ("gfp_plus_dependency_order", "Complement GFP+Dependency Order", "#009e73", (0, (4, 2)), 1.2),
        ("pyboolnet", "PyBoolNet", "#0072b2", "-", 1.5),
        ("gfp_plus_immutable", "Complement GFP+Immutable", "#e69f00", "--", 1.2),
        ("complement_gfp", "Complement GFP", "#7a3e9d", (0, (1, 1)), 1.2),
        ("forward_baseline", "Forward Baseline", "0.45", ":", 1.2),
    ]
    series.sort(key=lambda item: len(cactus_times(runs[item[0]])), reverse=True)

    fig, axis = plt.subplots(figsize=(7.0, 3.15))
    for config, label, color, style, width in series:
        times = cactus_times(runs[config])
        axis.plot(
            range(1, len(times) + 1),
            times,
            label=f"{label} ({len(times)})",
            color=color,
            linestyle=style,
            linewidth=width,
        )

    axis.axhline(TIME_LIMIT, color="0.55", linewidth=0.7, linestyle="--")
    axis.set(
        xlim=(0, 218),
        ylim=(0, 1850),
        xlabel="Number of solved instances",
        ylabel="CPU time (s)",
    )
    axis.grid(True, which="major", color="0.88", linewidth=0.5)
    axis.legend(loc="upper left", bbox_to_anchor=(0, 0.875), ncol=1, frameon=False)
    fig.tight_layout(pad=0.5)

    FIGURES.mkdir(parents=True, exist_ok=True)
    save_pdf(fig, FIGURES / "combined_cactus.pdf")
    plt.close(fig)


if __name__ == "__main__":
    main()

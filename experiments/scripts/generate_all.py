#!/usr/bin/env python3
"""Regenerate every paper figure and table from data/master_results.csv."""

import make_breakdown_tables
import make_summary_tables
import plot_combined_cactus


def main() -> None:
    make_summary_tables.main()
    make_breakdown_tables.main()
    plot_combined_cactus.main()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Offline fitness metric for grok-search-cli.

Prints exactly one number. Lower is better.
"""

from __future__ import annotations

import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target" / "release" / "grok-search-cli"
QUERY = "recent X community evidence about Grok 4.20 multi-agent console"
PENALTY = 1_000_000.0


def run(cmd: list[str], *, timeout: int = 60, capture: bool = True) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env.setdefault("NO_COLOR", "1")
    return subprocess.run(
        cmd,
        cwd=ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE if capture else subprocess.DEVNULL,
        timeout=timeout,
        check=False,
    )


def checked(cmd: list[str], *, timeout: int = 60, capture: bool = True) -> subprocess.CompletedProcess[str]:
    proc = run(cmd, timeout=timeout, capture=capture)
    if proc.returncode != 0:
        raise RuntimeError(f"{cmd!r} failed: {proc.stderr[-500:]}")
    return proc


def median_ms(cmd: list[str], repeats: int = 7) -> float:
    samples: list[float] = []
    for _ in range(repeats):
        start = time.perf_counter()
        proc = checked(cmd, timeout=30)
        elapsed = (time.perf_counter() - start) * 1000.0
        if proc.stdout:
            proc.stdout.encode()
        samples.append(elapsed)
    return statistics.median(samples)


def validate_plan() -> None:
    proc = checked([str(BIN), "plan", QUERY, "--format", "json"], timeout=30)
    data = json.loads(proc.stdout)
    assert data["recommended_surface"] == "x_search_surface"
    assert data["time_sensitivity"] == "realtime_or_recent"
    assert data["commands"][0]["args"][0] == "search"
    assert "--x-search" in data["commands"][0]["args"]


def validate_config_shape() -> None:
    proc = checked([str(BIN), "config", "show", "--format", "json"], timeout=30)
    data = json.loads(proc.stdout)
    assert "grok_model" in data
    assert "grok_api_key" in data
    assert data["grok_api_key"] == "not configured" or "***" in data["grok_api_key"]


def main() -> int:
    try:
        checked(["cargo", "test", "--quiet"], timeout=120, capture=True)
        checked(["cargo", "build", "--release", "--quiet"], timeout=180, capture=True)
        validate_plan()
        validate_config_shape()
        plan_ms = median_ms([str(BIN), "plan", QUERY, "--format", "json"])
        config_ms = median_ms([str(BIN), "config", "show", "--format", "json"])
        binary_mib = BIN.stat().st_size / (1024 * 1024)
        score = plan_ms + config_ms + binary_mib * 50.0
        print(f"{score:.3f}")
        return 0
    except Exception as exc:
        print(f"{PENALTY:.3f}")
        print(f"measure failed: {exc}", file=sys.stderr)
        return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Time datadiff against Graphtage on growing JSON inputs.

Each input is an array of objects with seven fields, one of them nested. The
new file is the old one shuffled, with 1% of the email fields changed, so a
correct diff reports exactly that many modifications. datadiff matches the
array by `--key id`; Graphtage runs with its defaults and has to find the
matching on its own, which is the harder problem it is built to solve.

Usage:
    cargo build --release
    pip install graphtage            # or: uv pip install graphtage
    python3 bench/compare_graphtage.py

Options:
    --datadiff PATH    datadiff binary (default: target/release/datadiff)
    --graphtage PATH   graphtage executable (default: graphtage on PATH)
    --timeout SECONDS  per-run limit for Graphtage (default: 120)
    --sizes N,N,...    array lengths (default: 100,250,500,1000,10000,100000)

Graphtage is skipped for sizes above 1000 and for every size after its first
timeout. datadiff times are the median of three runs.
"""

import argparse
import json
import os
import random
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

GRAPHTAGE_MAX_SIZE = 1000


def make_inputs(n, directory, rng):
    old = [
        {
            "id": i,
            "name": f"user{i}",
            "email": f"user{i}@example.com",
            "active": i % 3 == 0,
            "tags": [f"t{i % 7}", f"t{i % 11}"],
            "address": {"city": f"city{i % 50}", "zip": f"{10000 + i}"},
        }
        for i in range(n)
    ]
    new = [dict(o, address=dict(o["address"]), tags=list(o["tags"])) for o in old]
    changed = max(1, n // 100)
    for o in rng.sample(new, changed):
        o["email"] = o["email"].replace("example", "example2")
    rng.shuffle(new)
    old_path = os.path.join(directory, f"old-{n}.json")
    new_path = os.path.join(directory, f"new-{n}.json")
    with open(old_path, "w") as f:
        json.dump(old, f)
    with open(new_path, "w") as f:
        json.dump(new, f)
    return old_path, new_path, changed


def timed(cmd, timeout):
    start = time.perf_counter()
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None
    return time.perf_counter() - start


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--datadiff", default="target/release/datadiff")
    parser.add_argument("--graphtage", default="graphtage")
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--sizes", default="100,250,500,1000,10000,100000")
    args = parser.parse_args()

    if not os.path.exists(args.datadiff):
        sys.exit(f"no datadiff binary at {args.datadiff}; run `cargo build --release` first")
    graphtage = shutil.which(args.graphtage)
    if graphtage is None:
        print(f"note: {args.graphtage} not found, timing datadiff only\n")

    rng = random.Random(42)
    graphtage_gave_up = graphtage is None
    print(f"{'objects':>8} {'size':>9} {'datadiff':>10} {'graphtage':>14}")
    with tempfile.TemporaryDirectory() as tmp:
        for n in (int(s) for s in args.sizes.split(",")):
            old, new, changed = make_inputs(n, tmp, rng)
            size_kb = os.path.getsize(old) / 1024

            dd_cmd = [args.datadiff, old, new, "--key", "id", "--no-color"]
            summary = subprocess.run(dd_cmd, capture_output=True, text=True).stdout.splitlines()[-1]
            if not summary.startswith(f"{changed} change"):
                sys.exit(f"datadiff reported '{summary}' for {n} objects, expected {changed} changes")
            dd = statistics.median(timed(dd_cmd, None) for _ in range(3))

            if graphtage_gave_up or n > GRAPHTAGE_MAX_SIZE:
                gt_text = "skipped"
            else:
                gt = timed([graphtage, old, new], args.timeout)
                if gt is None:
                    graphtage_gave_up = True
                    gt_text = f">{args.timeout:.0f} s"
                else:
                    gt_text = f"{gt:.2f} s"

            print(f"{n:>8} {size_kb:>6.0f} KB {dd:>8.3f} s {gt_text:>14}", flush=True)


if __name__ == "__main__":
    main()

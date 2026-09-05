"""Point the Homebrew formula and the Scoop manifest at a new release.

Reads the version from argv and a {filename: sha256} map from CHECKSUMS, so a
checksum is always written next to the URL it actually belongs to rather than
by line order.
"""

import json
import os
import re
import sys

VERSION = sys.argv[1]
SUMS = json.loads(os.environ["CHECKSUMS"])
VERSION_IN_NAME = re.compile(r"v\d+\.\d+\.\d+")


def sha_for(url: str) -> str:
    name = url.rsplit("/", 1)[-1]
    try:
        return SUMS[name]
    except KeyError:
        sys.exit(f"no checksum collected for {name}")


def bump_formula(path: str) -> None:
    out, pending = [], None
    for line in open(path).read().split("\n"):
        url = re.search(r'url "([^"]+)"', line)
        if url:
            new = VERSION_IN_NAME.sub(f"v{VERSION}", url.group(1))
            line, pending = line.replace(url.group(1), new), new
        elif pending and re.search(r'sha256 "', line):
            line = re.sub(r'sha256 "[0-9a-f]{64}"', f'sha256 "{sha_for(pending)}"', line)
            pending = None
        elif re.match(r'\s*version "', line):
            line = re.sub(r'version "[^"]+"', f'version "{VERSION}"', line)
        out.append(line)
    open(path, "w").write("\n".join(out))


def bump_manifest(path: str) -> None:
    m = json.load(open(path))
    m["version"] = VERSION
    for arch in m["architecture"].values():
        arch["url"] = VERSION_IN_NAME.sub(f"v{VERSION}", arch["url"])
        arch["hash"] = sha_for(arch["url"])
    with open(path, "w") as f:
        json.dump(m, f, indent=4)
        f.write("\n")


if __name__ == "__main__":
    target = sys.argv[2]
    (bump_formula if target.endswith(".rb") else bump_manifest)(target)
    print(f"{target} -> {VERSION}")

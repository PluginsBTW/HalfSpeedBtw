#!/usr/bin/env python3
"""Writes the third-party license notices for a binary release of HalfSpeedBtw.

Covers every crate compiled into the plugin (normal dependencies of `halfspeedbtw` on
x86-64 Linux, without the optional standalone feature). Identical license texts are printed
once with the list of crates they apply to. For crates whose package ships no license file,
a notice is generated from the declared SPDX license and the crate's authors.

Usage: scripts/third_party_licenses.py OUTPUT_FILE
"""

import glob
import json
import os
import subprocess
import sys
from collections import OrderedDict

ROOT_PACKAGE = "halfspeedbtw"
TARGET = "x86_64-unknown-linux-gnu"
LICENSE_PREFIXES = ("LICEN", "COPYING", "NOTICE")

MIT_TEMPLATE = """MIT License

Copyright (c) {holders}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

ISC_TEMPLATE = """ISC License

Copyright (c) {holders}

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
"""


def shipped_packages(metadata):
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root = next(p["id"] for p in metadata["packages"] if p["name"] == ROOT_PACKAGE)
    seen, stack = set(), [root]
    while stack:
        package_id = stack.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        for dep in nodes[package_id]["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    workspace = set(metadata["workspace_members"])
    packages = [p for p in metadata["packages"] if p["id"] in seen and p["id"] not in workspace]
    return sorted(packages, key=lambda p: (p["name"], p["version"]))


def license_files(package):
    directory = os.path.dirname(package["manifest_path"])
    names = sorted(
        name for name in os.listdir(directory) if name.upper().startswith(LICENSE_PREFIXES)
    )
    paths = [os.path.join(directory, name) for name in names]
    if package.get("license_file"):
        paths.append(os.path.join(directory, package["license_file"]))
    # Bundled fonts carry their own licenses next to the font files.
    if package["name"] == "epaint_default_fonts":
        paths += sorted(glob.glob(os.path.join(directory, "fonts", "*.txt")))
    return list(OrderedDict.fromkeys(p for p in paths if os.path.isfile(p)))


def generated_notice(package, apache_text):
    """A notice for a crate that ships no license file, from its SPDX expression."""
    holders = ", ".join(package["authors"]) or f"the {package['name']} authors"
    spdx = package.get("license") or ""
    # For "A OR B" choices, take the first permissive option we have a text for.
    for option in (part.strip(" ()") for part in spdx.replace(" AND ", " OR ").split(" OR ")):
        if option == "MIT":
            return MIT_TEMPLATE.format(holders=holders)
        if option == "ISC":
            return ISC_TEMPLATE.format(holders=holders)
        if option == "Apache-2.0":
            return apache_text
    raise SystemExit(f"No license text for {package['name']} ({spdx})")


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)

    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET]
        )
    )
    packages = shipped_packages(metadata)

    apache_text = None
    for package in packages:
        for path in license_files(package):
            text = open(path, encoding="utf-8", errors="replace").read()
            if "Apache License" in text and "Version 2.0" in text and len(text) > 9000:
                apache_text = text
                break
        if apache_text:
            break
    if apache_text is None:
        raise SystemExit("Could not find an Apache-2.0 license text to reuse")

    # Group crates by identical license text.
    texts = OrderedDict()
    for package in packages:
        label = f"{package['name']} {package['version']}"
        paths = license_files(package)
        if paths:
            blobs = [open(p, encoding="utf-8", errors="replace").read().strip() for p in paths]
        else:
            blobs = [generated_notice(package, apache_text).strip()]
        for blob in blobs:
            texts.setdefault(blob, []).append(label)

    with open(sys.argv[1], "w", encoding="utf-8") as out:
        out.write("HalfSpeedBtw: third-party software notices\n")
        out.write("=" * 43 + "\n\n")
        out.write(
            "HalfSpeedBtw is licensed under GPL-3.0-or-later (see LICENSE). The binary\n"
            "includes the following third-party crates, used under the licenses below.\n"
            "Where a crate offers a choice of licenses, it is used under a permissive one.\n\n"
        )
        out.write("Crates and their declared licenses\n")
        out.write("-" * 34 + "\n\n")
        for package in packages:
            out.write(f"{package['name']} {package['version']}: {package.get('license')}\n")
        out.write("\n\nLicense texts\n")
        out.write("-" * 13 + "\n")
        for blob, labels in texts.items():
            out.write("\n" + "=" * 78 + "\n")
            out.write("Applies to: " + ", ".join(labels) + "\n")
            out.write("=" * 78 + "\n\n")
            out.write(blob + "\n")

    print(f"{len(packages)} crates, {len(texts)} distinct license texts -> {sys.argv[1]}")


if __name__ == "__main__":
    main()

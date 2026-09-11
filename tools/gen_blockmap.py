#!/usr/bin/env python3
"""Generate AppxBlockMap.xml for MSIX package.

MSIX requires AppxBlockMap.xml containing SHA256 hashes of each 64KB block
of every file in the package.

Usage (from repo root):
    python tools/gen_blockmap.py
"""

import hashlib
import os
import base64
import xml.etree.ElementTree as ET

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STAGING = os.path.join(REPO_ROOT, "target", "msix", "msix-staging")
BLOCK_SIZE = 64 * 1024  # 64 KB


def file_blocks(filepath):
    """Yield (block_hash_b64, block_size) for each 64KB block of a file."""
    sha = hashlib.sha256()
    with open(filepath, "rb") as f:
        while True:
            data = f.read(BLOCK_SIZE)
            if not data:
                break
            block_hash = hashlib.sha256(data).digest()
            yield base64.b64encode(block_hash).decode("ascii"), len(data)


def main():
    ns = "http://schemas.microsoft.com/appx/manifest/blockmap/2017"
    root = ET.Element("BlockMap", xmlns=ns)

    for dirpath, dirnames, filenames in os.walk(STAGING):
        dirnames.sort()
        for fname in sorted(filenames):
            full = os.path.join(dirpath, fname)
            rel = os.path.relpath(full, STAGING).replace("\\", "/")

            # Skip the block map itself and signature
            if rel in ("AppxBlockMap.xml", "AppxSignature.p7x"):
                continue

            file_elem = ET.SubElement(root, "File", Name=rel)
            for block_hash, block_size in file_blocks(full):
                block_elem = ET.SubElement(file_elem, "Block",
                                           BlockHash=block_hash)
                if block_size != BLOCK_SIZE:
                    block_elem.set("Size", str(block_size))

    tree = ET.ElementTree(root)
    ET.indent(tree, space="  ")
    out_path = os.path.join(STAGING, "AppxBlockMap.xml")
    tree.write(out_path, encoding="utf-8", xml_declaration=True)

    # Count files and blocks
    file_count = len(root.findall("File"))
    block_count = sum(len(f.findall("Block")) for f in root.findall("File"))
    print(f"Generated: {out_path}")
    print(f"Files: {file_count}, Blocks: {block_count}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Rebuild MSIX with correct path separators and no BOM."""

import zipfile
import os

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(REPO_ROOT, "target", "msix", "msix-staging")
DST = os.path.join(REPO_ROOT, "target", "msix", "danqing-pomodoro-free-v0.2.0-x64.msix")

# Remove old file
if os.path.exists(DST):
    os.remove(DST)

# Fix XML files: remove BOM
for xml_name in ["AppxManifest.xml", "[Content_Types].xml"]:
    xml_path = os.path.join(SRC, xml_name)
    with open(xml_path, "rb") as f:
        data = f.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
        with open(xml_path, "wb") as f:
            f.write(data)
        print(f"Fixed BOM: {xml_name}")
    else:
        print(f"No BOM: {xml_name}")

# Fix manifest: use forward slashes for asset paths (MSIX standard)
manifest_path = os.path.join(SRC, "AppxManifest.xml")
with open(manifest_path, "r", encoding="utf-8") as f:
    manifest = f.read()
manifest = manifest.replace("Assets\\", "Assets/")
manifest = manifest.replace("assets\\", "assets/")
with open(manifest_path, "w", encoding="utf-8") as f:
    f.write(manifest)
print("Fixed manifest paths to forward slashes")

# Create MSIX with forward slash paths (MSIX standard)
with zipfile.ZipFile(DST, "w", zipfile.ZIP_DEFLATED) as zf:
    for root, dirs, files in os.walk(SRC):
        for f in files:
            full = os.path.join(root, f)
            arcname = os.path.relpath(full, SRC)
            # Use forward slashes (MSIX spec allows both, but forward is standard)
            arcname = arcname.replace("\\", "/")
            zf.write(full, arcname)
            print(f"  + {arcname}")

size = os.path.getsize(DST)
print(f"\nMSIX: {DST}")
print(f"Size: {size:,} bytes ({size / 1024 / 1024:.1f} MB)")

#!/usr/bin/env python3
"""Generate Microsoft Store asset images from the pomodoro logo.

Usage (from repo root):
    python tools/gen_store_assets.py

Reads assets/logo/pomodoro_256.png and produces:
    target/msix/assets/StoreLogo.png        (50x50)
    target/msix/assets/Square44x44Logo.png   (44x44)
    target/msix/assets/Square150x150Logo.png (150x150)
    target/msix/assets/Wide310x150Logo.png   (310x150, logo centered)
    target/msix/assets/SplashScreen.png      (620x300, logo centered)
"""

import os
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    print("ERROR: Pillow not installed. Run: pip install Pillow")
    sys.exit(1)

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC_LOGO = REPO_ROOT / "assets" / "logo" / "pomodoro_256.png"
OUT_DIR = REPO_ROOT / "target" / "msix" / "assets"

# Store-required asset sizes
ASSETS = {
    "StoreLogo.png": (50, 50),
    "Square44x44Logo.png": (44, 44),
    "Square150x150Logo.png": (150, 150),
    "Wide310x150Logo.png": (310, 150),
    "SplashScreen.png": (620, 300),
}

BG_COLOR = (26, 15, 10, 0)  # Transparent (match bonfire scene base)


def create_logo(src: Image.Image, size: tuple[int, int], bg=BG_COLOR) -> Image.Image:
    """Create a centered logo on a transparent/canvas background."""
    w, h = size
    canvas = Image.new("RGBA", (w, h), bg)

    # Scale logo to fit within the canvas with padding
    padding = int(min(w, h) * 0.15)
    target = min(w, h) - padding * 2
    logo = src.copy()
    logo.thumbnail((target, target), Image.Resampling.LANCZOS)

    # Center
    x = (w - logo.width) // 2
    y = (h - logo.height) // 2
    canvas.paste(logo, (x, y), logo if logo.mode == "RGBA" else None)
    return canvas


def main():
    if not SRC_LOGO.exists():
        print(f"ERROR: Source logo not found: {SRC_LOGO}")
        sys.exit(1)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    src = Image.open(SRC_LOGO).convert("RGBA")

    for name, size in ASSETS.items():
        out_path = OUT_DIR / name
        img = create_logo(src, size)
        img.save(str(out_path), "PNG")
        print(f"  {name:30s} {size[0]}x{size[1]}  ({out_path.stat().st_size:,} bytes)")

    print(f"\nDone. {len(ASSETS)} assets generated in {OUT_DIR}")


if __name__ == "__main__":
    main()

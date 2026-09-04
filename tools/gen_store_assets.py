#!/usr/bin/env python3
"""Generate Microsoft Store asset images from the pomodoro logo.

Usage (from repo root):
    python tools/gen_store_assets.py

Reads assets/logo/pomodoro_256.png and produces (shared farm msix dir):
    ../release-archives/pomodoro/msix/assets/StoreLogo.png        (50x50)
    ../release-archives/pomodoro/msix/assets/Square44x44Logo.png   (44x44)
    ../release-archives/pomodoro/msix/assets/Square150x150Logo.png (150x150)
    ../release-archives/pomodoro/msix/assets/Wide310x150Logo.png   (310x150, logo centered)
    ../release-archives/pomodoro/msix/assets/SplashScreen.png      (620x300, logo centered)
    ../release-archives/pomodoro/msix/assets/Square44x44Logo.targetsize-{n}[_altform-unplated].png
        (n = 16/20/24/30/32/36/40/48/60/64/72/80/96/256, plated + 无底板双家族)

为什么要有 altform-unplated: MSIX 应用在任务栏/Alt-Tab 等 shell 表面, 包图标
会被垫上 manifest BackgroundColor 的底板, 且 "transparent" 在任务栏表面会回落成
Windows 默认蓝 (2026-09-04 商店版实测)。提供 targetsize-*_altform-unplated 后,
shell 直接使用裸图标, 不垫底板 —— 与便携版任务栏观感一致。
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
OUT_DIR = REPO_ROOT / ".." / "release-archives" / "pomodoro" / "msix" / "assets"

# Store-required asset sizes
ASSETS = {
    "StoreLogo.png": (50, 50),
    "Square44x44Logo.png": (44, 44),
    "Square150x150Logo.png": (150, 150),
    "Wide310x150Logo.png": (310, 150),
    "SplashScreen.png": (620, 300),
}

# ⚠️ 必须是 (0,0,0,0) 干净透明, 不能带 RGB 残留!
# 曾误设 (26,15,10,0): alpha=0 但 RGB=深棕的「脏透明」像素, 会让 Windows 任务栏
# (premultiplied alpha 合成) 判定图标非干净透明, 强制垫 BackgroundColor 底板 →
# transparent 回落默认蓝 (2026-09-04 商店版蓝底板根因, 对比 ScreenToGif 干净透明裸图标定位)。
BG_COLOR = (0, 0, 0, 0)

# targetsize 变体尺寸: 覆盖任务栏 / Alt-Tab / 标题栏等 shell 表面。
# 复刻 ScreenToGif 实测成功的同款尺寸 (16/24/32/48/256, plated + unplated 双家族)。
# 参考 anthropics/claude-code#59477: 缺尺寸会回退到底板 (plated) 基础资产。
TARGETSIZES = (16, 24, 32, 48, 256)

# scale 变体: DPI 缩放资产, 是 shell 走「现代资源解析」路径的触发器。
# 缺 scale 家族时, shell 落到 legacy 基础图标路径 → 垫 BackgroundColor,
# transparent 回落默认蓝 (2026-09-04 商店版实测: 有 scale 的 ScreenToGif/WT 裸,
# 无 scale 的我们垫蓝)。scale-N 物理尺寸 = 44 * N/100。
SCALES = {100: 44, 125: 55, 150: 66, 200: 88, 400: 176}


def create_logo(src: Image.Image, size: tuple[int, int], bg=BG_COLOR, padding_ratio: float = 0.15) -> Image.Image:
    """Create a centered logo on a transparent/canvas background."""
    w, h = size
    canvas = Image.new("RGBA", (w, h), bg)

    # Scale logo to fit within the canvas with padding
    padding = int(min(w, h) * padding_ratio)
    target = min(w, h) - padding * 2
    logo = src.copy()
    logo.thumbnail((target, target), Image.Resampling.LANCZOS)

    # Center
    x = (w - logo.width) // 2
    y = (h - logo.height) // 2
    canvas.paste(logo, (x, y), logo if logo.mode == "RGBA" else None)

    # 清理脏透明: alpha=0 的像素 RGB 必须归零 (straight alpha 语义)。
    # 源图缩放/抗锯齿边缘会残留「a=0 但 RGB≠0」的像素, Windows premultiplied
    # alpha 合成会据此误判非干净透明 → 垫底板。
    r, g, b, a = canvas.split()
    opaque = a.point(lambda v: 255 if v > 0 else 0)
    zero = Image.new("L", canvas.size, 0)
    r = Image.composite(r, zero, opaque)
    g = Image.composite(g, zero, opaque)
    b = Image.composite(b, zero, opaque)
    return Image.merge("RGBA", (r, g, b, a))


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

    # scale 家族 (DPI 缩放, 复刻 ScreenToGif): 图形与基础款一致 (padding 0.15)
    for scale, px in SCALES.items():
        name = f"Square44x44Logo.scale-{scale}.png"
        out_path = OUT_DIR / name
        img = create_logo(src, (px, px))
        img.save(str(out_path), "PNG")
        print(f"  {name:52s} {px}x{px}  ({out_path.stat().st_size:,} bytes)")

    # targetsize 双家族: plated (壳可垫底板) + altform-unplated (裸图标)。
    # 均接近满幅 (无底板时留白越小越好)。
    for n in TARGETSIZES:
        for suffix in ("", "_altform-unplated"):
            name = f"Square44x44Logo.targetsize-{n}{suffix}.png"
            out_path = OUT_DIR / name
            img = create_logo(src, (n, n), padding_ratio=0.05)
            img.save(str(out_path), "PNG")
            print(f"  {name:52s} {n}x{n}  ({out_path.stat().st_size:,} bytes)")

    print(f"\nDone. {len(ASSETS) + len(SCALES) + len(TARGETSIZES) * 2} assets generated in {OUT_DIR}")


if __name__ == "__main__":
    main()

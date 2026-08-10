#!/usr/bin/env python3
"""patch_icon.py — 在已编译的 Windows .exe 资源段中替换图标。

走 stdlib `ctypes` 调用 `kernel32!BeginUpdateResourceW` / `UpdateResourceW` /
`EndUpdateResourceW`,不依赖 windres / pefile / pywin32。Win32 API 自带 PE 资源段
布局 (.rsrc 段不存在时自动新建),因此无需手工解析 PE。

每个被解析的 ICO 条目写为 `RT_ICON`,随后用一个 `RT_GROUP_ICON` 组索引全部条目。
组的 id 默认是 1 — 假设目标 exe 还没有图标资源(.rsrc 段为空)。
若目标已有图标组,可改 `--group-id` 避开冲突。

Usage:
    python tools/patch_icon.py [--ico PATH] [--exe PATH] [--group-id N] [--verify]

Defaults:
    --ico        assets/logo/logo.ico
    --exe        target/release/examples/pomodoro.exe
    --group-id   1
"""
from __future__ import annotations

import argparse
import ctypes
import struct
import sys
from ctypes import wintypes
from pathlib import Path

# Win32: RT_ICON = 3, RT_GROUP_ICON = 14, MAKELANGID(NEUTRAL, NEUTRAL) = 0.
RT_ICON = 3
RT_GROUP_ICON = 14
LANG_NEUTRAL = 0

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ICO = REPO_ROOT / "assets" / "logo" / "logo.ico"
DEFAULT_EXE = REPO_ROOT / "target" / "release" / "examples" / "pomodoro.exe"

_kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
_kernel32.BeginUpdateResourceW.argtypes = [wintypes.LPCWSTR, wintypes.BOOL]
_kernel32.BeginUpdateResourceW.restype = wintypes.HANDLE
_kernel32.UpdateResourceW.argtypes = [
    wintypes.HANDLE,
    wintypes.LPCWSTR,
    wintypes.LPCWSTR,
    wintypes.WORD,
    wintypes.LPCVOID,
    wintypes.DWORD,
]
_kernel32.UpdateResourceW.restype = wintypes.BOOL
_kernel32.EndUpdateResourceW.argtypes = [wintypes.HANDLE, wintypes.BOOL]
_kernel32.EndUpdateResourceW.restype = wintypes.BOOL

# MAKEINTRESOURCE(i) 把小整数零扩展到 ULONG_PTR 再强转 LPWSTR。
# 在 ctypes 里用 `LPCWSTR(i)` 即可,小整数 < 0x10000 不会被截断。
def _rt(i: int) -> wintypes.LPCWSTR:
    return wintypes.LPCWSTR(i)


def parse_ico(path: Path) -> list[dict]:
    """读 .ico,返回每条图标的元数据 + 像素字节。ICO 0 表示 256 px(微软规范)。"""
    blob = path.read_bytes()
    if len(blob) < 6:
        raise ValueError(f"{path} 不是有效的 ICO (少于 6 字节)")
    reserved, ftype, count = struct.unpack_from("<HHH", blob, 0)
    if reserved != 0 or ftype != 1 or count == 0:
        raise ValueError(
            f"{path} 不是单图类型 ICO (reserved={reserved} type={ftype} count={count})"
        )
    icons: list[dict] = []
    for i in range(count):
        off = 6 + 16 * i
        w, h, palette, _r, planes, bpp, sz, data_off = struct.unpack_from(
            "<BBBBHHII", blob, off
        )
        if data_off + sz > len(blob):
            raise ValueError(f"{path} 第 {i} 条图标超出文件末尾")
        icons.append(
            {
                "id": i + 1,
                "width": w if w else 256,
                "height": h if h else 256,
                "color_count": palette,
                "planes": planes,
                "bpp": bpp,
                "data": blob[data_off : data_off + sz],
            }
        )
    return icons


def build_group(icons: list[dict]) -> bytes:
    """组索引 (GRPICONDIR) — 紧跟在 RT_ICON 写完后单独写一遍。"""
    header = struct.pack("<HHH", 0, 1, len(icons))
    body = b""
    for ic in icons:
        w = ic["width"] if ic["width"] < 256 else 0
        h = ic["height"] if ic["height"] < 256 else 0
        body += struct.pack(
            "<BBBBHHIH",
            w,
            h,
            ic["color_count"],
            0,
            ic["planes"],
            ic["bpp"],
            len(ic["data"]),
            ic["id"],
        )
    return header + body


def patch(exe: Path, ico: Path, group_id: int) -> int:
    """嵌入 ICO,返回写入的图标条目数。失败抛 OSError(ctypes 错误码已编码于 errno)。"""
    icons = parse_ico(ico)
    group = build_group(icons)
    # BeginUpdateResourceW 不接受相对路径 — 必须绝对路径(Win32 docs 明说)。
    abs_exe = exe if exe.is_absolute() else exe.resolve()
    h = _kernel32.BeginUpdateResourceW(str(abs_exe), False)
    if not h:
        raise OSError(f"BeginUpdateResourceW 失败 (win32 err={ctypes.get_last_error()})")
    try:
        for ic in icons:
            ok = _kernel32.UpdateResourceW(
                h,
                _rt(RT_ICON),
                _rt(ic["id"]),
                LANG_NEUTRAL,
                ic["data"],
                len(ic["data"]),
            )
            if not ok:
                raise OSError(
                    f"UpdateResourceW RT_ICON id={ic['id']} 失败 "
                    f"(win32 err={ctypes.get_last_error()})"
                )
        ok = _kernel32.UpdateResourceW(
            h,
            _rt(RT_GROUP_ICON),
            _rt(group_id),
            LANG_NEUTRAL,
            group,
            len(group),
        )
        if not ok:
            raise OSError(
                f"UpdateResourceW RT_GROUP_ICON id={group_id} 失败 "
                f"(win32 err={ctypes.get_last_error()})"
            )
    finally:
        if not _kernel32.EndUpdateResourceW(h, False):
            raise OSError(f"EndUpdateResourceW 失败 (win32 err={ctypes.get_last_error()})")
    return len(icons)


def count_icons(exe: Path) -> int:
    """用 shell32!ExtractIconExW 数 exe 内嵌的图标个数 (索引 -1 = 仅返回总数)。"""
    shell32 = ctypes.WinDLL("shell32", use_last_error=True)
    shell32.ExtractIconExW.argtypes = [
        wintypes.LPCWSTR,
        wintypes.INT,
        ctypes.POINTER(wintypes.HANDLE),
        ctypes.POINTER(wintypes.HANDLE),
        wintypes.UINT,
    ]
    shell32.ExtractIconExW.restype = wintypes.UINT
    return shell32.ExtractIconExW(str(exe), -1, None, None, 0)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--ico", type=Path, default=DEFAULT_ICO)
    ap.add_argument("--exe", type=Path, default=DEFAULT_EXE)
    ap.add_argument("--group-id", type=int, default=1)
    args = ap.parse_args()

    if not args.exe.exists():
        print(
            f"ERROR: 目标 exe 不存在: {args.exe}\n"
            "先运行 `cargo build --release --example pomodoro`",
            file=sys.stderr,
        )
        return 1
    if not args.ico.exists():
        print(f"ERROR: 图标文件不存在: {args.ico}", file=sys.stderr)
        return 1

    before_bytes = args.exe.stat().st_size
    before_icons = count_icons(args.exe)
    print(f"Patch 前: {args.exe} ({before_bytes:,} B, 内嵌图标数={before_icons})")

    n = patch(args.exe, args.ico, args.group_id)

    after_bytes = args.exe.stat().st_size
    after_icons = count_icons(args.exe)
    delta = after_bytes - before_bytes
    print(
        f"Patch 后: {args.exe} ({after_bytes:,} B, "
        f"内嵌图标数={after_icons}, Δ字节={delta:+d}, 写入条目={n})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

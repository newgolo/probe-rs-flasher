#!/usr/bin/env python3
"""生成 Probe-rs Flasher 的应用图标（可复现）。

设计：蓝色渐变圆角方块 + 白色「下载/烧录」箭头与托盘。
用法：
    python3 tools/gen_icon.py
产物（assets/）：
    icon.svg          设计源文件
    icon-1024.png     主图
    icon-512/256/128/64/48/32/16.png
    icon.ico          Windows 图标（多尺寸）
    icon-64.rgba      编译期嵌入用的原始 RGBA 数据
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

ASSETS = Path(__file__).resolve().parent.parent / "assets"

SIZE = 1024
SS = 4  # 超采样倍数，保证边缘平滑
CANVAS = SIZE * SS

TOP = (0x4F, 0xA8, 0xFF)
BOTTOM = (0x14, 0x5F, 0xC0)
WHITE = (0xFF, 0xFF, 0xFF)

RADIUS = int(SIZE * 0.22)


def rounded_mask(size: int, radius: int) -> np.ndarray:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return np.asarray(mask, dtype=np.float32) / 255.0


def diagonal_gradient(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> np.ndarray:
    y, x = np.mgrid[0:size, 0:size].astype(np.float32)
    t = (x + y) / (2.0 * (size - 1))  # 左上 -> 右下
    t = t[..., None]
    top_arr = np.array(top, dtype=np.float32)
    bottom_arr = np.array(bottom, dtype=np.float32)
    return top_arr * (1.0 - t) + bottom_arr * t


def build_master() -> Image.Image:
    rgb = diagonal_gradient(CANVAS, TOP, BOTTOM)

    # 左上角高光，让图标更有层次
    y, x = np.mgrid[0:CANVAS, 0:CANVAS].astype(np.float32)
    glow = np.clip(1.0 - np.sqrt((x - CANVAS * 0.22) ** 2 + (y - CANVAS * 0.18) ** 2) / (CANVAS * 0.85), 0.0, 1.0)
    glow = (glow**2)[..., None] * 0.16
    rgb = rgb * (1.0 - glow) + 255.0 * glow

    mask = rounded_mask(CANVAS, RADIUS * SS)

    icon = Image.fromarray(np.dstack([rgb, mask * 255.0]).astype(np.uint8), mode="RGBA")

    # 前景（白色箭头 + 托盘），坐标基于 1024 设计稿，按超采样放大
    def s(value: float) -> float:
        return value * SS

    layer = Image.new("L", (CANVAS, CANVAS), 0)
    draw = ImageDraw.Draw(layer)

    # 箭头杆
    draw.rounded_rectangle(
        (s(466), s(232), s(558), s(516)),
        radius=s(46),
        fill=255,
    )
    # 箭头头部
    draw.polygon(
        [(s(322), s(492)), (s(702), s(492)), (s(512), s(672))],
        fill=255,
    )
    # 托盘
    draw.rounded_rectangle(
        (s(312), s(712), s(712), s(796)),
        radius=s(42),
        fill=255,
    )

    foreground = np.asarray(layer, dtype=np.float32)[..., None] / 255.0
    base = np.asarray(icon, dtype=np.float32)
    base[..., :3] = base[..., :3] * (1.0 - foreground) + np.array(WHITE, dtype=np.float32) * foreground
    base[..., 3] = np.maximum(base[..., 3], np.asarray(layer, dtype=np.float32))

    return Image.fromarray(base.astype(np.uint8), mode="RGBA").resize(
        (SIZE, SIZE), Image.LANCZOS
    )


def write_svg(master: Image.Image) -> None:
    """写一份与生成图一致的矢量源文件（便于后续手工微调）。"""
    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024" role="img" aria-label="Probe-rs Flasher">
  <title>Probe-rs Flasher</title>
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#4FA8FF"/>
      <stop offset="1" stop-color="#145FC0"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.22" cy="0.18" r="0.85">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.16"/>
      <stop offset="1" stop-color="#ffffff" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="round">
      <rect width="1024" height="1024" rx="{RADIUS}" ry="{RADIUS}"/>
    </clipPath>
  </defs>
  <g clip-path="url(#round)">
    <rect width="1024" height="1024" fill="url(#bg)"/>
    <rect width="1024" height="1024" fill="url(#glow)"/>
    <g fill="#ffffff">
      <rect x="466" y="232" width="92" height="284" rx="46"/>
      <path d="M322 492h380L512 672Z"/>
      <rect x="312" y="712" width="400" height="84" rx="42"/>
    </g>
  </g>
</svg>
"""
    (ASSETS / "icon.svg").write_text(svg, encoding="utf-8")


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)

    master = build_master()
    master.save(ASSETS / "icon-1024.png")
    write_svg(master)

    for size in (512, 256, 128, 64, 48, 32, 16):
        master.resize((size, size), Image.LANCZOS).save(ASSETS / f"icon-{size}.png")

    master.save(
        ASSETS / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )

    # 编译期嵌入用（egui::IconData 需要原始 RGBA）
    rgba = master.resize((64, 64), Image.LANCZOS).tobytes()
    (ASSETS / "icon-64.rgba").write_bytes(rgba)

    print(f"wrote icons to {ASSETS}")


if __name__ == "__main__":
    main()

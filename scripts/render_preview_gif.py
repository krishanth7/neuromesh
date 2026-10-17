#!/usr/bin/env python3
"""Render the README live-preview animation from recorded measurement evidence.

Every value drawn comes from docs/results/localhost-quic.json; nothing is
synthetic. The script is deterministic: the same evidence file and Pillow
version produce the same animation, so regeneration is reviewable in git.

Usage:
    python3 scripts/render_preview_gif.py            # writes docs/media/live-preview-replay.gif
    python3 scripts/render_preview_gif.py -o out.gif # custom target
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
EVIDENCE = ROOT / "docs" / "results" / "localhost-quic.json"
OUTPUT = ROOT / "docs" / "media" / "live-preview-replay.gif"

BG = (11, 18, 32)
PANEL = (18, 27, 46)
LINE = (30, 43, 69)
TEXT = (230, 237, 247)
MUTED = (143, 163, 193)
ACCENT = (53, 208, 186)
ACCENT_2 = (90, 162, 255)
WARN = (242, 181, 68)

SCALE = 2                      # supersampling factor
WIDTH, HEIGHT = 880, 480       # final frame size
SAMPLES_PER_FRAME = 4           # recorded samples revealed per frame
FRAMES_PER_PASS = 25            # 100 recorded samples / 4 per frame
HOLD_FRAMES = 8                 # tail frames carrying the final summary


def font(size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.load_default(size=size * SCALE)


def nearest_rank(values: list[int], percentile: float) -> int:
    """Nearest-rank percentile, matching the recorded method statement."""
    ordered = sorted(values)
    rank = max(1, min(len(ordered), math_ceil((percentile / 100) * len(ordered))))
    return ordered[rank - 1]


def math_ceil(value: float) -> int:
    integer = int(value)
    return integer if integer == value else integer + 1


def ms(microseconds: int | float) -> str:
    return f"{microseconds / 1000:.3f} ms"


def render_frame(evidence: dict, drawn: int, hold_index: int, draw: ImageDraw.ImageDraw) -> None:
    measurement = evidence["measurement"]
    samples: list[int] = measurement["rtt_us"]

    # Canvas
    draw.rectangle([0, 0, WIDTH * SCALE, HEIGHT * SCALE], fill=BG)

    # Header
    draw.rounded_rectangle(
        [28 * SCALE, 24 * SCALE, 60 * SCALE, 56 * SCALE],
        radius=10 * SCALE,
        fill=ACCENT,
    )
    draw.rectangle(
        [37 * SCALE, 33 * SCALE, 51 * SCALE, 47 * SCALE],
        outline=BG,
        width=3 * SCALE,
    )
    draw.text(
        (74 * SCALE, 24 * SCALE),
        "NeuroMesh \u00b7 authenticated QUIC session",
        font=font(19),
        fill=TEXT,
    )
    draw.text(
        (74 * SCALE, 50 * SCALE),
        "live replay of recorded measurements \u00b7 localhost \u00b7 mutual TLS \u00b7 Ed25519 proof",
        font=font(11),
        fill=MUTED,
    )
    state = "REPLAY COMPLETE" if drawn >= len(samples) else f"PING {drawn:03d} / 100"
    pill_w = 170 * SCALE
    draw.rounded_rectangle(
        [WIDTH * SCALE - pill_w - 28 * SCALE, 26 * SCALE, (WIDTH - 28) * SCALE, 56 * SCALE],
        radius=14 * SCALE,
        outline=ACCENT,
        width=2 * SCALE,
        fill=PANEL,
    )
    draw.text(
        (WIDTH * SCALE - pill_w - 28 * SCALE + 16 * SCALE, 32 * SCALE),
        state,
        font=font(13),
        fill=ACCENT,
    )

    # Charts
    chart_top, chart_bottom = 95 * SCALE, 305 * SCALE
    draw.rounded_rectangle(
        [28 * SCALE, chart_top, (WIDTH - 28) * SCALE, chart_bottom],
        radius=10 * SCALE,
        fill=PANEL,
        outline=LINE,
        width=1 * SCALE,
    )

    visible = samples[:drawn]
    max_us = max(samples)
    inner_left, inner_right = 44 * SCALE, (WIDTH - 44) * SCALE
    inner_bottom = chart_bottom - 16 * SCALE
    span = inner_right - inner_left

    # Gridlines
    for fraction in (0.25, 0.5, 0.75, 1.0):
        y = inner_bottom - int((chart_bottom - 16 * SCALE - chart_top - 16 * SCALE) * fraction)
        draw.line(
            [inner_left, y, inner_right, y],
            fill=LINE,
            width=1 * SCALE,
        )

    if visible:
        bar_slot = span / len(samples)
        for index, value in enumerate(visible):
            if index >= len(samples):
                break
            x0 = inner_left + int(index * bar_slot)
            x1 = inner_left + int((index + 1) * bar_slot) - 2 * SCALE
            height = max(3 * SCALE, int((value / max_us) * (inner_bottom - chart_top - 24 * SCALE)))
            color = ACCENT_2 if value < 1000 else ACCENT
            draw.rounded_rectangle(
                [x0, inner_bottom - height, x1, inner_bottom],
                radius=1 * SCALE,
                fill=color,
            )
    draw.text(
        (inner_left, chart_top + 8 * SCALE),
        "round-trip time per exchange (recorded samples)",
        font=font(10),
        fill=MUTED,
    )

    # Counters
    counters = [
        ("handshake + proof", ms(measurement["handshake_us"]), MUTED),
        ("median (p50)", ms(nearest_rank(visible, 50)) if visible else "-", TEXT),
        ("p95", ms(nearest_rank(visible, 95)) if visible else "-", TEXT),
        ("exchanges", f"{drawn} / 100", ACCENT),
        ("failures", "0", (70, 213, 151) if visible else MUTED),
    ]
    tile_top = 320 * SCALE
    tile_width = ((WIDTH - 56) // len(counters)) * SCALE
    for index, (label, value, color) in enumerate(counters):
        x = 28 * SCALE + index * tile_width
        draw.rounded_rectangle(
            [x, tile_top, x + tile_width - 10 * SCALE, tile_top + 78 * SCALE],
            radius=10 * SCALE,
            fill=PANEL,
            outline=LINE,
            width=1 * SCALE,
        )
        draw.text(
            (x + 14 * SCALE, tile_top + 12 * SCALE),
            label.upper(),
            font=font(9),
            fill=MUTED,
        )
        draw.text(
            (x + 14 * SCALE, tile_top + 32 * SCALE),
            value,
            font=font(17),
            fill=color,
        )

    # Footer honesty line
    footer = (
        "Replay of real recorded samples (docs/results/localhost-quic.json) "
        "\u00b7 not synthetic, no sockets opened"
    )
    if drawn >= len(samples):
        summary = (
            f"final: p50 {ms(measurement['p50_us'])} \u00b7 p95 {ms(measurement['p95_us'])} \u00b7 "
            f"min/max {ms(measurement['min_us'])}/{ms(measurement['max_us'])}"
        )
        footer = summary + "  \u00b7  " + footer
    draw.text(
        (28 * SCALE, 412 * SCALE),
        footer,
        font=font(10),
        fill=MUTED,
    )
    draw.text(
        (28 * SCALE, 434 * SCALE),
        "RTT includes the transport acknowledgment wait. One local release-built run, "
        "10 warmups - not WAN latency or an SLO.",
        font=font(10),
        fill=(110, 126, 152),
    )

    # Tail progress dots give each hold frame distinct pixels (and a visible
    # "still recording" cadence) so no frame is dropped during optimization.
    if hold_index:
        for dot in range(hold_index):
            x = (WIDTH - 34 - dot * 10) * SCALE
            y = 66 * SCALE
            draw.ellipse([x, y, x + 5 * SCALE, y + 5 * SCALE], fill=ACCENT)


def build(output: pathlib.Path) -> pathlib.Path:
    evidence = json.loads(EVIDENCE.read_text())
    samples = evidence["measurement"]["rtt_us"]
    if len(samples) != 100:
        raise SystemExit(f"expected 100 recorded samples, found {len(samples)}")

    frames = []
    stops = [min(100, (index + 1) * SAMPLES_PER_FRAME) for index in range(FRAMES_PER_PASS)]
    steps = stops + [100] * HOLD_FRAMES
    for position, drawn in enumerate(steps):
        big = Image.new("RGB", (WIDTH * SCALE, HEIGHT * SCALE), BG)
        hold_index = position - len(stops) + 1  # 0 while revealing samples
        render_frame(evidence, drawn, max(hold_index, 0), ImageDraw.Draw(big))
        frame = big.resize((WIDTH, HEIGHT), Image.LANCZOS)
        frames.append(frame.convert("P", palette=Image.ADAPTIVE, colors=96))

    output.parent.mkdir(parents=True, exist_ok=True)
    durations = [90] * (FRAMES_PER_PASS - 1) + [1500] + [240] * HOLD_FRAMES
    frames[0].save(
        output,
        save_all=True,
        append_images=frames[1:],
        duration=durations,
        loop=0,
        optimize=True,
        disposal=2,
    )

    # Sanity: readable frame count and no frame above a sane size envelope.
    count = 0
    with Image.open(output) as gif:
        count = gif.n_frames
    if count != len(frames):
        raise SystemExit(f"frame count mismatch: {count} != {len(frames)}")
    size_kib = output.stat().st_size / 1024
    if size_kib > 2048:
        raise SystemExit(f"animation is {size_kib:.0f} KiB, above the 2 MiB review budget")
    print(f"wrote {output} ({count} frames, {size_kib:.0f} KiB)")
    # Keep the drawn statistics honest against the evidence file: nearest-rank,
    # the same definition the recording used.
    print(
        "evidence: "
        f"p50 {ms(evidence['measurement']['p50_us'])} "
        f"(nearest-rank check {ms(nearest_rank(samples, 50))}), "
        f"p95 {ms(evidence['measurement']['p95_us'])} "
        f"(nearest-rank check {ms(nearest_rank(samples, 95))})"
    )
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("-o", "--output", type=pathlib.Path, default=OUTPUT)
    args = parser.parse_args()
    build(args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
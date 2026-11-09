#!/usr/bin/env python3
"""Ensambla los fotogramas de capture.mjs en un GIF animado lado a lado (solo biblioteca estándar).

Uso: python -I tools/anim_compare/make_gif.py <carpeta-capturas> <estado> <salida.gif> <variante> [<variante> ...]

Cada variante es una carpeta <carpeta-capturas>/<estado>/<variante>/NNNN.png; las columnas salen en el orden dado
y el fotograma i de todas las columnas corresponde al MISMO instante (times.json). Los retardos del GIF (centésimas
de segundo) se reparten para respetar la cadencia: 30 fps → 3,4,3,3,4,3...; 20 fps → 5; 10 fps → 10.
"""
import struct
import sys
import zlib
from collections import Counter
from pathlib import Path


# ---------------------------------------------------------------- PNG (8 bits, RGB o RGBA, sin entrelazado)
def read_png(path: Path):
    d = path.read_bytes()
    assert d[:8] == b"\x89PNG\r\n\x1a\n"
    i, idat = 8, b""
    w = h = ct = None
    while i < len(d):
        n, = struct.unpack(">I", d[i:i + 4])
        t = d[i + 4:i + 8]
        if t == b"IHDR":
            w, h, bd, ct, _, _, il = struct.unpack(">IIBBBBB", d[i + 8:i + 21])
            assert bd == 8 and il == 0 and ct in (2, 6), (bd, ct, il)
        elif t == b"IDAT":
            idat += d[i + 8:i + 8 + n]
        i += 12 + n
    bpp = 3 if ct == 2 else 4
    raw = zlib.decompress(idat)
    stride = w * bpp
    prev = bytearray(stride)
    rows = []
    p = 0
    for _ in range(h):
        f = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        if f == 1:
            for x in range(bpp, stride):
                line[x] = (line[x] + line[x - bpp]) & 255
        elif f == 2:
            for x in range(stride):
                line[x] = (line[x] + prev[x]) & 255
        elif f == 3:
            for x in range(stride):
                a = line[x - bpp] if x >= bpp else 0
                line[x] = (line[x] + ((a + prev[x]) >> 1)) & 255
        elif f == 4:
            for x in range(stride):
                a = line[x - bpp] if x >= bpp else 0
                b = prev[x]
                c = prev[x - bpp] if x >= bpp else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        rows.append(bytes(line) if bpp == 3 else b"".join(bytes(line[k:k + 3]) for k in range(0, stride, 4)))
        prev = line
    return w, h, rows  # filas RGB


# ---------------------------------------------------------------- GIF
def lzw(indices: bytes, min_code=8) -> bytes:
    clear, eoi = 1 << min_code, (1 << min_code) + 1
    out = bytearray()
    bitbuf = nbits = 0
    size = min_code + 1
    table = {}
    nxt = eoi + 1

    def emit(code):
        nonlocal bitbuf, nbits
        bitbuf |= code << nbits
        nbits += size
        while nbits >= 8:
            out.append(bitbuf & 255)
            bitbuf >>= 8
            nbits -= 8

    emit(clear)
    cur = indices[0]
    for b in indices[1:]:
        key = (cur << 8) | b
        c = table.get(key)
        if c is not None:
            cur = c
            continue
        emit(cur)
        if nxt < 4096:
            table[key] = nxt
            nxt += 1
            if nxt - 1 == (1 << size) and size < 12:
                size += 1
        else:
            emit(clear)
            table.clear()
            nxt = eoi + 1
            size = min_code + 1
        cur = b
    emit(cur)
    emit(eoi)
    if nbits:
        out.append(bitbuf & 255)
    return bytes(out)


def blocks(data: bytes) -> bytes:
    return b"".join(bytes([len(data[i:i + 255])]) + data[i:i + 255] for i in range(0, len(data), 255)) + b"\x00"


def main() -> None:
    root, state, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
    variants = sys.argv[4:]
    meta = __import__("json").loads((root / state / "times.json").read_text())
    n = len(meta["times_ms"])
    gap = 6
    bg = b"\xf4\xf1\xea"
    # 1) leer y componer todos los fotogramas (filas RGB concatenadas con un hueco entre columnas)
    frames = []
    W = H = None
    for i in range(n):
        cols = [read_png(root / state / v / f"{i:04d}.png") for v in variants]
        h = cols[0][1]
        rows = []
        for y in range(h):
            parts = []
            for k, c in enumerate(cols):
                if k:
                    parts.append(bg * gap)
                parts.append(c[2][y])
            rows.append(b"".join(parts))
        W = len(rows[0]) // 3
        H = h
        frames.append(rows)
    # 2) paleta adaptativa: los 255 colores más frecuentes de una muestra + el 256.º = fondo
    sample = Counter()
    for rows in frames[:: max(1, n // 12)]:
        for r in rows:
            sample.update(r[k:k + 3] for k in range(0, len(r), 3))
    pal = [c for c, _ in sample.most_common(255)]
    cache = {c: j for j, c in enumerate(pal)}

    def nearest(c):
        r, g, b = c
        best, bj = 1 << 30, 0
        for j, p in enumerate(pal):
            d = (r - p[0]) ** 2 + (g - p[1]) ** 2 + (b - p[2]) ** 2
            if d < best:
                best, bj = d, j
        return bj

    pal = [tuple(p) for p in pal]
    cache = {bytes(p): j for j, p in enumerate(pal)}
    pal_bytes = b"".join(bytes(p) for p in pal).ljust(768, b"\x00")
    # 3) escribir
    delays = {30: [3, 4, 3], 20: [5], 10: [10], 15: [7, 6]}.get(meta["fps"], [max(2, round(100 / meta["fps"]))])
    g = bytearray(b"GIF89a")
    g += struct.pack("<HHBBB", W, H, 0xF7, 0, 0) + pal_bytes
    g += b"\x21\xFF\x0BNETSCAPE2.0\x03\x01\x00\x00\x00"
    for i, rows in enumerate(frames):
        idx = bytearray()
        for r in rows:
            for k in range(0, len(r), 3):
                c = r[k:k + 3]
                j = cache.get(c)
                if j is None:
                    j = nearest(tuple(c))
                    cache[c] = j
                idx.append(j)
        g += b"\x21\xF9\x04\x04" + struct.pack("<H", delays[i % len(delays)]) + b"\x00\x00"
        g += b"\x2C" + struct.pack("<HHHHB", 0, 0, W, H, 0) + b"\x08" + blocks(lzw(bytes(idx)))
    g += b"\x3B"
    out.write_bytes(bytes(g))
    print(f"{out} — {W}x{H}, {n} fotogramas, {len(g) / 1e6:.2f} MB")


if __name__ == "__main__":
    main()

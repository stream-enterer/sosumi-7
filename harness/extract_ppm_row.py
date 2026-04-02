#!/usr/bin/env python3
"""Extract specific pixels from a PPM file (P6 binary format)."""
import sys

def read_ppm(path):
    with open(path, 'rb') as f:
        magic = f.readline().strip()
        assert magic == b'P6', f"Expected P6, got {magic}"
        # Skip comments
        line = f.readline()
        while line.startswith(b'#'):
            line = f.readline()
        w, h = map(int, line.split())
        maxval = int(f.readline().strip())
        assert maxval == 255
        data = f.read(w * h * 3)
    return w, h, data

if __name__ == '__main__':
    path = sys.argv[1]
    row = int(sys.argv[2])
    x_start = int(sys.argv[3])
    x_end = int(sys.argv[4])

    w, h, data = read_ppm(path)
    print(f"PPM: {w}x{h}")
    print(f"Row {row}, x={x_start}..{x_end}:")
    for x in range(x_start, x_end + 1):
        off = (row * w + x) * 3
        r, g, b = data[off], data[off+1], data[off+2]
        print(f"  x={x}: rgb({r},{g},{b})")

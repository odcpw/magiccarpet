"""Generate the Modern scroll's tileable, mip-visible parchment grain."""
import argparse
import math
import random
import struct
import zlib
from pathlib import Path


def modern_paper_texture(path):
    rng = random.Random(25909)
    width, height = 1024, 512
    grids = [(nx, ny, [rng.uniform(-1, 1) for _ in range(nx * ny)])
             for nx, ny in ((8, 6), (19, 13), (43, 29))]

    def noise(x, y, nx, ny, values):
        xx, yy = x * nx / width, y * ny / height
        ix, iy = int(xx), int(yy)
        u, v = xx - ix, yy - iy
        u, v = u * u * (3 - 2 * u), v * v * (3 - 2 * v)
        a = values[(iy % ny) * nx + ix % nx]
        b = values[(iy % ny) * nx + (ix + 1) % nx]
        c = values[((iy + 1) % ny) * nx + ix % nx]
        d = values[((iy + 1) % ny) * nx + (ix + 1) % nx]
        return (a * (1 - u) + b * u) * (1 - v) + (c * (1 - u) + d * u) * v

    pixels = bytearray()
    for y in range(height):
        pixels.append(0)  # PNG scanline with no filter
        for x in range(width):
            mottling = sum(weight * noise(x, y, *grid) for weight, grid in zip((.11, .065, .025), grids))
            fiber = .008 * math.sin(math.tau * (y / 19 + .15 * math.sin(x / 47)))
            value = max(.64, min(.98, .85 + mottling + fiber + rng.uniform(-.015, .015)))
            pixels.extend((round(255 * value), round(255 * value * .96), round(255 * value * .88), 255))

    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))

    with path.open('xb') as output:
        output.write(b'\x89PNG\r\n\x1a\n' +
                     chunk(b'IHDR', struct.pack('!IIBBBBB', width, height, 8, 6, 0, 0, 0)) +
                     chunk(b'IDAT', zlib.compress(pixels, 9)) + chunk(b'IEND', b''))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    modern_paper_texture(parser.parse_args().output)

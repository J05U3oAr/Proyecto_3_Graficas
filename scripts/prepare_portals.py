"""Convert the supplied WEBP/GIF to the PNGs used by the CPU renderer.

python scripts/prepare_portals.py --yellow path/to/Portal_amarillo.webp \
    --green path/to/Portal_verde.gif

Requires Pillow only when regenerating. Preserves the GIF's 56 frames and white
sparks; flood-fills only the connected pale background, never all white pixels.
"""
import argparse
from collections import deque
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SIZE = 256


def remove_background(image):
    image = image.convert('RGBA')
    pixels = image.load()
    w, h = image.size
    seen = bytearray(w * h)
    queue = deque([(x, 0) for x in range(w)] + [(x, h - 1) for x in range(w)]
                  + [(0, y) for y in range(h)] + [(w - 1, y) for y in range(h)])
    while queue:
        x, y = queue.popleft()
        index = y * w + x
        if seen[index]:
            continue
        seen[index] = 1
        r, g, b, a = pixels[x, y]
        if min(r, g, b) < 225 or max(r, g, b) - min(r, g, b) > 28:
            continue
        pixels[x, y] = (0, 0, 0, 0)
        for nx, ny in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]:
            if 0 <= nx < w and 0 <= ny < h and not seen[ny * w + nx]:
                queue.append((nx, ny))
    return image


def tile(image, bounds):
    image = image.crop(bounds)
    image.thumbnail((SIZE - 8, SIZE - 8), Image.Resampling.LANCZOS)
    result = Image.new('RGBA', (SIZE, SIZE))
    result.alpha_composite(image, ((SIZE - image.width) // 2, (SIZE - image.height) // 2))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--yellow', required=True, type=Path)
    parser.add_argument('--green', required=True, type=Path)
    args = parser.parse_args()
    output = ROOT / 'assets' / 'portals'
    output.mkdir(parents=True, exist_ok=True)
    yellow = Image.open(args.yellow).convert('RGBA')
    tile(yellow, yellow.getbbox()).save(output / 'yellow.png', optimize=True)
    gif = Image.open(args.green)
    if gif.n_frames != 56:
        raise ValueError('El atlas del juego espera los 56 cuadros del GIF proporcionado.')
    frames, durations = [], []
    for frame in range(gif.n_frames):
        gif.seek(frame)
        frames.append(remove_background(gif))
        durations.append(gif.info.get('duration', 50))
    if set(durations) != {50}:
        raise ValueError('El GIF de referencia debe usar 50 ms por cuadro.')
    boxes = [frame.getbbox() for frame in frames]
    bounds = (min(b[0] for b in boxes), min(b[1] for b in boxes),
              max(b[2] for b in boxes), max(b[3] for b in boxes))
    atlas = Image.new('RGBA', (8 * SIZE, 7 * SIZE))
    for i, frame in enumerate(frames):
        atlas.alpha_composite(tile(frame, bounds), ((i % 8) * SIZE, (i // 8) * SIZE))
    atlas.save(output / 'green-atlas.png', optimize=True)
    print(f'Portals: yellow 256x256; green {len(frames)} frames, 2048x1792, 2.8 s loop')


if __name__ == '__main__':
    main()

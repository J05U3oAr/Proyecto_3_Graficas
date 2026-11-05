"""Download the original astronomy plates and losslessly convert them to PNG.

Requires Pillow. Does not retouch, crop, or remove marks from source images.
The orange accretion image is a separate generated artwork kept in the project.
"""
from io import BytesIO
from pathlib import Path
import urllib.request

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCES = {
    'helix': 'https://astropix.org/archive/spitzer/ssc2007-03a1/spitzer_ssc2007-03a1_3000.jpg',
    'southern-ring': 'https://cdn.esawebb.org/archives/images/publicationjpg/weic2207b.jpg',
    'binary': 'https://www.ligo.caltech.edu/system/avm_image_sqls/binaries/48/titanic/ligo20160211d.jpg',
    'spiral': 'https://astropix.org/archive/esahubble/potw1422a/esahubble_potw1422a_3000.jpg',
}


def main():
    output = ROOT / 'assets/deep-space/textures'
    output.mkdir(parents=True, exist_ok=True)
    for name, url in SOURCES.items():
        destination = output / f'{name}.png'
        if destination.exists():
            print(f'{name}: already downloaded', flush=True)
            continue
        request = urllib.request.Request(url, headers={'User-Agent': 'Orbital asset preparation'})
        with urllib.request.urlopen(request, timeout=45) as response:
            image = Image.open(BytesIO(response.read())).convert('RGB')
        image.save(destination)
        print(f'{name}: {image.width} x {image.height}', flush=True)


if __name__ == '__main__':
    main()

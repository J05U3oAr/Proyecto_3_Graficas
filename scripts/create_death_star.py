"""Rebuild the original procedural Death Star mesh and panel atlas (Pillow).

Visual reference: https://www.starwars.com/databank/death-star
No downloaded models or textures. Runtime needs only the generated OBJ/MTL/PNG.
"""
from pathlib import Path
import math
import random
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1] / "assets" / "death-star"
ROOT.mkdir(parents=True, exist_ok=True)
rng = random.Random(1977)
image = Image.new("RGB", (2048, 1024), (105, 110, 115))
draw = ImageDraw.Draw(image)
for band in range(128):
    y = band * 8
    x = -rng.randrange(12, 70)
    while x < 2048:
        w = rng.randrange(12, 72)
        value = rng.randrange(85, 151)
        draw.rectangle((x, y, x + w - 2, y + 6), fill=(value, value + 3, value + 7))
        draw.line((x, y + 7, x + w, y + 7), fill=(46, 51, 57))
        for _ in range(w // 6):
            px = x + rng.randrange(max(1, w - 2))
            py = y + rng.randrange(2, 6)
            draw.line((px, py, px + rng.randrange(1, 6), py), fill=rng.choice([
                (42, 48, 54), (73, 79, 86), (164, 170, 173), (196, 204, 207)]))
        x += w
for y in [182, 326, 448, 566, 698, 830]:
    draw.rectangle((0, y, 2047, y + 4), fill=(47, 52, 58))
    draw.line((0, y - 1, 2047, y - 1), fill=(173, 177, 182))
draw.rectangle((0, 502, 2047, 522), fill=(23, 29, 36))
for x in range(0, 2048, 7):
    draw.rectangle((x, 508, x + 3, 511), fill=(112, 127, 139))
    draw.rectangle((x + 2, 516, x + 4, 518), fill=(65, 80, 93))
image.save(ROOT / "panels.png")

vertices, uvs, faces = [], [], []
def vertex(p, uv=(0, 0)):
    vertices.append(p)
    uvs.append(uv)
    return len(vertices)

def face(a, b, c, material):
    # All triangles face away from the station's center (including the bowl).
    p, q, r = (vertices[i - 1] for i in (a, b, c))
    u, v = ([q[i] - p[i] for i in range(3)], [r[i] - p[i] for i in range(3)])
    n = (u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0])
    if sum(n[i]*p[i] for i in range(3)) < 0:
        b, c = c, b
    faces.append((a, b, c, material))

# Latitude rings include the two lips and recessed floor of the equatorial trench.
latitudes = sorted(set([math.pi*i/96 for i in range(97)] + [math.pi/2+d for d in [-.025, -.018, .018, .025]]))
segments = 192
axis = (0, .42, math.sqrt(1-.42**2))
dish_angle = .34
for row, theta in enumerate(latitudes):
    radius = .981 if abs(theta - math.pi/2) <= .0181 else 1
    for col in range(segments + 1):
        phi = col / segments * math.tau
        p = (math.sin(theta)*math.sin(phi), math.cos(theta), math.sin(theta)*math.cos(phi))
        vertex(tuple(c*radius for c in p), (col/segments, 1-theta/math.pi))
for row in range(len(latitudes) - 1):
    for col in range(segments):
        a = row*(segments+1)+col+1
        for tri in [(a, a+segments+1, a+1), (a+1, a+segments+1, a+segments+2)]:
            center = [sum(vertices[i-1][j] for i in tri)/3 for j in range(3)]
            if sum(center[j]*axis[j] for j in range(3)) < math.cos(dish_angle):
                face(*tri, "hull")

# A real concave focusing dish: concentric stepped rings, radial ribs, central well.
tangent = (0, axis[2], -axis[1])
start = len(vertices) + 1
ring_count, dish_segments = 28, 160
for ring in range(ring_count + 1):
    radial = math.sin(dish_angle)*ring/ring_count
    depth = .76 + .18*(ring/ring_count)**2
    if ring >= ring_count-2:
        depth = .945 + .006*(ring-ring_count+2)
    for col in range(dish_segments+1):
        phi = math.tau*col/dish_segments
        vertex(tuple(axis[j]*depth + radial*((1 if j == 0 else 0)*math.cos(phi)+tangent[j]*math.sin(phi)) for j in range(3)))
for ring in range(ring_count):
    for col in range(dish_segments):
        a = start + ring*(dish_segments+1)+col
        material = "rim" if ring >= ring_count-3 else ("rib" if col%20 == 0 or ring%5 == 0 else "dish")
        if ring < 3:
            material = "well"
        face(a, a+1, a+dish_segments+1, material)
        face(a+1, a+dish_segments+2, a+dish_segments+1, material)

with (ROOT / "death-star.obj").open("w", newline="\n") as out:
    out.write("# Original procedural battle station\nmtllib death-star.mtl\n")
    for p in vertices:
        out.write("v %.6f %.6f %.6f\n" % p)
    for uv in uvs:
        out.write("vt %.6f %.6f\n" % uv)
    current = None
    for a, b, c, material in faces:
        if current != material:
            out.write(f"usemtl {material}\n")
            current = material
        out.write(f"f {a}/{a} {b}/{b} {c}/{c}\n")
(ROOT / "death-star.mtl").write_text("""newmtl hull
Kd 1 1 1
map_Kd panels.png
newmtl dish
Kd 0.31 0.35 0.36
newmtl rib
Kd 0.19 0.22 0.23
newmtl rim
Kd 0.60 0.64 0.65
newmtl well
Kd 0.055 0.075 0.06
""", encoding="utf-8")
print(f"Death Star: {len(vertices)} vertices, {len(faces)} triangles")

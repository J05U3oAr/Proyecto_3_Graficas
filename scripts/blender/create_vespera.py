"""Build Vespera in Blender: lavender snow dunes, mesas and branching canyons.

blender --background --factory-startup --python scripts/blender/create_vespera.py
No images are rendered or baked. Also runs in Blender's Text Editor.
"""
import argparse
import math
from pathlib import Path
import sys

import bpy
from mathutils import Vector

DIRECTORIO_SALIDA = ""
SEGMENTOS, ANILLOS = 160, 80
# Local footprints keep terrain formations independent of planetary latitude.
MESETAS = [
    ((-0.46, 0.38, 0.8), 0.25, 0.28, 0.13),
    ((0.51, 0.46, 0.73), 0.30, 0.19, 0.10),
    ((0.40, -0.56, 0.73), 0.22, 0.33, 0.12),
    ((-0.80, -0.40, 0.38), 0.27, 0.24, 0.11),
    ((0.22, 0.88, -0.43), 0.32, 0.26, 0.14),
    ((-0.66, 0.27, -0.70), 0.23, 0.35, 0.12),
    ((0.61, -0.46, -0.66), 0.29, 0.24, 0.13),
]
# Center, twist, half-length, width, depth. Overlapping branches join naturally.
CANONES = [
    ((0, 0.12, 1), -0.35, 0.85, 0.085, 0.18),
    ((0.22, -0.04, 0.98), 0.95, 0.52, 0.075, 0.16),
    ((-0.24, 0.34, 0.91), -1.00, 0.43, 0.070, 0.15),
    ((-0.77, 0.20, -0.61), 0.45, 0.65, 0.090, 0.17),
    ((0.20, -0.57, -0.80), -0.75, 0.59, 0.080, 0.16),
]
DUNAS = [(0.55, 0.35, 0.76), (-0.45, -0.55, 0.70), (-0.55, 0.45, -0.70), (0.65, -0.30, -0.69)]
COLORES = [(0.66, 0.64, 0.79), (0.83, 0.80, 0.90), (0.40, 0.35, 0.50), (0.25, 0.22, 0.34)]
NOMBRES = ["Nieve lavanda", "Cimas escarchadas", "Estratos erosionados", "Fondo de canon"]


def smooth(a, b, v):
    t = max(0.0, min(1.0, (v - a) / (b - a)))
    return t * t * (3 - 2 * t)


def frame(center, twist=0):
    center = Vector(center).normalized()
    tangent = center.cross(Vector((0, 1, 0))).normalized()
    other = tangent.cross(center).normalized()
    right = tangent * math.cos(twist) + other * math.sin(twist)
    return center, right, right.cross(center)


def local(direction, basis):
    center, right, along = basis
    facing = direction.dot(center)
    if facing < 0.65:
        return None
    return direction.dot(right) / facing, direction.dot(along) / facing


def features():
    mesas = [(frame(center), w, length, h) for center, w, length, h in MESETAS]
    canyons = [(frame(center, twist), length, w, depth) for center, twist, length, w, depth in CANONES]
    dunes = [frame(center, i * 0.9) for i, center in enumerate(DUNAS)]
    return mesas, canyons, dunes


def elevation(direction, terrain):
    mesas, canyons, dunes = terrain
    plateau, drift, cut = 0.0, 0.0, 0.0
    for basis, width, length, height in mesas:
        point = local(direction, basis)
        if point is None:
            continue
        x, y = point
        # Eroded, rounded outlines with wide, nearly level tops.
        q = (x / width) ** 2 + (y / length) ** 2
        q += 0.08 * math.sin(x * 35 + y * 16)
        plateau = max(plateau, height * (1 - smooth(0.48, 1.0, q)))
    for basis in dunes:
        point = local(direction, basis)
        if point is None:
            continue
        x, y = point
        mask = 1 - smooth(0.3, 1.0, (x / 0.52) ** 2 + (y / 0.64) ** 2)
        wave = (0.5 + 0.5 * math.sin(x * 27 + math.sin(y * 9) * 1.7)) ** 2
        drift += mask * wave * 0.028
    for basis, length, width, depth in canyons:
        point = local(direction, basis)
        if point is None:
            continue
        x, y = point
        centerline = math.sin(y * 6) * 0.065
        channel = 1 - smooth(0.35, 1, abs(x - centerline) / width)
        ends = 1 - smooth(length * 0.65, length, abs(y))
        cut = max(cut, depth * channel * ends)
    return 1.0 + plateau + drift - cut, plateau, cut


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p / "Cargo.toml").is_file()), None)
    default = root / "assets/planets/vespera" if root else Path.home() / "Vespera"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    terrain = features()
    vertices, attributes, faces, ids = [], [], [], []
    for lat in range(ANILLOS + 1):
        phi = math.pi * lat / ANILLOS
        for lon in range(SEGMENTOS):
            theta = math.tau * lon / SEGMENTOS
            direction = Vector((math.sin(phi) * math.cos(theta), math.cos(phi), math.sin(phi) * math.sin(theta)))
            radius, plateau, cut = elevation(direction, terrain)
            vertices.append(direction * radius)
            attributes.append((plateau, cut))

    def add(indices):
        a, b, c = (vertices[i] for i in indices)
        point = (a + b + c) / 3
        normal = (b - a).cross(c - a)
        if normal.dot(point) < 0:
            indices = (indices[0], indices[2], indices[1])
        _, plateau, cut = elevation(point.normalized(), terrain)
        slope = abs(normal.normalized().dot(point.normalized()))
        material = 3 if cut > 0.12 else 2 if slope < 0.83 else 1 if plateau > 0.06 else 0
        faces.append(indices)
        ids.append(material)

    for lat in range(ANILLOS):
        for lon in range(SEGMENTOS):
            nxt = (lon + 1) % SEGMENTOS
            a, b = lat * SEGMENTOS + lon, lat * SEGMENTOS + nxt
            c, d = a + SEGMENTOS, b + SEGMENTOS
            if lat > 0:
                add((a, b, c))
            if lat < ANILLOS - 1:
                add((b, d, c))
    scene = bpy.data.scenes.new("Vespera | desierto de nieve lavanda")
    bpy.context.window.scene = scene
    mesh = bpy.data.meshes.new("Vespera | dunas, mesetas y canones")
    mesh.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], faces)
    mesh.update()
    for name, color in zip(NOMBRES, COLORES):
        material = bpy.data.materials.new(name)
        material.diffuse_color = (*color, 1)
        material.use_nodes = True
        shader = material.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*color, 1)
        shader.inputs["Roughness"].default_value = 0.92
        mesh.materials.append(material)
    for polygon, material in zip(mesh.polygons, ids):
        polygon.material_index = material
    planet = bpy.data.objects.new("Vespera", mesh)
    scene.collection.objects.link(planet)
    for name, selection in [("Mesetas", [i for i, (p, _) in enumerate(attributes) if p > 0.05]),
                            ("Canones", [i for i, (_, c) in enumerate(attributes) if c > 0.05])]:
        group = planet.vertex_groups.new(name=name)
        group.add(selection, 1.0, "REPLACE")
    planet["diseno"] = "7 mesetas aisladas, 5 cauces ramificados, 4 campos de dunas"
    planet["atmosfera"] = "Nieve tenue y viento calculados por el renderizador CPU"
    bpy.context.view_layer.objects.active = planet
    planet.select_set(True)
    with (output / "vespera.obj").open("w", encoding="utf-8") as file:
        file.write("# Vespera generado en Blender; Y arriba\nmtllib vespera.mtl\no Vespera\n")
        for v in mesh.vertices:
            p = v.co
            file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
        current = None
        for polygon in mesh.polygons:
            if polygon.material_index != current:
                current = polygon.material_index
                file.write(f"usemtl material_{current}\n")
            file.write("f " + " ".join(str(i + 1) for i in polygon.vertices) + "\n")
    (output / "vespera.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nillum 2\n" for i, (r, g, b) in enumerate(COLORES)), encoding="utf-8")
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "vespera.blend"), compress=True)
    print(f"VESPERA: {len(vertices)} vertices, {len(faces)} triangulos; {output}", flush=True)


if __name__ == "__main__":
    main()

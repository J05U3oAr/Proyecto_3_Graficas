"""Create Onice in Blender: dark impact basins, blue frost and traversable arches.

blender --background --factory-startup --python scripts/blender/create_onice.py
No images are rendered or baked. Can also run from Blender's Text Editor.
"""
import argparse
import math
from pathlib import Path
import sys

import bpy
import bmesh
from mathutils import Vector

DIRECTORIO_SALIDA = ""
SEGMENTOS, ANILLOS = 160, 80
CRATERES = [
    ((0.05, 0.18, 1), 0.36, 0.13),
    ((-0.58, 0.46, 0.65), 0.24, 0.10),
    ((0.61, -0.25, 0.75), 0.27, 0.11),
    ((-0.35, -0.70, 0.61), 0.20, 0.08),
    ((0.20, 0.90, -0.38), 0.30, 0.12),
    ((-0.80, 0.10, -0.56), 0.33, 0.13),
    ((0.65, 0.35, -0.67), 0.22, 0.09),
    ((0.15, -0.64, -0.75), 0.29, 0.12),
    ((0.95, 0.25, 0.07), 0.14, 0.06),
]
# Crater index, orientation, half-span, height. Arches remain separate editable objects.
ARCOS = [(0, 0.35, 0.28, 0.27), (1, -0.6, 0.23, 0.22), (5, 0.8, 0.26, 0.25)]
COLORES = [(0.065, 0.085, 0.11), (0.13, 0.18, 0.23), (0.26, 0.43, 0.53),
           (0.09, 0.23, 0.30), (0.38, 0.57, 0.65)]
NOMBRES = ["Basalto negro", "Roca fracturada", "Escarcha azul", "Hielo de impacto", "Cresta helada"]


def smooth(a, b, v):
    t = max(0.0, min(1.0, (v - a) / (b - a)))
    return t * t * (3 - 2 * t)


def detail(p):
    return math.sin(p.x * 23 + math.sin(p.z * 17)) * math.sin(p.y * 19 - p.z * 11)


def craters():
    return [(Vector(center).normalized(), radius, depth) for center, radius, depth in CRATERES]


def terrain(direction, impacts):
    height = 1.0 + detail(direction) * 0.007
    floor, rim = 0.0, 0.0
    for center, radius, depth in impacts:
        angle = math.acos(max(-1.0, min(1.0, direction.dot(center))))
        t = angle / radius
        if t > 1.4:
            continue
        bowl = 1.0 - smooth(0.12, 0.92, t)
        lip = math.exp(-((t - 0.98) / 0.16) ** 2)
        height -= depth * bowl
        height += 0.055 * lip
        floor, rim = max(floor, bowl), max(rim, lip)
    return height, floor, rim


def frame(center, twist):
    tangent = center.cross(Vector((0, 1, 0))).normalized()
    other = tangent.cross(center).normalized()
    right = tangent * math.cos(twist) + other * math.sin(twist)
    return right, right.cross(center)


def make_object(scene, name, vertices, faces, material_ids, materials):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], faces)
    for mat in materials:
        mesh.materials.append(mat)
    for polygon, material in zip(mesh.polygons, material_ids):
        polygon.material_index = material
    # Closed crust and arches have outward normals, including the inner vault.
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    scene.collection.objects.link(obj)
    return obj


def crust(scene, impacts, materials):
    vertices, faces, ids = [], [], []
    for lat in range(ANILLOS + 1):
        phi = math.pi * lat / ANILLOS
        for lon in range(SEGMENTOS):
            theta = math.tau * lon / SEGMENTOS
            d = Vector((math.sin(phi) * math.cos(theta), math.cos(phi), math.sin(phi) * math.sin(theta)))
            vertices.append(d * terrain(d, impacts)[0])

    def add(indices):
        a, b, c = (vertices[i] for i in indices)
        point = (a + b + c) / 3
        direction = point.normalized()
        normal = (b - a).cross(c - a)
        _, floor, rim = terrain(direction, impacts)
        slope = abs(normal.normalized().dot(direction))
        ids.append(3 if floor > 0.65 else 4 if rim > 0.68 else 2 if floor > 0.28 else 1 if slope < 0.83 else 0)
        faces.append(indices)

    for lat in range(ANILLOS):
        for lon in range(SEGMENTOS):
            nxt = (lon + 1) % SEGMENTOS
            a, b = lat * SEGMENTOS + lon, lat * SEGMENTOS + nxt
            c, d = a + SEGMENTOS, b + SEGMENTOS
            if lat > 0:
                add((a, b, c))
            if lat < ANILLOS - 1:
                add((b, d, c))
    return make_object(scene, "Onice | corteza y crateres", vertices, faces, ids, materials)


def arch(scene, index, impact, twist, span, height, impacts, materials):
    center = impacts[impact][0]
    right, forward = frame(center, twist)
    base = terrain(center, impacts)[0] + 0.005
    vertices, faces, ids = [], [], []
    for step in range(33):
        angle = math.pi * step / 32
        uneven = 0.004 * math.sin(angle * 7)
        for xradius, yradius, thickness in [(span, height, -0.045), (span, height, 0.045),
                                           (span - 0.06, height - 0.06, 0.045),
                                           (span - 0.06, height - 0.06, -0.045)]:
            x = xradius * math.cos(angle)
            y = yradius * math.sin(angle) + uneven
            vertices.append(center * (base + y) + right * x + forward * thickness)
    for step in range(32):
        for side in range(4):
            a, b = step * 4 + side, step * 4 + (side + 1) % 4
            faces.append((a, b, b + 4, a + 4))
            ids.append(2 if side == 0 else 1)
    faces.extend([(3, 2, 1, 0), (128, 129, 130, 131)])
    ids.extend([0, 0])
    obj = make_object(scene, f"Onice | arco natural {index + 1:02}", vertices, faces, ids, materials)
    obj["paso"] = "Boveda abierta y colision real en las paredes; no rellenar el hueco"
    return obj


def export(objects, output):
    offset, triangles = 0, 0
    with (output / "onice.obj").open("w", encoding="utf-8") as file:
        file.write("# Onice generado en Blender; Y arriba; arcos abiertos\nmtllib onice.mtl\n")
        for obj in objects:
            mesh = obj.data
            mesh.calc_loop_triangles()
            file.write(f"o {obj.name.replace(' ', '_')}\n")
            for vertex in mesh.vertices:
                p = obj.matrix_world @ vertex.co
                file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
            current = None
            for face in mesh.loop_triangles:
                if face.material_index != current:
                    current = face.material_index
                    file.write(f"usemtl material_{current}\n")
                file.write("f " + " ".join(str(i + 1 + offset) for i in face.vertices) + "\n")
                triangles += 1
            offset += len(mesh.vertices)
    (output / "onice.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nillum 2\n" for i, (r, g, b) in enumerate(COLORES)), encoding="utf-8")
    return offset, triangles


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p / "Cargo.toml").is_file()), None)
    default = root / "assets/planets/onice" if root else Path.home() / "Onice"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes.new("Onice | mundo de basalto y escarcha")
    bpy.context.window.scene = scene
    materials = []
    for name, color in zip(NOMBRES, COLORES):
        material = bpy.data.materials.new(name)
        material.diffuse_color = (*color, 1)
        material.use_nodes = True
        shader = material.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*color, 1)
        shader.inputs["Roughness"].default_value = 0.72
        materials.append(material)
    impacts = craters()
    objects = [crust(scene, impacts, materials)]
    objects.extend(arch(scene, i, *params, impacts, materials) for i, params in enumerate(ARCOS))
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    objects[0]["diseno"] = "9 cuencas de impacto; 3 arcos naturales; basalto oscuro y escarcha azul"
    bpy.context.view_layer.update()
    counts = export(objects, output)
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "onice.blend"), compress=True)
    print(f"ONICE: {counts[0]} vertices, {counts[1]} triangulos; {output}", flush=True)


if __name__ == "__main__":
    main()

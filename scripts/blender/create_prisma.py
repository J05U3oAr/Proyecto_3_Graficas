"""Create Prisma in Blender: dark plains and seeded blue/amethyst crystal groves.

blender --background --factory-startup --python scripts/blender/create_prisma.py
Only writes .blend, .obj and .mtl; no images are rendered or baked.
"""
import argparse
import math
from pathlib import Path
import random
import sys

import bpy
import bmesh
from mathutils import Vector

DIRECTORIO_SALIDA = ""
SEMILLA = 73191
MATERIALES = [
    ("Basalto frio", (0.075, 0.10, 0.15), 1.0),
    ("Escarcha de las llanuras", (0.19, 0.25, 0.32), 1.0),
    ("Nucleo azul", (0.12, 0.28, 0.38), 1.0),
    ("Nucleo amatista", (0.27, 0.17, 0.39), 1.0),
    ("Cristal azul translucido", (0.28, 0.67, 0.86), 0.34),
    ("Cristal amatista translucido", (0.57, 0.40, 0.79), 0.38),
]


def smooth(a, b, v):
    t = max(0.0, min(1.0, (v - a) / (b - a)))
    return t * t * (3 - 2 * t)


def hash_value(x, y, z):
    n = (x * 374761393 + y * 668265263 + z * 2147483647) & 0xffffffff
    n = ((n ^ (n >> 13)) * 1274126177) & 0xffffffff
    return (n ^ (n >> 16)) / 0xffffffff


def noise(p):
    cell = [math.floor(v) for v in p]
    t = [smooth(0, 1, p[i] - cell[i]) for i in range(3)]
    result = 0.0
    for z in range(2):
        for y in range(2):
            for x in range(2):
                weight = (t[0] if x else 1 - t[0]) * (t[1] if y else 1 - t[1]) * (t[2] if z else 1 - t[2])
                result += hash_value(cell[0] + x, cell[1] + y, cell[2] + z) * weight
    return result


def elevation(direction):
    return 1.0 + (noise(direction * 11 + Vector((17, 9, 31))) - 0.5) * 0.025


def make_mesh(scene, name, vertices, faces, ids, materials):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], faces)
    for mat in materials:
        mesh.materials.append(mat)
    for polygon, material in zip(mesh.polygons, ids):
        polygon.material_index = material
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    scene.collection.objects.link(obj)
    return obj


def crust(scene, materials):
    vertices, faces, ids = [], [], []
    for lat in range(65):
        phi = math.pi * lat / 64
        for lon in range(128):
            theta = math.tau * lon / 128
            direction = Vector((math.sin(phi) * math.cos(theta), math.cos(phi), math.sin(phi) * math.sin(theta)))
            vertices.append(direction * elevation(direction))
    for lat in range(64):
        for lon in range(128):
            a, b = lat * 128 + lon, lat * 128 + (lon + 1) % 128
            c, d = a + 128, b + 128
            triangles = ([] if lat == 0 else [(a, b, c)]) + ([] if lat == 63 else [(b, d, c)])
            for triangle in triangles:
                faces.append(triangle)
                direction = sum((vertices[i] for i in triangle), Vector()).normalized()
                ids.append(1 if noise(direction * 16 + Vector((5, 23, 7))) > 0.64 else 0)
    return make_mesh(scene, "Prisma | llanuras de basalto", vertices, faces, ids, materials)


def grove_centers():
    centers = []
    candidates = list(range(128))
    random.Random(SEMILLA).shuffle(candidates)
    for i in candidates:
        y = 1 - 2 * (i + 0.5) / 128
        angle = i * 2.39996323 + SEMILLA * 0.001
        r = math.sqrt(1 - y * y)
        direction = Vector((r * math.cos(angle), y, r * math.sin(angle)))
        density = noise(direction * 4 + Vector((SEMILLA * 0.013, 17, 29)))
        if density < 0.49 or any(direction.dot(c) > 0.84 for c in centers):
            continue
        centers.append(direction)
        if len(centers) == 12:
            break
    if len(centers) < 8:
        raise RuntimeError("La semilla no produjo suficientes grupos separados")
    return centers


def crystals(scene, materials):
    rng = random.Random(SEMILLA)
    objects = []
    for cluster, center in enumerate(grove_centers()):
        right = center.cross(Vector((0, 1, 0))).normalized()
        forward = right.cross(center)
        vertices, faces, ids = [], [], []
        for shard in range(5):
            angle = rng.random() * math.tau
            distance = 0 if shard == 0 else 0.17 + rng.random() * 0.08
            axis = (center + right * (math.cos(angle) * distance) + forward * (math.sin(angle) * distance)).normalized()
            anchor = axis * (elevation(axis) - 0.012)
            tangent = axis.cross(Vector((0, 1, 0))).normalized()
            side = tangent.cross(axis)
            width = (0.06 + rng.random() * 0.025) if shard == 0 else 0.025 + rng.random() * 0.025
            height = (0.32 + rng.random() * 0.10) if shard == 0 else 0.13 + rng.random() * 0.13
            lean = (tangent * math.cos(angle) + side * math.sin(angle)) * height * (0.20 + rng.random() * 0.30)
            spin = rng.random() * math.tau
            amethyst = (cluster + shard) % 3 == 0
            for shell in [False, True]:
                start = len(vertices)
                radius, h = (width, height) if shell else (width * 0.74, height * 0.86)
                for level, factor in [(0.0, 1.0), (0.76, 0.92)]:
                    for corner in range(6):
                        theta = spin + corner * math.tau / 6
                        vertices.append(anchor + axis * (h * level) + lean * (h / height * level)
                                        + (tangent * math.cos(theta) + side * math.sin(theta)) * radius * factor)
                vertices.append(anchor + axis * h + lean * (h / height))
                for corner in range(6):
                    nxt = (corner + 1) % 6
                    faces.extend([(start + corner, start + nxt, start + 6 + nxt, start + 6 + corner),
                                  (start + 6 + corner, start + 6 + nxt, start + 12)])
                    ids.extend([(5 if amethyst else 4) if shell else (3 if amethyst else 2)] * 2)
                faces.append(tuple(start + i for i in reversed(range(6))))
                ids.append((5 if amethyst else 4) if shell else (3 if amethyst else 2))
        obj = make_mesh(scene, f"Prisma | grupo cristalino {cluster + 1:02}", vertices, faces, ids, materials)
        obj["diseno"] = "5 cristales hexagonales inclinados; nucleo opaco y cubierta translucida"
        objects.append(obj)
    return objects


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p / "Cargo.toml").is_file()), None)
    default = root / "assets/planets/prisma" if root else Path.home() / "Prisma"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes.new("Prisma | jardines de cristal azul y amatista")
    bpy.context.window.scene = scene
    materials = []
    for name, rgb, opacity in MATERIALES:
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb, opacity)
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        bsdf.inputs["Base Color"].default_value = (*rgb, 1)
        bsdf.inputs["Roughness"].default_value = 0.18 if opacity < 1 else 0.72
        bsdf.inputs["Transmission Weight"].default_value = 0.55 if opacity < 1 else 0
        bsdf.inputs["IOR"].default_value = 1.46
        materials.append(mat)
    objects = [crust(scene, materials)] + crystals(scene, materials)
    offset, triangles = 0, 0
    with (output / "prisma.obj").open("w", encoding="utf-8") as file:
        file.write("# Prisma generado en Blender; Y arriba\nmtllib prisma.mtl\n")
        for obj in objects:
            obj.data.calc_loop_triangles()
            file.write(f"o {obj.name.replace(' ', '_')}\n")
            for vertex in obj.data.vertices:
                p = vertex.co
                file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
            current = None
            for triangle in obj.data.loop_triangles:
                if triangle.material_index != current:
                    current = triangle.material_index
                    file.write(f"usemtl material_{current}\n")
                file.write("f " + " ".join(str(i + 1 + offset) for i in triangle.vertices) + "\n")
                triangles += 1
            offset += len(obj.data.vertices)
    (output / "prisma.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nd {opacity}\nillum 2\n"
        for i, (_, (r, g, b), opacity) in enumerate(MATERIALES)), encoding="utf-8")
    bpy.context.view_layer.objects.active = objects[0]
    for obj in objects:
        obj.select_set(True)
    objects[0]["semilla_distribucion"] = SEMILLA
    objects[0]["distribucion"] = "Muestreo Fibonacci, ruido de valores 3D y separacion angular"
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "prisma.blend"), compress=True)
    print(f"PRISMA: {len(objects)-1} grupos, {offset} vertices, {triangles} triangulos; {output}", flush=True)


if __name__ == "__main__":
    main()

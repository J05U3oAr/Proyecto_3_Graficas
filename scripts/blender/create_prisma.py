"""Create Prisma in Blender: a crystalline crust and irregular mineral massifs.

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
    ("Cristal azul marino", (0.055, 0.12, 0.30), 1.0),
    ("Cristal azul de la corteza", (0.10, 0.32, 0.56), 1.0),
    ("Nucleo azul", (0.12, 0.28, 0.38), 1.0),
    ("Nucleo amatista", (0.27, 0.17, 0.39), 1.0),
    ("Cristal azul translucido", (0.28, 0.67, 0.86), 0.34),
    ("Cristal amatista translucido", (0.57, 0.40, 0.79), 0.38),
    ("Cristal purpura de la corteza", (0.31, 0.17, 0.48), 1.0),
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
    broad = noise(direction * 5 + Vector((17, 9, 31)))
    return 0.98 + round(broad * 5) / 5 * 0.045


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
    ico = bmesh.new()
    bmesh.ops.create_icosphere(ico, subdivisions=5, radius=1)
    ico.verts.ensure_lookup_table()
    ico.verts.index_update()
    for vertex in ico.verts:
        direction = vertex.co.normalized()
        vertices.append(direction * elevation(direction))
    for face in ico.faces:
        triangle = tuple(v.index for v in face.verts)
        faces.append(triangle)
        direction = sum((vertices[i] for i in triangle), Vector()).normalized()
        hue = noise(direction * 3.2 + Vector((5, 23, 7)))
        ids.append(0 if hue < 0.40 else 6 if hue > 0.57 else 1)
    ico.free()
    return make_mesh(scene, "Prisma | corteza cristalina azul marino y purpura", vertices, faces, ids, materials)


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
        if density < 0.49 or any(direction.dot(c) > 0.78 for c in centers):
            continue
        centers.append(direction)
        if len(centers) == 10:
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
        count = rng.randint(5, 7)
        for shard in range(count):
            angle = rng.random() * math.tau
            distance = 0 if shard == 0 else 0.12 + rng.random() * 0.12
            axis = (center + right * (math.cos(angle) * distance) + forward * (math.sin(angle) * distance)).normalized()
            anchor = axis * (elevation(axis) - 0.025)
            tangent = axis.cross(Vector((0, 1, 0))).normalized()
            side = tangent.cross(axis)
            width = (0.16 + rng.random() * 0.08) if shard == 0 else 0.055 + rng.random() * 0.065
            height = (0.27 + rng.random() * 0.13) if shard == 0 else 0.10 + rng.random() * 0.16
            lean = (tangent * math.cos(angle) + side * math.sin(angle)) * height * (0.15 + rng.random() * 0.55)
            spin = rng.random() * math.tau
            corners = rng.choice([5, 6, 7, 8])
            outline = [(spin + i * math.tau / corners + rng.uniform(-0.10, 0.10),
                        rng.uniform(0.75, 1.18)) for i in range(corners)]
            elongation = rng.uniform(0.55, 1.1)
            outer = []
            # Bulging roots, uneven fracture shoulders, two unequal terminal peaks.
            for ring in range(3):
                for theta, radius in outline:
                    level = 0.0 if ring == 0 else rng.uniform(0.27, 0.43) if ring == 1 else rng.uniform(0.57, 0.78)
                    taper = 0.92 if ring == 0 else rng.uniform(1.0, 1.18) if ring == 1 else rng.uniform(0.60, 0.90)
                    lateral = tangent * math.cos(theta) + side * (math.sin(theta) * elongation)
                    outer.append(anchor + axis * (height * level) + lean * level + lateral * width * radius * taper)
            outer.extend([anchor + axis * height + lean - tangent * width * 0.17,
                          anchor + axis * (height * rng.uniform(0.80, 0.94)) + lean * 0.8 + tangent * width * 0.32])
            amethyst = (cluster + shard) % 3 == 0
            for shell in [False, True]:
                start = len(vertices)
                factor = 1.0 if shell else 0.79
                vertices.extend(anchor + (v - anchor) * factor for v in outer)
                material = (5 if amethyst else 4) if shell else (3 if amethyst else 2)
                for ring in range(2):
                    for corner in range(corners):
                        nxt = (corner + 1) % corners
                        faces.append((start + ring * corners + corner, start + ring * corners + nxt,
                                      start + (ring + 1) * corners + nxt, start + (ring + 1) * corners + corner))
                        ids.append(material)
                top = start + 3 * corners
                split = corners // 2
                for corner in range(corners):
                    nxt = (corner + 1) % corners
                    tip = top if corner < split else top + 1
                    faces.append((start + 2 * corners + corner, start + 2 * corners + nxt, tip))
                    ids.append(material)
                faces.extend([(start + 2 * corners, top, top + 1),
                              (start + 2 * corners + split, top + 1, top),
                              tuple(start + i for i in reversed(range(corners)))])
                ids.extend([material] * 3)
        obj = make_mesh(scene, f"Prisma | macizo mineral {cluster + 1:02}", vertices, faces, ids, materials)
        obj["diseno"] = "Crecimientos anchos entrelazados, fracturas irregulares y terminaciones partidas"
        obj["crecimientos"] = count
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
    scene = bpy.data.scenes.new("Prisma | mundo de cristal azul marino y purpura")
    bpy.context.window.scene = scene
    materials = []
    for name, rgb, opacity in MATERIALES:
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb, opacity)
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        bsdf.inputs["Base Color"].default_value = (*rgb, 1)
        bsdf.inputs["Roughness"].default_value = 0.12 if opacity < 1 else 0.22
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

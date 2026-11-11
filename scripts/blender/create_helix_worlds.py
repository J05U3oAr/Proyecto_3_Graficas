"""Model three original Helice worlds in Blender, with real mountains and craters.

blender --background --factory-startup --python-exit-code 1 \
  --python scripts/blender/create_helix_worlds.py -- --overwrite

All terrain and vegetation are editable meshes; no downloaded assets or textures.
Game-space coordinates use Y up. Blender coordinates use Z up.
"""
import argparse
import json
import math
from pathlib import Path
import random
import sys

import bpy
import bmesh
from mathutils import Vector, noise

ROOT = Path(__file__).resolve().parents[2]
WORLDS = {
    "micelia": {
        "seed": 407, "title": "Micelia | bosque de esporas",
        "colors": [(0.17, 0.10, 0.25), (0.29, 0.16, 0.38), (0.40, 0.25, 0.44),
                   (0.11, 0.30, 0.28), (0.19, 0.52, 0.40), (0.57, 0.85, 0.67),
                   (0.60, 0.43, 0.69), (0.82, 0.66, 0.75)],
        "names": ["Humus violeta", "Roca malva", "Cresta lila", "Micelio oscuro",
                  "Micelio esmeralda", "Esporas luminosas", "Sombrero ciruela", "Tallo marfil"],
        "impacts": [((0.25, 0.28, 1), 0.40, 0.16), ((-0.64, -0.28, 0.8), 0.25, 0.10),
                    ((0.8, 0.48, -0.5), 0.32, 0.12), ((-0.5, 0.7, -0.6), 0.28, 0.12),
                    ((0.15, -0.8, -0.6), 0.34, 0.13)],
    },
    "azufria": {
        "seed": 821, "title": "Azufria | calderas de azufre",
        "colors": [(0.45, 0.31, 0.09), (0.66, 0.51, 0.14), (0.90, 0.76, 0.25),
                   (0.98, 0.88, 0.45), (0.21, 0.35, 0.16), (0.27, 0.68, 0.46),
                   (0.10, 0.26, 0.22), (0.75, 0.56, 0.31)],
        "names": ["Escoria ocre", "Azufre viejo", "Azufre amarillo", "Cresta de limon",
                  "Sedimento acido", "Acido jade", "Fondo de caldera", "Estrato mineral"],
        "impacts": [((0.12, 0.15, 1), 0.33, 0.22), ((-0.63, 0.55, 0.7), 0.28, 0.18),
                    ((0.7, -0.4, 0.6), 0.31, 0.20), ((-0.3, -0.8, 0.4), 0.22, 0.14),
                    ((0.4, 0.6, -0.8), 0.35, 0.20), ((-0.8, -0.2, -0.5), 0.26, 0.17),
                    ((0.5, -0.7, -0.6), 0.23, 0.16)],
    },
    "ferrita": {
        "seed": 1231, "title": "Ferrita | cordilleras de cobre",
        "colors": [(0.30, 0.13, 0.08), (0.52, 0.24, 0.12), (0.73, 0.41, 0.23),
                   (0.88, 0.60, 0.37), (0.09, 0.25, 0.24), (0.18, 0.43, 0.37),
                   (0.34, 0.61, 0.51), (0.22, 0.24, 0.27)],
        "names": ["Hierro oscuro", "Oxido rojizo", "Cobre", "Cresta metalica",
                  "Patina profunda", "Cardenillo", "Patina clara", "Niquel de impacto"],
        "impacts": [((0.16, 0.25, 1), 0.36, 0.20), ((-0.67, -0.2, 0.8), 0.27, 0.15),
                    ((0.6, 0.6, 0.6), 0.23, 0.13), ((-0.35, 0.8, -0.4), 0.30, 0.17),
                    ((0.3, -0.75, 0.4), 0.21, 0.13), ((-0.7, -0.25, -0.6), 0.32, 0.18),
                    ((0.6, 0.15, -0.8), 0.26, 0.15), ((0.15, -0.7, -0.8), 0.24, 0.13)],
    },
}


def smooth(a, b, value):
    t = max(0.0, min(1.0, (value - a) / (b - a)))
    return t * t * (3.0 - 2.0 * t)


def to_blender(p):
    return (p.x, -p.z, p.y)


def fbm(p):
    return sum(noise.noise(p * frequency, noise_basis="PERLIN_ORIGINAL") * weight
               for frequency, weight in [(1, 0.55), (2.1, 0.27), (4.3, 0.13), (8.7, 0.05)])


def terrain(d, name, impacts, seed):
    p = d * 3.0 + Vector((seed * 0.013, 7.3, -2.1))
    broad = fbm(p)
    folded = 1.0 - abs(noise.noise(p * 2.0, noise_basis="PERLIN_ORIGINAL"))
    # Long continuous ranges, with broken subsidiary ridges rather than isolated spikes.
    belt = math.exp(-((d.y + 0.32 * math.sin(math.atan2(d.z, d.x) * 3.0)) / 0.24) ** 2)
    mountain = belt * (0.055 + 0.15 * folded ** 3)
    h = 1.0 + broad * 0.06 + mountain
    floor, rim = 0.0, 0.0
    for center, radius, depth in impacts:
        t = (d - center).length / radius
        if t >= 1.55:
            continue
        bowl = 1.0 - smooth(0.12, 0.94, t)
        lip = math.exp(-((t - 0.98) / 0.17) ** 2)
        # Clear the mountain range inside each basin; leave its raised impact rim.
        h -= mountain * bowl + depth * bowl
        h += (0.065 if name != "azufria" else 0.10) * lip
        if name == "ferrita":
            h += 0.033 * math.exp(-((t - 1.27) / 0.085) ** 2)
            h += 0.065 * math.exp(-(t / 0.17) ** 2)  # central rebound peak
        floor, rim = max(floor, bowl), max(rim, lip)
    return h, floor, rim, broad, mountain


def material_index(name, point, impacts, seed):
    d = point.normalized()
    h, floor, rim, broad, mountain = terrain(d, name, impacts, seed)
    strata = math.sin(h * 125.0 + d.y * 13.0 + broad * 3.0)
    if name == "micelia":
        return 4 if floor > 0.65 else 3 if floor > 0.25 else 2 if mountain > 0.12 or rim > 0.65 else 1 if broad > 0 else 0
    if name == "azufria":
        return 6 if floor > 0.80 else 4 if floor > 0.38 else 3 if rim > 0.52 else 2 if mountain > 0.08 else 7 if strata > 0.65 else 1 if broad > -0.12 else 0
    return 7 if floor > 0.8 else 4 if floor > 0.4 else 6 if rim > 0.6 else 3 if mountain > 0.14 else 5 if broad > 0.13 else 2 if strata > 0.3 else 1 if strata > -0.5 else 0


def make_mesh(scene, name, vertices, faces, ids, materials):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([to_blender(p) for p in vertices], [], faces)
    for mat in materials:
        mesh.materials.append(mat)
    for face, mat_id in zip(mesh.polygons, ids):
        face.material_index = mat_id
        face.use_smooth = True
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    scene.collection.objects.link(obj)
    return obj


def crust(scene, name, impacts, spec, materials):
    # Uniform triangles avoid pinched poles and give every crater comparable detail.
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=6, radius=1.0)
    bm.verts.ensure_lookup_table()
    vertices = []
    for vertex in bm.verts:
        d = vertex.co.normalized()
        vertices.append(d * terrain(d, name, impacts, spec["seed"])[0])
    faces = [tuple(v.index for v in face.verts) for face in bm.faces]
    bm.free()
    ids = [material_index(name, sum((vertices[i] for i in f), Vector()) / 3,
                          impacts, spec["seed"]) for f in faces]
    return make_mesh(scene, f"{name} | cordilleras y crateres", vertices, faces, ids, materials)


def frame(d):
    axis = Vector((0, 1, 0)) if abs(d.y) < 0.9 else Vector((1, 0, 0))
    right = d.cross(axis).normalized()
    return right, right.cross(d).normalized()


def acid_lake(scene, index, impact, materials):
    center, radius, depth = impact
    right, forward = frame(center)
    # A level disk sits inside the actual depressed caldera. Its edge intersects
    # the bowl walls, so there is neither a floating shoreline nor a hidden sphere.
    level = 1.0 - depth + 0.035
    spread = radius * 0.43
    vertices = [center * level]
    for i in range(49):
        a = math.tau * i / 48
        vertices.append(center * level + (right * math.cos(a) + forward * math.sin(a)) * spread)
    faces = [(0, i + 1, i + 2) for i in range(48)]
    return make_mesh(scene, f"Azufria | lago acido {index + 1:02}", vertices, faces, [5] * len(faces), materials)


def mushroom(scene, index, direction, size, height, materials):
    right, forward = frame(direction)
    vertices, faces, ids = [], [], []
    # Closed profile: buried foot, tapered stalk, luminous gills, rounded umbrella.
    profile = [(0.018, -0.018, 7), (0.014, 0.04, 7), (0.011, 0.09, 7),
               (0.060, 0.085, 5), (0.082, 0.100, 6), (0.066, 0.135, 6),
               (0.037, 0.156, 6), (0.001, 0.165, 6)]
    segments = 14
    for radius, elevation, _ in profile:
        for i in range(segments):
            angle = math.tau * i / segments
            vertices.append(direction * (height + elevation * size)
                            + (right * math.cos(angle) + forward * math.sin(angle)) * radius * size)
    for ring in range(len(profile) - 1):
        for i in range(segments):
            j = (i + 1) % segments
            faces.append((ring * segments + i, ring * segments + j,
                          (ring + 1) * segments + j, (ring + 1) * segments + i))
            ids.append(profile[ring + 1][2])
    faces.extend([tuple(reversed(range(segments))),
                  tuple((len(profile) - 1) * segments + i for i in range(segments))])
    ids.extend([7, 6])
    return make_mesh(scene, f"Micelia | hongo gigante {index + 1:02}", vertices, faces, ids, materials)


def export(objects, output, name, spec):
    offset, triangles = 0, 0
    with (output / f"{name}.obj").open("w", encoding="utf-8", newline="\n") as file:
        file.write(f"# Original Blender meshes, Y up; mountains and impact bowls are geometry\nmtllib {name}.mtl\n")
        for obj in objects:
            mesh = obj.data
            mesh.calc_loop_triangles()
            file.write(f"o {obj.name.replace(' ', '_')}\n")
            for vertex in mesh.vertices:
                p = obj.matrix_world @ vertex.co
                file.write(f"v {p.x:.7f} {p.z:.7f} {-p.y:.7f}\n")
            current = None
            for face in mesh.loop_triangles:
                if face.material_index != current:
                    current = face.material_index
                    file.write(f"usemtl material_{current}\n")
                file.write("f " + " ".join(str(i + offset + 1) for i in face.vertices) + "\n")
                triangles += 1
            offset += len(mesh.vertices)
    with (output / f"{name}.mtl").open("w", encoding="utf-8", newline="\n") as file:
        for i, (r, g, b) in enumerate(spec["colors"]):
            unlit = name == "micelia" and i == 5
            file.write(f"# {spec['names'][i]}\nnewmtl material_{i}\nKd {r} {g} {b}\nillum {0 if unlit else 2}\n\n")
    stats = {"world": name, "seed": spec["seed"], "vertices": offset,
             "triangles": triangles, "craters": len(spec["impacts"]),
             "objects": [obj.name for obj in objects], "textures": 0}
    (output / "mesh_stats.json").write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    return stats


def generate(name, output, overwrite):
    spec = WORLDS[name]
    output = output / name
    if not overwrite and any((output / f"{name}.{ext}").exists() for ext in ("blend", "obj", "mtl")):
        raise FileExistsError(f"{output}: use --overwrite to replace generated worlds")
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes.new(spec["title"])
    bpy.context.window.scene = scene
    materials = []
    for i, (title, color) in enumerate(zip(spec["names"], spec["colors"])):
        mat = bpy.data.materials.new(title)
        mat.diffuse_color = (*color, 1)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*color, 1)
        shader.inputs["Roughness"].default_value = 0.58 if name == "ferrita" else 0.85
        shader.inputs["Metallic"].default_value = 0.55 if name == "ferrita" else 0.0
        if name == "micelia" and i == 5:
            shader.inputs["Emission Color"].default_value = (*color, 1)
            shader.inputs["Emission Strength"].default_value = 1.5
        materials.append(mat)
    impacts = [(Vector(d).normalized(), radius, depth) for d, radius, depth in spec["impacts"]]
    objects = [crust(scene, name, impacts, spec, materials)]
    if name == "azufria":
        objects.extend(acid_lake(scene, i, impact, materials) for i, impact in enumerate(impacts))
    if name == "micelia":
        rng = random.Random(spec["seed"])
        placed = []
        for _ in range(2000):
            d = Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1)))
            if d.length < 0.1:
                continue
            d.normalize()
            h, floor, _, _, mountain = terrain(d, name, impacts, spec["seed"])
            if floor > 0.25 or mountain > 0.13 or any((d - p).length < 0.14 for p in placed):
                continue
            objects.append(mushroom(scene, len(placed), d, rng.uniform(0.65, 1.35), h - 0.004, materials))
            placed.append(d)
            if len(placed) == 58:
                break
    objects[0]["terrain"] = "Cordilleras continuas y cuencas de impacto modeladas; colision con toda la malla"
    objects[0]["seed"] = spec["seed"]
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.context.view_layer.update()
    stats = export(objects, output, name, spec)
    # Keep both generator and a self-contained editable scene with every material.
    text = bpy.data.texts.new("create_helix_worlds.py")
    text.write(Path(__file__).read_text(encoding="utf-8"))
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.6
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / f"{name}.blend"), compress=True)
    print(json.dumps(stats), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--only", choices=list(WORLDS))
    parser.add_argument("--output", type=Path, default=ROOT / "assets/planets/helix")
    parser.add_argument("--overwrite", action="store_true")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    for name in [args.only] if args.only else WORLDS:
        generate(name, args.output.resolve(), args.overwrite)


if __name__ == "__main__":
    main()

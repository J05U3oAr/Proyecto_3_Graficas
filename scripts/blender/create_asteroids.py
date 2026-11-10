"""Generate six editable asteroid meshes and their distance LODs in Blender.

blender --background --factory-startup --python scripts/blender/create_asteroids.py
No image textures, renders or external Python modules are required.
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
VARIANTES = [
    ("pequeno_fragmento", 3, (1.0, 0.58, 0.75), 0, 80),
    ("pequeno_erosionado", 3, (0.74, 0.93, 0.68), 2, 80),
    ("mediano_alargado", 4, (0.63, 1.0, 0.72), 0, 160),
    ("mediano_crateres", 4, (0.88, 0.71, 1.0), 4, 160),
    ("grande_macizo", 5, (0.78, 1.0, 0.69), 7, 320),
    ("grande_fracturado", 5, (1.0, 0.64, 0.81), 5, 320),
]
COLORES = [(0.22, 0.24, 0.27), (0.28, 0.29, 0.31),
           (0.15, 0.17, 0.20), (0.32, 0.33, 0.35)]


def smooth(a, b, x):
    t = max(0.0, min(1.0, (x-a)/(b-a)))
    return t*t*(3-2*t)


def noise(p):
    cell = [math.floor(v) for v in p]
    fade = [smooth(0, 1, p[i]-cell[i]) for i in range(3)]
    def value(x, y, z):
        h = ((x*374761393+y*668265263+z*2147483647) & 0xffffffff)
        h = ((h ^ (h >> 13))*1274126177) & 0xffffffff
        return ((h ^ (h >> 16)) & 0xffffffff)/0xffffffff
    total = 0.0
    for x in range(2):
        for y in range(2):
            for z in range(2):
                weight = (fade[0] if x else 1-fade[0])*(fade[1] if y else 1-fade[1])*(fade[2] if z else 1-fade[2])
                total += value(cell[0]+x, cell[1]+y, cell[2]+z)*weight
    return total


def export(obj, path):
    mesh = obj.data
    mesh.calc_loop_triangles()
    with path.open("w", encoding="utf-8") as f:
        f.write(f"# Blender asteroid, Y up\nmtllib asteroides.mtl\no {obj.name}\n")
        for v in mesh.vertices:
            p = v.co
            f.write(f"v {p.x:.7f} {p.z:.7f} {-p.y:.7f}\n")
        material = -1
        for face in sorted(mesh.loop_triangles, key=lambda face: tuple(face.vertices)):
            index = mesh.polygons[face.polygon_index].material_index
            if index != material:
                f.write(f"usemtl roca_{index}\n")
                material = index
            f.write("f " + " ".join(str(v+1) for v in face.vertices) + "\n")
    return len(mesh.loop_triangles)


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd()]
    if bpy.data.filepath:
        candidates.append(Path(bpy.data.filepath))
    root = next((p for c in candidates for p in [c, *c.parents]
                 if (p/"Cargo.toml").is_file()), None)
    default = root/"assets/asteroids" if root else Path.home()/"Asteroides"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--")+1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes.new("Asteroides | seis rocas independientes y LOD")
    bpy.context.window.scene = scene
    materials = []
    for i, rgb in enumerate(COLORES):
        mat = bpy.data.materials.new(f"Roca mate {i}")
        mat.diffuse_color = (*rgb, 1)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*rgb, 1)
        shader.inputs["Roughness"].default_value = 1.0
        materials.append(mat)
    for index, (name, subdivisions, axes, crater_count, lod_faces) in enumerate(VARIANTES):
        rng = random.Random(92371+index*137)
        large = name.startswith("grande_")
        impacts = []
        for j in range(crater_count):
            # One broad, readable impact; smaller irregular depressions elsewhere.
            center = Vector((0.25, -0.30, 1)).normalized() if j == 0 else Vector(
                (rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized()
            radius = 0.44 if j == 0 else rng.uniform(0.27 if large else 0.18, 0.36)
            depth = rng.uniform(0.08, 0.14) if large else rng.uniform(0.13, 0.24)
            impacts.append((center, radius, depth))
        waves = [(Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized(),
                  rng.uniform(3, 7), rng.random()*math.tau) for _ in range(5)]

        def relief(d):
            broad = sum(math.sin(d.dot(v)*frequency+phase) for v, frequency, phase in waves)/5
            fine = noise(d*(12 if large else 37)+Vector((index*13, 7, 11)))*2-1
            height = 1+0.26*broad+(0.004 if large else 0.018)*fine
            floor, rim = 0.0, 0.0
            for center, radius, depth in impacts:
                angle = math.acos(max(-1, min(1, d.dot(center))))
                t = angle/radius * (1+0.07*math.sin(d.x*23+d.y*17))
                if t > 1.5:
                    continue
                bowl = 1-smooth(0.0 if large else 0.08, 0.98 if large else 0.92, t)
                lip = math.exp(-((t-1)/(0.25 if large else 0.16))**2)
                height -= depth*bowl
                height += (0.012 if large else 0.045)*lip
                floor, rim = max(floor, bowl), max(rim, lip)
            return height, floor, rim

        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdivisions, radius=1)
        obj = bpy.context.object
        obj.name = name
        directions = [v.co.normalized() for v in obj.data.vertices]
        for vertex, d in zip(obj.data.vertices, directions):
            height = relief(d)[0]
            # Asymmetric shear and unequal axes prevent scaled spherical silhouettes.
            p = Vector(((d.x+0.15*d.z*d.z)*axes[0], d.y*axes[1],
                        (d.z+0.12*d.x*d.y)*axes[2]))*height
            vertex.co = (p.x, -p.z, p.y)
        for mat in materials:
            obj.data.materials.append(mat)
        for face in obj.data.polygons:
            d = sum((directions[v] for v in face.vertices), Vector()).normalized()
            _, floor, rim = relief(d)
            # Large rocks rely on the relief for shading, without painted polygonal steps.
            face.material_index = (index % 2) if large else (2 if floor > 0.55 else 3 if rim > 0.6 else (index % 2))
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        bm.to_mesh(obj.data)
        bm.free()
        if large:
            bpy.context.view_layer.objects.active = obj
            erosion = obj.modifiers.new("Erosion suave de picos y bordes de impacto", "SMOOTH")
            erosion.factor = 0.45
            erosion.iterations = 4
            bpy.ops.object.modifier_apply(modifier=erosion.name)
        obj["semilla"] = 92371+index*137
        obj["crateres"] = crater_count
        obj["tamano"] = name.split("_")[0]
        full_count = export(obj, output/f"{name}.obj")
        lod = obj.copy()
        lod.data = obj.data.copy()
        lod.name = name+"_lod"
        scene.collection.objects.link(lod)
        bpy.context.view_layer.objects.active = lod
        decimate = lod.modifiers.new("Detalle a distancia exportado desde Blender", "DECIMATE")
        decimate.ratio = lod_faces/full_count
        bpy.ops.object.modifier_apply(modifier=decimate.name)
        low_count = export(lod, output/f"{name}_lod.obj")
        # Presentation offsets live only in the editable scene, never in OBJ coordinates.
        obj.location = ((index % 3)*3.3, (index//3)*3.3, 0)
        lod.location = obj.location.copy()
        lod.hide_viewport = True
        lod.hide_render = True
        print(f"{name}: {full_count} triangulos, LOD {low_count}, {crater_count} crateres", flush=True)
    (output/"asteroides.mtl").write_text("".join(
        f"newmtl roca_{i}\nKd {r} {g} {b}\nillum 2\n" for i, (r, g, b) in enumerate(COLORES)), encoding="utf-8")
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output/"asteroides.blend"), compress=True)


if __name__ == "__main__":
    main()

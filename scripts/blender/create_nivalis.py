"""Generate the editable Nivalis Blender scene and export its CPU-renderer OBJ.

Run in Blender's Scripting workspace or with:
blender --background --factory-startup --python scripts/blender/create_nivalis.py
The output directory can be overridden below, even in an unsaved .blend.
"""
import argparse
import math
from pathlib import Path
import sys

import bpy
from mathutils import Vector

DIRECTORIO_SALIDA = ""
SEGMENTOS, ANILLOS = 160, 80
EJE_LAGO = Vector((0.18, 0.12, 1.0)).normalized()
# Center, width, length, height, twist. Each glacier has its own local frame.
# No latitude/longitude bands: isolated massifs and bent tongues of ice.
GLACIARES = [
    ((-0.46, 0.43, 0.78), 0.17, 0.28, 0.19, 0.5),
    ((0.62, 0.26, 0.74), 0.15, 0.31, 0.16, -0.7),
    ((-0.57, -0.28, 0.77), 0.23, 0.17, 0.14, 0.9),
    ((0.20, -0.59, 0.79), 0.13, 0.34, 0.18, 0.2),
    ((-0.12, 0.86, 0.46), 0.22, 0.25, 0.17, -0.3),
    ((-0.88, 0.18, -0.39), 0.19, 0.34, 0.20, 0.4),
    ((0.62, 0.52, -0.58), 0.24, 0.23, 0.16, -0.8),
    ((-0.23, -0.54, -0.81), 0.19, 0.31, 0.18, 1.2),
    ((0.68, -0.68, -0.23), 0.16, 0.28, 0.15, 0.6),
    ((-0.61, -0.76, 0.14), 0.21, 0.24, 0.17, -0.5),
    ((0.95, 0.08, 0.03), 0.12, 0.27, 0.13, 0.8),
    ((0.05, 0.21, -0.97), 0.28, 0.19, 0.18, -0.4),
]
COLORES = [(0.82, 0.91, 0.93), (0.39, 0.70, 0.79), (0.176, 0.424, 0.557)]


def smooth(a, b, value):
    t = max(0.0, min(1.0, (value - a) / (b - a)))
    return t * t * (3.0 - 2.0 * t)


def detail(p):
    # Smooth deterministic terrain variation; separate from glacier footprints.
    return (math.sin(p.x * 21 + math.sin(p.z * 12))
            * math.sin(p.y * 17 + p.z * 8) + 1) * 0.5


def glacier_frames():
    result = []
    for center, width, length, height, twist in GLACIARES:
        center = Vector(center).normalized()
        tangent = center.cross(Vector((0, 1, 0))).normalized()
        other = tangent.cross(center).normalized()
        right = tangent * math.cos(twist) + other * math.sin(twist)
        result.append((center, right, right.cross(center), width, length, height))
    return result


def elevation(direction, glaciers):
    relief = 0.0
    for center, right, along, width, length, height in glaciers:
        facing = direction.dot(center)
        if facing < 0.8:
            continue
        x, y = direction.dot(right) / facing, direction.dot(along) / facing
        x -= 0.035 * math.sin(y * 11.0)
        footprint = (x / width) ** 2 + (y / length) ** 2
        shoulder = 1.0 - smooth(0.35, 1.0, footprint)
        # Uneven crests break the silhouette without making needle-like peaks.
        relief = max(relief, height * shoulder * (0.88 + detail(direction * 1.8) * 0.12))
    distance = (direction - EJE_LAGO).length
    relief *= smooth(0.38, 0.47, distance)
    terrain = 1.0 + detail(direction) * 0.012 + relief
    lake = 0.965 / max(0.5, direction.dot(EJE_LAGO))
    coast = smooth(0.35, 0.43, distance)
    return lake * (1.0 - coast) + terrain * coast, relief


def generate_mesh():
    vertices, reliefs, triangles, materials = [], [], [], []
    frames = glacier_frames()
    for latitude in range(ANILLOS + 1):
        phi = math.pi * latitude / ANILLOS
        for longitude in range(SEGMENTOS):
            theta = math.tau * longitude / SEGMENTOS
            direction = Vector((math.sin(phi) * math.cos(theta), math.cos(phi),
                                math.sin(phi) * math.sin(theta)))
            radius, relief = elevation(direction, frames)
            vertices.append(direction * radius)
            reliefs.append(relief)

    def add(indices):
        a, b, c = (vertices[i] for i in indices)
        point = (a + b + c) / 3
        normal = (b - a).cross(c - a)
        if normal.dot(point) < 0:
            indices = (indices[0], indices[2], indices[1])
        lake = (point.normalized() - EJE_LAGO).length < 0.35
        slope = abs(normal.normalized().dot(point.normalized()))
        materials.append(2 if lake else 1 if slope < 0.83 else 0)
        triangles.append(indices)

    for latitude in range(ANILLOS):
        for longitude in range(SEGMENTOS):
            nxt = (longitude + 1) % SEGMENTOS
            a, b = latitude * SEGMENTOS + longitude, latitude * SEGMENTOS + nxt
            c, d = a + SEGMENTOS, b + SEGMENTOS
            if latitude > 0:
                add((a, b, c))
            if latitude < ANILLOS - 1:
                add((b, d, c))
    return vertices, reliefs, triangles, materials


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p / "Cargo.toml").is_file()), None)
    default = root / "assets/planets/nivalis" if root else Path.home() / "Nivalis"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    vertices, reliefs, triangles, material_ids = generate_mesh()

    scene = bpy.data.scenes.new("Nivalis | modelo editable")
    bpy.context.window.scene = scene
    mesh = bpy.data.meshes.new("Nivalis | nieve, lago y macizos glaciares")
    # Runtime uses Y up; Blender uses Z up. This is a rotation, not a reflection.
    mesh.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], triangles)
    mesh.update()
    planet = bpy.data.objects.new("Nivalis", mesh)
    scene.collection.objects.link(planet)
    for name, rgb in zip(["Nieve", "Paredes glaciares", "Lago congelado"], COLORES):
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb, 1)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*rgb, 1)
        shader.inputs["Roughness"].default_value = 0.32 if name.startswith("Lago") else 0.8
        mesh.materials.append(mat)
    for face, material in zip(mesh.polygons, material_ids):
        face.material_index = material
    group = planet.vertex_groups.new(name="Glaciares | esculpir")
    group.add([i for i, h in enumerate(reliefs) if h > 0.025], 1.0, "REPLACE")
    lake = planet.vertex_groups.new(name="Lago | cuenca plana")
    lake.add([i for i, p in enumerate(vertices) if (p.normalized() - EJE_LAGO).length < 0.35], 1.0, "REPLACE")
    planet["diseno"] = "12 macizos y lenguas glaciares irregulares; sin anillos"
    planet["exportacion"] = "OBJ triangulado, materiales planos, eje Y arriba"
    bpy.context.view_layer.objects.active = planet
    planet.select_set(True)

    with (output / "nivalis.obj").open("w", encoding="utf-8") as file:
        file.write("# Nivalis generado en Blender; Y arriba\nmtllib nivalis.mtl\no Nivalis\n")
        for vertex in mesh.vertices:
            p = vertex.co
            file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
        current = None
        for face in mesh.polygons:
            if face.material_index != current:
                current = face.material_index
                file.write(f"usemtl material_{current}\n")
            file.write("f " + " ".join(str(i + 1) for i in face.vertices) + "\n")
    (output / "nivalis.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nillum 2\n" for i, (r, g, b) in enumerate(COLORES)), encoding="utf-8")

    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "nivalis.blend"), compress=True)
    print(f"NIVALIS: {len(vertices)} vertices, {len(triangles)} triangulos; {output}", flush=True)


if __name__ == "__main__":
    main()

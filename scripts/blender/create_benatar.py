"""Refine the original Benatar in Blender with a small CPU-rendering triangle budget.

blender --background --factory-startup --python scripts/blender/create_benatar.py
The Sketchfab model is a visual reference; its mesh is not imported or downloaded.
"""
import math
from pathlib import Path

import bpy
import bmesh
from mathutils import Vector
from mathutils.bvhtree import BVHTree

PALETTE = [
    ("Cobre del casco", (0.50, 0.255, 0.14), 1.0, False),
    ("Placas de cobre satinado", (0.61, 0.34, 0.185), 1.0, False),
    ("Titanio de alas y motores", (0.35, 0.37, 0.39), 1.0, False),
    ("Juntas y estructura", (0.075, 0.095, 0.11), 1.0, False),
    ("Cristal de cabina", (0.035, 0.115, 0.17), 0.88, False),
    ("Luz de motor", (0.36, 0.76, 0.88), 1.0, True),
    ("Bordes de bronce", (0.59, 0.405, 0.26), 1.0, False),
    ("Marcos de acero", (0.42, 0.46, 0.48), 1.0, False),
]


def components(path):
    vertices, faces = [], []
    for line in path.read_text().splitlines():
        if line.startswith("v "):
            vertices.append(Vector(tuple(map(float, line.split()[1:4]))))
        elif line.startswith("f "):
            faces.append(tuple(int(v.split("/")[0])-1 for v in line.split()[1:]))
    adjacent = [set() for _ in vertices]
    for face in faces:
        for a, b in zip(face, face[1:]+face[:1]):
            adjacent[a].add(b)
            adjacent[b].add(a)
    seen, result = set(), []
    for index in range(len(vertices)):
        if index in seen:
            continue
        stack, indices = [index], []
        seen.add(index)
        while stack:
            i = stack.pop()
            indices.append(i)
            for other in sorted(adjacent[i]-seen):
                stack.append(other)
                seen.add(other)
        indices.sort()
        remap = {v: i for i, v in enumerate(indices)}
        part_faces = [tuple(remap[v] for v in face) for face in faces if face[0] in remap]
        result.append(([vertices[i].copy() for i in indices], part_faces))
    return sorted(result, key=lambda part: len(part[0]), reverse=True)


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd()]
    if bpy.data.filepath:
        candidates.append(Path(bpy.data.filepath).resolve())
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p/"Cargo.toml").is_file()), None)
    if root is None:
        raise FileNotFoundError("No se encontro el proyecto. Abre Benatar.blend o ejecuta el script desde el repositorio.")
    output = root/"assets/ships"
    scene = bpy.data.scenes.new("Benatar | casco, cabina, motores y paneles editables")
    bpy.context.window.scene = scene
    materials, objects = [], []
    for name, rgb, alpha, emissive in PALETTE:
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb, alpha)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*rgb, 1)
        shader.inputs["Alpha"].default_value = alpha
        shader.inputs["Roughness"].default_value = 0.22 if alpha < 1 else 0.30
        shader.inputs["Metallic"].default_value = 0.0 if alpha < 1 else 0.82
        if emissive:
            shader.inputs["Emission Color"].default_value = (*rgb, 1)
            shader.inputs["Emission Strength"].default_value = 1.5
        materials.append(mat)

    def mesh(name, vertices, faces, material):
        data = bpy.data.meshes.new(name)
        data.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], faces)
        for mat in materials:
            data.materials.append(mat)
        for face in data.polygons:
            face.material_index = material
        bm = bmesh.new()
        bm.from_mesh(data)
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        bm.to_mesh(data)
        bm.free()
        data.update()
        obj = bpy.data.objects.new(name, data)
        scene.collection.objects.link(obj)
        objects.append(obj)
        return obj

    def beam(name, a, b, radius, material=7, sides=5):
        a, b = Vector(a), Vector(b)
        axis = (b-a).normalized()
        right = axis.cross(Vector((0, 1, 0)) if abs(axis.y) < 0.9 else Vector((1, 0, 0))).normalized()
        up = axis.cross(right)
        vertices = [center+(right*math.cos(i*math.tau/sides)+up*math.sin(i*math.tau/sides))*radius
                    for center in [a, b] for i in range(sides)]
        faces = [(i, (i+1) % sides, (i+1) % sides+sides, i+sides) for i in range(sides)]
        faces += [tuple(reversed(range(sides))), tuple(range(sides, 2*sides))]
        return mesh(name, vertices, faces, material)

    parts = components(output/"source/Benatar_base.obj")
    hull_vertices, hull_faces = parts[0]
    hull = mesh("Benatar | casco y alas originales refinados", hull_vertices, hull_faces, 0)
    tree = BVHTree.FromPolygons(hull_vertices, hull_faces, all_triangles=True)
    sections = {}
    def wing_mask(p):
        key = round(p.x, 3)
        if key not in sections:
            hits = [sample*0.05 for sample in range(-110, 96)
                    if tree.ray_cast(Vector((p.x, 10, sample*0.05)), Vector((0, -1, 0)))[0] is not None]
            sections[key] = (min(hits), max(hits)) if hits else (p.z-1, p.z+1)
        back, front = sections[key]
        boundary = back+(front-back)*0.58
        return min(abs(p.x)-1.05, 8.9-abs(p.x), p.z-boundary)

    def split_paint(obj, field, material):
        # Cut existing planar faces at the paint boundary; the hull surface does not move.
        # Shared edge intersections make continuous panel outlines instead of triangular zigzags.
        data = obj.data
        vertices = [Vector((v.co.x, v.co.z, -v.co.y)) for v in data.vertices]
        result, ids, lookup = [], [], {tuple(round(v, 8) for v in p): i for i,p in enumerate(vertices)}
        def clip(polygon, positive):
            out = []
            for (a, fa), (b, fb) in zip(polygon, polygon[1:]+polygon[:1]):
                inside_a, inside_b = (fa >= 0) == positive, (fb >= 0) == positive
                if inside_a:
                    out.append((a, fa))
                if inside_a != inside_b:
                    out.append((a+(b-a)*(fa/(fa-fb)), 0.0))
            return out
        for face in data.polygons:
            if face.material_index == material:
                result.append(tuple(face.vertices))
                ids.append(material)
                continue
            polygon = [(vertices[i], field(vertices[i])) for i in face.vertices]
            for positive in [False, True]:
                part = clip(polygon, positive)
                if len(part) < 3:
                    continue
                indices = []
                for point, _ in part:
                    key = tuple(round(v, 8) for v in point)
                    if key not in lookup:
                        lookup[key] = len(vertices)
                        vertices.append(point)
                    if not indices or indices[-1] != lookup[key]:
                        indices.append(lookup[key])
                if len(indices) > 2 and indices[0] == indices[-1]:
                    indices.pop()
                if len(set(indices)) >= 3:
                    result.append(tuple(indices))
                    ids.append(material if positive else face.material_index)
        data.clear_geometry()
        data.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], result)
        for face, index in zip(data.polygons, ids):
            face.material_index = index
        data.update()

    for face in hull.data.polygons:
        x, z = face.center.x, -face.center.y
        face.material_index = 2 if face.normal.z < -0.35 else 1 if abs(x) < 1.5 and z < 2 else 0
    split_paint(hull, wing_mask, 2)
    canopy_vertices = None
    for i, (vertices, faces) in enumerate(parts[1:]):
        count = len(vertices)
        if count == 84:
            canopy_vertices = vertices
            obj = mesh("Benatar | cabina acristalada", vertices, faces, 4)
            for face in obj.data.polygons:
                if face.normal.z < -0.25:
                    face.material_index = 3
        elif count == 40:
            for p in vertices:
                p.z = -1.2+(p.z+1.2)*1.16
            obj = mesh(f"Benatar | cola doble {i}", vertices, faces, 1)
            for face in obj.data.polygons:
                if face.normal.z < -0.2:
                    face.material_index = 3
        else:
            obj = mesh(f"Benatar | {'carcasa de turbina' if count == 120 else 'aleta inferior'} {i}",
                       vertices, faces, 2 if count == 120 else 0)
            if count == 112:
                split_paint(obj, lambda p: min(8.5-abs(p.x), p.z+1.1), 7)

    # Small geometry follows the existing surface via a BVH, rather than floating above it.
    tree = BVHTree.FromPolygons(hull_vertices, hull_faces, all_triangles=True)

    def top(x, z):
        hit, _, _, _ = tree.ray_cast(Vector((x, 10, z)), Vector((0, -1, 0)))
        return None if hit is None else hit.y

    def seam(name, path, width=0.035, material=3):
        vertices, faces = [], []
        for a, b in zip(path, path[1:]):
            a, b = Vector(a), Vector(b)
            delta = b-a
            normal = Vector((-delta.y, delta.x)).normalized()*width*0.5
            steps = max(1, math.ceil(delta.length/0.45))
            for i in range(steps):
                corners = [a+delta*(i/steps)+normal, a+delta*((i+1)/steps)+normal,
                           a+delta*((i+1)/steps)-normal, a+delta*(i/steps)-normal]
                heights = [top(p.x, p.y) for p in corners]
                if any(h is None for h in heights):
                    continue
                offset = len(vertices)
                vertices.extend(Vector((p.x, h+0.017, p.y)) for p, h in zip(corners, heights))
                faces.append(tuple(range(offset, offset+4)))
        if faces:
            mesh(name, vertices, faces, material)

    seam("Benatar | panel central", [(-0.6, 2.05), (-1.1, 1.1), (-1.1, -0.5),
         (-0.6, -1.25), (0.6, -1.25), (1.1, -0.5), (1.1, 1.1), (0.6, 2.05)])
    for side in [-1, 1]:
        edge = []
        for x in [1.6, 2.4, 3.5, 4.8, 6.2, 7.5, 8.5, 9.1]:
            valid = [z*0.1 for z in range(-55, 31) if top(side*x, z*0.1) is not None]
            if valid:
                edge.append((side*x, max(valid)-0.13))
        seam(f"Benatar | borde de ataque {side}", edge, 0.065, 7)
        for x in [2.1, 3.7, 5.3, 6.9]:
            valid = [z*0.1 for z in range(-55, 31) if top(side*x, z*0.1) is not None]
            if valid:
                seam(f"Benatar | junta de ala {side} {x}", [(side*x, min(valid)+0.15),
                     (side*x, max(valid)-0.2)], 0.045)

    # Exposed machinery along the trailing wing roots, following the reference's grey edge.
    for side in [-1, 1]:
        for j, x in enumerate([1.9, 2.8, 3.7, 4.6]):
            x *= side
            footprint = [(-0.24, -0.25), (0.24, -0.25), (0.32, -0.17), (0.32, 0.17),
                         (0.24, 0.25), (-0.24, 0.25), (-0.32, 0.17), (-0.32, -0.17)]
            valid = [z*0.1 for z in range(-50, 16) if top(x, z*0.1) is not None]
            if not valid:
                continue
            for z in [min(valid)+0.3+i*0.1 for i in range(12)]:
                heights = [top(x+dx, z+dz) for dx, dz in footprint]
                if all(h is not None for h in heights):
                    break
            else:
                continue
            vertices = [Vector((x+dx, h+offset, z+dz))
                        for offset in [0.01, 0.17] for (dx, dz), h in zip(footprint, heights)]
            faces = [(i, (i+1) % 8, (i+1) % 8+8, i+8) for i in range(8)]
            faces += [tuple(reversed(range(8))), tuple(range(8, 16))]
            mesh(f"Benatar | modulo mecanico de ala {side} {j}", vertices, faces, 2)
            for k, dz in enumerate([-0.08, 0.08]):
                seam(f"Benatar | rejilla {side} {j} {k}", [(x-0.23, z+dz), (x+0.23, z+dz)], 0.035, 3)
                # Raise the grille onto the module's upper skin instead of the base hull.
                for v in objects[-1].data.vertices:
                    v.co.z += 0.16

    if canopy_vertices is None:
        raise ValueError("La base no contiene la cabina esperada")
    rings = {}
    for p in canopy_vertices:
        rings.setdefault(round(p.z, 4), []).append(p)
    rings = sorted(rings.values(), key=lambda ring: ring[0].z)
    for j in [0, 2, 4, 6]:
        ring = rings[j]
        center = sum(ring, Vector())/len(ring)
        ordered = sorted(ring, key=lambda p: math.atan2(p.y-center.y, p.x-center.x))
        for i, a in enumerate(ordered):
            b = ordered[(i+1) % len(ordered)]
            if a.y >= 0.08 and b.y >= 0.08:
                beam(f"Benatar | cuaderna de cabina {j} {i}", a, b, 0.025)
    for side in [-1, 1]:
        path = [max(ring, key=lambda p: side*p.x+0.45*p.y) for ring in rings]
        for i, (a, b) in enumerate(zip(path, path[1:])):
            beam(f"Benatar | larguero de cabina {side} {i}", a, b, 0.025)

    # Dorsal intake blades and short rear exhausts make the machinery readable without micro-greebles.
    for side in [-1, 1]:
        center = Vector((side*3.7, 0.15, 0.12))
        vertices, faces = [], []
        for i in range(12):
            angle = i*math.tau/12
            offset = len(vertices)
            for radius, twist in [(0.22, 0), (0.68, 0.16), (0.68, 0.30), (0.22, 0.18)]:
                vertices.append(center+Vector((math.cos(angle+twist)*radius, 0,
                                               math.sin(angle+twist)*radius)))
            faces.append(tuple(range(offset, offset+4)))
        mesh(f"Benatar | alabes de turbina {side}", vertices, faces, 3)
        x = side*1.45
        vertices = [Vector((x+math.cos(i*math.tau/12)*radius,
                    -0.12+math.sin(i*math.tau/12)*radius, z))
                    for z, radius in [(-1.5, 0.38), (-2.0, 0.42), (-2.12, 0.30)] for i in range(12)]
        faces = [(row*12+i, row*12+(i+1) % 12, (row+1)*12+(i+1) % 12, (row+1)*12+i)
                 for row in range(2) for i in range(12)]
        mesh(f"Benatar | tobera {side}", vertices, faces, 2)
        mesh(f"Benatar | nucleo de motor {side}", vertices[-12:], [tuple(range(12))], 5)

    total, vertex_count = 0, 0
    with (output/"Benatar.obj").open("w", encoding="utf-8") as f:
        f.write("# Benatar refinada en Blender; Y arriba, proa +Z\nmtllib Benatar.mtl\n")
        for obj in objects:
            obj.data.calc_loop_triangles()
            f.write(f"o {obj.name.replace(' ', '_')}\n")
            for v in obj.data.vertices:
                f.write(f"v {v.co.x:.7f} {v.co.z:.7f} {-v.co.y:.7f}\n")
            material = -1
            for face in obj.data.loop_triangles:
                index = obj.data.polygons[face.polygon_index].material_index
                if index != material:
                    f.write(f"usemtl benatar_{index}\n")
                    material = index
                f.write("f "+" ".join(str(i+1+vertex_count) for i in face.vertices)+"\n")
                total += 1
            vertex_count += len(obj.data.vertices)
    (output/"Benatar.mtl").write_text("".join(
        f"newmtl benatar_{i}\nKd {rgb[0]} {rgb[1]} {rgb[2]}\nd {alpha}\nillum {0 if light else 2}\n"
        for i, (_, rgb, alpha, light) in enumerate(PALETTE)), encoding="utf-8")
    assert total <= 4200, f"Presupuesto excedido: {total} triangulos"
    scene["triangulos_exportados"] = total
    scene["referencia_visual"] = "https://sketchfab.com/3d-models/guardians-of-the-galaxy-benatar-ship-model-90feb21135734062ac03557cbf2dc328"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output/"Benatar.blend"), compress=True)
    print(f"BENATAR: {total} triangulos; {len(objects)} piezas editables; sin imagenes", flush=True)


if __name__ == "__main__":
    main()

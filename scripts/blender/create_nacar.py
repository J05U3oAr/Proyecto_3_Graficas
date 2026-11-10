"""Generate Nacar in Blender: fractured sea ice and tilted tabular icebergs.

blender --background --factory-startup --python scripts/blender/create_nacar.py
Only creates an editable .blend, OBJ and MTL; no images are rendered or baked.
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
SEMILLA = 88421
MATERIALES = [
    ("Canales azul profundo", (0.03, 0.11, 0.23)),
    ("Banquisa nacarada", (0.72, 0.86, 0.90)),
    ("Placas de escarcha", (0.83, 0.92, 0.94)),
    ("Bordes de hielo azul", (0.23, 0.50, 0.64)),
    ("Paredes del iceberg", (0.36, 0.65, 0.78)),
    ("Cubierta del iceberg", (0.90, 0.95, 0.98)),
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
    t = [smooth(0,1,p[i]-cell[i]) for i in range(3)]
    return sum(hash_value(cell[0]+x,cell[1]+y,cell[2]+z)
               * (t[0] if x else 1-t[0]) * (t[1] if y else 1-t[1]) * (t[2] if z else 1-t[2])
               for x in range(2) for y in range(2) for z in range(2))


def plate_centers():
    rng = random.Random(SEMILLA)
    centers = []
    for _ in range(48):
        centers.append(Vector((rng.gauss(0,1),rng.gauss(0,1),rng.gauss(0,1))).normalized())
    return centers


def ice_field(direction, centers):
    warp = Vector((noise(direction*4+Vector((17,9,31)))-0.5,
                   noise(direction*4+Vector((5,23,11)))-0.5,
                   noise(direction*4+Vector((29,3,19)))-0.5)) * 0.18
    warped = (direction+warp).normalized()
    nearest = sorted(((warped.dot(c) + hash_value(i,SEMILLA,7)*0.035, i)
                      for i,c in enumerate(centers)), reverse=True)
    edge = nearest[0][0] - nearest[1][0]
    index = nearest[0][1]
    width = 0.004 + noise(direction*11+Vector((3,19,7))) * 0.022
    fracture = smooth(width*0.12,width,edge)
    # Refrozen bridges interrupt the polygon outlines instead of tracing every cell.
    bridge = smooth(0.46,0.68,noise(direction*7+Vector((11,29,5)))) * 0.60
    plate = fracture + (1-fracture)*bridge
    slope = 0.008 * math.sin(index * 2.7) * direction.x
    level = 1.0 + slope + 0.006 * math.sin(index * 1.9)
    return 0.955 * (1 - plate) + level * plate, plate, index


def make_object(scene, name, vertices, faces, ids, materials):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([(p.x, -p.z, p.y) for p in vertices], [], faces)
    for material in materials:
        mesh.materials.append(material)
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


def ocean(scene, centers, materials):
    ico = bmesh.new()
    bmesh.ops.create_icosphere(ico, subdivisions=6, radius=1)
    ico.verts.ensure_lookup_table()
    ico.verts.index_update()
    vertices = []
    areas = [0]*len(centers)
    for vertex in ico.verts:
        direction = vertex.co.normalized()
        radius,_,index = ice_field(direction,centers)
        vertices.append(direction*radius)
        areas[index] += 1
    faces, ids = [], []
    for face in ico.faces:
        triangle = tuple(v.index for v in face.verts)
        point = sum((vertices[i] for i in triangle), Vector()) / 3
        _, plate, index = ice_field(point.normalized(), centers)
        faces.append(triangle)
        ids.append(0 if plate < 0.40 else 3 if plate < 0.90 else 1)
    ico.free()
    obj = make_object(scene, "Nacar | oceano congelado y placas", vertices, faces, ids, materials)
    group = obj.vertex_groups.new(name="Canales entre placas")
    group.add([i for i,p in enumerate(vertices) if p.length < 0.98], 1.0, "REPLACE")
    obj["semilla"] = SEMILLA
    obj["diseno"] = "Semillas aleatorias ponderadas, fracturas curvas y puentes recongelados"
    mean = sum(areas)/len(areas)
    obj["variacion_de_areas"] = math.sqrt(sum((a-mean)**2 for a in areas)/len(areas))/mean
    return obj


def icebergs(scene, materials):
    rng = random.Random(SEMILLA + 1)
    centers, objects = [], []
    for _ in range(512):
        axis = Vector((rng.gauss(0,1),rng.gauss(0,1),rng.gauss(0,1))).normalized()
        if any(axis.dot(c) > 0.96 for c in centers):
            continue
        centers.append(axis)
        right = axis.cross(Vector((0,1,0))).normalized()
        forward = right.cross(axis)
        width, length = rng.uniform(0.08,0.28), rng.uniform(0.09,0.27)
        height = rng.uniform(0.07,0.25)
        tilt_x, tilt_z = rng.uniform(-0.45,0.45), rng.uniform(-0.35,0.35)
        lean = right * rng.uniform(-0.07,0.07) + forward * rng.uniform(-0.06,0.06)
        corners = rng.choice([4,5,6,7])
        spin = rng.random() * math.tau
        footprint = []
        for j in range(corners):
            theta = spin + j * math.tau / corners + rng.uniform(-0.14,0.14)
            footprint.append((math.cos(theta) * width * rng.uniform(0.75,1.20),
                              math.sin(theta) * length * rng.uniform(0.75,1.20)))
        vertices = []
        for top in [False,True]:
            for x,z in footprint:
                radial = 0.93 if not top else 0.97 + height + x * tilt_x + z * tilt_z
                vertices.append(axis * radial + right * x + forward * z + (lean if top else Vector()))
        faces, ids = [], []
        for j in range(corners):
            nxt = (j+1) % corners
            faces.append((j,nxt,nxt+corners,j+corners))
            ids.append(4 if j % 3 else 3)
        faces.extend([tuple(reversed(range(corners))), tuple(range(corners,2*corners))])
        ids.extend([4,5])
        obj = make_object(scene, f"Nacar | iceberg tabular {len(objects)+1:02}", vertices, faces, ids, materials)
        obj["diseno"] = "Bloque tabular inclinado, cubierta plana y base sumergida"
        objects.append(obj)
        if len(objects) == 16:
            break
    return objects


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate,*candidate.parents]
                 if (p / "Cargo.toml").is_file()),None)
    default = root / "assets/planets/nacar" if root else Path.home() / "Nacar"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--")+1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True,exist_ok=True)
    scene = bpy.data.scenes.new("Nacar | banquisa fracturada e icebergs")
    bpy.context.window.scene = scene
    materials = []
    for name,rgb in MATERIALES:
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb,1)
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        bsdf.inputs["Base Color"].default_value = (*rgb,1)
        bsdf.inputs["Roughness"].default_value = 0.27 if len(materials) == 0 else 0.62
        materials.append(mat)
    objects = [ocean(scene,plate_centers(),materials)] + icebergs(scene,materials)
    offset,triangles = 0,0
    with (output / "nacar.obj").open("w",encoding="utf-8") as file:
        file.write("# Nacar generado en Blender; Y arriba\nmtllib nacar.mtl\n")
        for obj in objects:
            obj.data.calc_loop_triangles()
            file.write(f"o {obj.name.replace(' ','_')}\n")
            for vertex in obj.data.vertices:
                p = vertex.co
                file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
            current = None
            for triangle in obj.data.loop_triangles:
                if triangle.material_index != current:
                    current = triangle.material_index
                    file.write(f"usemtl material_{current}\n")
                file.write("f " + " ".join(str(i+1+offset) for i in triangle.vertices) + "\n")
                triangles += 1
            offset += len(obj.data.vertices)
    (output / "nacar.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nillum 2\n" for i,(_, (r,g,b)) in enumerate(MATERIALES)),encoding="utf-8")
    bpy.context.view_layer.objects.active = objects[0]
    for obj in objects:
        obj.select_set(True)
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "nacar.blend"),compress=True)
    print(f"NACAR: {offset} vertices, {triangles} triangulos, {len(objects)-1} icebergs; {output}",flush=True)


if __name__ == "__main__":
    main()

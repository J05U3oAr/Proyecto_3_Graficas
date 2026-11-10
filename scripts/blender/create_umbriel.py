"""Create Umbriel and its icy rings in Blender without rendering image files.

blender --background --factory-startup --python scripts/blender/create_umbriel.py
Can also run in Blender's Text Editor; no saved .blend is required beforehand.
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
SEMILLA = 61327
INCLINACION = math.radians(27)
BANDAS = [(1.27,1.36,1),(1.40,1.53,2),(1.59,1.70,3)]
ANILLOS = [
    ("Fragmentos de hielo",(0.70,0.83,0.94),1.0),
    ("Velo interior azulado",(0.42,0.53,0.65),0.46),
    ("Cinturon nacarado",(0.68,0.77,0.86),0.58),
    ("Velo exterior azul grisaceo",(0.38,0.47,0.59),0.40),
]


def tilt(p):
    return Vector((p.x,p.y*math.cos(INCLINACION)-p.z*math.sin(INCLINACION),
                   p.y*math.sin(INCLINACION)+p.z*math.cos(INCLINACION)))


def make_mesh(scene,name,vertices,faces,material):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata([(p.x,-p.z,p.y) for p in vertices],[],faces)
    mesh.materials.append(material)
    mesh.update()
    obj = bpy.data.objects.new(name,mesh)
    scene.collection.objects.link(obj)
    return obj


def export(objects,path,mtl,material_index):
    count,triangles = 0,0
    with path.open("w",encoding="utf-8") as file:
        file.write(f"# Umbriel generado en Blender; Y arriba\nmtllib {mtl}\n")
        for obj in objects:
            mesh = obj.data
            mesh.calc_loop_triangles()
            file.write(f"o {obj.name.replace(' ','_')}\n")
            for vertex in mesh.vertices:
                p = obj.matrix_world @ vertex.co
                file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
            file.write(f"usemtl material_{material_index(obj)}\n")
            for face in mesh.loop_triangles:
                file.write("f " + " ".join(str(i+1+count) for i in face.vertices)+"\n")
                triangles += 1
            count += len(mesh.vertices)
    return triangles


def main():
    candidates = [Path(globals().get("__file__",".")).resolve(),Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate,*candidate.parents]
                 if (p/"Cargo.toml").is_file()),None)
    default = root/"assets/planets/umbriel" if root else Path.home()/"Umbriel"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--")+1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True,exist_ok=True)
    scene = bpy.data.scenes.new("Umbriel | gigante de tormentas y anillos helados")
    bpy.context.window.scene = scene
    bpy.ops.mesh.primitive_uv_sphere_add(segments=96,ring_count=48,radius=1)
    body = bpy.context.object
    body.name = "Umbriel | atmosfera editable"
    for vertex in body.data.vertices:
        vertex.co.z *= 0.93
    for face in body.data.polygons:
        face.use_smooth = True
    gas = bpy.data.materials.new("Umbriel | atmosfera azul marino y tormentas blancas")
    gas.diffuse_color = (0.025,0.055,0.12,1)
    gas.use_nodes = True
    nodes,links = gas.node_tree.nodes,gas.node_tree.links
    bsdf = nodes.get("Principled BSDF")
    bsdf.inputs["Roughness"].default_value = 0.82
    coords = nodes.new("ShaderNodeTexCoord")
    turbulence = nodes.new("ShaderNodeTexNoise")
    turbulence.inputs["Scale"].default_value = 7
    turbulence.inputs["Detail"].default_value = 3
    links.new(coords.outputs["Object"],turbulence.inputs["Vector"])
    # Distorted, stretched cloud streams. All coordinates remain procedural.
    distortion = nodes.new("ShaderNodeVectorMath")
    distortion.operation = "SCALE"
    distortion.inputs[3].default_value = 0.3
    links.new(turbulence.outputs["Color"],distortion.inputs[0])
    warp = nodes.new("ShaderNodeVectorMath")
    warp.operation = "ADD"
    links.new(coords.outputs["Object"],warp.inputs[0])
    links.new(distortion.outputs[0],warp.inputs[1])
    stretch = nodes.new("ShaderNodeVectorMath")
    stretch.operation = "MULTIPLY"
    stretch.inputs[1].default_value = (4.5,4.5,19.0)
    links.new(warp.outputs[0],stretch.inputs[0])
    clouds = nodes.new("ShaderNodeTexNoise")
    clouds.inputs["Scale"].default_value = 1.0
    clouds.inputs["Detail"].default_value = 3
    links.new(stretch.outputs[0],clouds.inputs["Vector"])
    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.22
    ramp.color_ramp.elements[0].color = (0.008,0.018,0.048,1)
    ramp.color_ramp.elements[1].position = 0.82
    ramp.color_ramp.elements[1].color = (0.78,0.85,0.91,1)
    ramp.color_ramp.elements.new(0.60).color = (0.035,0.075,0.14,1)
    ramp.color_ramp.interpolation = "EASE"
    links.new(clouds.outputs["Fac"],ramp.inputs[0])
    links.new(ramp.outputs["Color"],bsdf.inputs["Base Color"])
    body.data.materials.append(gas)
    body["radio_en_juego"] = 155.0
    body["diseno"] = "Gigante oblato; conveccion y vortices blancos animados en el rasterizador CPU"
    ring_materials = []
    for name,rgb,alpha in ANILLOS:
        mat = bpy.data.materials.new(name)
        mat.diffuse_color = (*rgb,alpha)
        mat.use_nodes = True
        shader = mat.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Base Color"].default_value = (*rgb,1)
        shader.inputs["Alpha"].default_value = alpha
        shader.inputs["Roughness"].default_value = 0.5
        ring_materials.append(mat)
    # Independent collections and exports keep the rings editable without touching the body.
    body_collection = bpy.data.collections.new("UMBriel | planeta")
    ring_collection = bpy.data.collections.new("UMBriel | anillos independientes")
    scene.collection.children.link(body_collection)
    scene.collection.children.link(ring_collection)
    scene.collection.objects.unlink(body)
    body_collection.objects.link(body)
    rings = []
    for index,(inner,outer,material) in enumerate(BANDAS):
        vertices,faces = [],[]
        for i in range(192):
            angle = math.tau*i/192
            for radius in [inner,outer]:
                vertices.append(tilt(Vector((math.cos(angle)*radius,0,math.sin(angle)*radius))))
        for i in range(192):
            nxt = (i+1)%192
            faces.append((i*2,nxt*2,nxt*2+1,i*2+1))
        obj = make_mesh(scene,f"Umbriel | banda de hielo {index+1:02}",vertices,faces,ring_materials[material])
        obj["material_exportado"] = material
        rings.append(obj)
    ico = bmesh.new()
    bmesh.ops.create_icosphere(ico,subdivisions=1,radius=1)
    ico.verts.ensure_lookup_table()
    ico.verts.index_update()
    template_vertices = [v.co.copy() for v in ico.verts]
    template_faces = [tuple(v.index for v in face.verts) for face in ico.faces]
    ico.free()
    rng = random.Random(SEMILLA)
    for i in range(96):
        inner,outer,_ = rng.choice(BANDAS)
        radius = rng.uniform(inner,outer)
        angle = rng.random()*math.tau
        center = Vector((radius*math.cos(angle),rng.uniform(-0.006,0.006),radius*math.sin(angle)))
        size = rng.uniform(0.004,0.0125)
        stretch = Vector((rng.uniform(0.6,1.0),rng.uniform(0.5,0.9),rng.uniform(0.6,1.0)))
        vertices = [tilt(center+Vector((v.x*stretch.x,v.y*stretch.y,v.z*stretch.z))*size) for v in template_vertices]
        obj = make_mesh(scene,f"Umbriel | fragmento helado {i+1:03}",vertices,template_faces,ring_materials[0])
        obj["material_exportado"] = 0
        rings.append(obj)
    for obj in rings:
        scene.collection.objects.unlink(obj)
        ring_collection.objects.link(obj)
    body_triangles = export([body],output/"umbriel.obj","umbriel.mtl",lambda obj:0)
    ring_triangles = export(rings,output/"anillos.obj","anillos.mtl",lambda obj:int(obj["material_exportado"]))
    (output/"umbriel.mtl").write_text("newmtl material_0\nKd 1 1 1\nillum 2\n",encoding="utf-8")
    (output/"anillos.mtl").write_text("".join(
        f"newmtl material_{i}\nKd {r} {g} {b}\nd {alpha}\nillum 2\n"
        for i,(_, (r,g,b),alpha) in enumerate(ANILLOS)),encoding="utf-8")
    bpy.context.view_layer.objects.active = body
    for obj in [body,*rings]:
        obj.select_set(True)
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 5.0
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output/"umbriel.blend"),compress=True)
    print(f"UMBRIEL: cuerpo {body_triangles} triangulos, anillos {ring_triangles} triangulos, 96 fragmentos; {output}",flush=True)


if __name__ == "__main__":
    main()

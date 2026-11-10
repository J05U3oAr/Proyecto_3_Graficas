"""Create Umbria's massive blue-white sun, without rendering or baking images.

Run with Blender --background --factory-startup --python scripts/blender/create_asterion.py
or open this script in Blender's Text Editor and Run Script.
"""
import argparse
from pathlib import Path
import sys

import bpy

DIRECTORIO_SALIDA = ""


def main():
    candidates = [Path(globals().get("__file__", ".")).resolve(), Path.cwd(),
                  Path(bpy.data.filepath).resolve() if bpy.data.filepath else Path.cwd()]
    root = next((p for candidate in candidates for p in [candidate, *candidate.parents]
                 if (p / "Cargo.toml").is_file()), None)
    default = root / "assets/planets/asterion" if root else Path.home() / "Asterion"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=DIRECTORIO_SALIDA or str(default))
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = Path(args.output).expanduser().resolve()
    output.mkdir(parents=True, exist_ok=True)
    scene = bpy.data.scenes.new("Asterion | sol de Umbria")
    bpy.context.window.scene = scene
    bpy.ops.mesh.primitive_uv_sphere_add(segments=96, ring_count=48, radius=1)
    star = bpy.context.object
    star.name = "Asterion | fotosfera editable"
    for polygon in star.data.polygons:
        polygon.use_smooth = True

    material = bpy.data.materials.new("Asterion | plasma blanco azulado")
    material.diffuse_color = (0.65, 0.82, 1, 1)
    material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    nodes.clear()
    coordinates = nodes.new("ShaderNodeTexCoord")
    turbulence = nodes.new("ShaderNodeTexNoise")
    turbulence.inputs["Scale"].default_value = 38
    turbulence.inputs["Detail"].default_value = 2
    links.new(coordinates.outputs["Generated"], turbulence.inputs["Vector"])
    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.25
    ramp.color_ramp.elements[0].color = (0.22, 0.48, 1, 1)
    ramp.color_ramp.elements[1].position = 0.75
    ramp.color_ramp.elements[1].color = (0.95, 0.98, 1, 1)
    links.new(turbulence.outputs["Fac"], ramp.inputs["Fac"])
    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Strength"].default_value = 2
    links.new(ramp.outputs["Color"], emission.inputs["Color"])
    surface = nodes.new("ShaderNodeOutputMaterial")
    links.new(emission.outputs[0], surface.inputs["Surface"])
    star.data.materials.append(material)
    star["radio_en_juego"] = 360.0
    star["diseno"] = "Fotosfera blanca azulada; corona cian y conveccion animada en CPU"
    star["escala"] = "Radio unitario editable; escala final aplicada por el renderizador"

    mesh = star.data
    mesh.calc_loop_triangles()
    with (output / "asterion.obj").open("w", encoding="utf-8") as file:
        file.write("# Asterion generado en Blender; Y arriba\nmtllib asterion.mtl\no Asterion\n")
        for vertex in mesh.vertices:
            p = vertex.co
            file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
        file.write("usemtl Photosphere\n")
        for triangle in mesh.loop_triangles:
            file.write("f " + " ".join(str(i + 1) for i in triangle.vertices) + "\n")
    (output / "asterion.mtl").write_text(
        "newmtl Photosphere\nKd 0.65 0.82 1.0\nillum 0\n", encoding="utf-8")
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "asterion.blend"), compress=True)
    print(f"ASTERION: {len(mesh.vertices)} vertices; {len(mesh.loop_triangles)} triangulos; {output}", flush=True)


if __name__ == "__main__":
    main()

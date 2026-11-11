"""Create Aureon, Helice's giant golden sun, as an editable Blender asset.

blender --background --factory-startup --python-exit-code 1 \
  --python scripts/blender/create_aureon.py
"""
import argparse
from pathlib import Path
import sys

import bpy

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "assets/planets/aureon")
    parser.add_argument("--overwrite", action="store_true")
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else [])
    output = args.output.resolve()
    if (output / "aureon.blend").exists() and not args.overwrite:
        raise FileExistsError("Aureon already exists; pass --overwrite to regenerate")
    output.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.name = "Aureon | gran sol de Helice"
    bpy.ops.mesh.primitive_uv_sphere_add(segments=96, ring_count=48, radius=1)
    star = bpy.context.object
    star.name = "Aureon | fotosfera dorada"
    for face in star.data.polygons:
        face.use_smooth = True
    material = bpy.data.materials.new("Aureon | plasma ambar y oro")
    material.diffuse_color = (1.0, 0.62, 0.12, 1.0)
    material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    nodes.clear()
    coordinates = nodes.new("ShaderNodeTexCoord")
    cells = nodes.new("ShaderNodeTexNoise")
    cells.inputs["Scale"].default_value = 26
    cells.inputs["Detail"].default_value = 3
    links.new(coordinates.outputs["Generated"], cells.inputs["Vector"])
    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.20
    ramp.color_ramp.elements[0].color = (0.55, 0.07, 0.005, 1)
    ramp.color_ramp.elements[1].position = 0.80
    ramp.color_ramp.elements[1].color = (1.0, 0.93, 0.52, 1)
    ramp.color_ramp.elements.new(0.48).color = (1.0, 0.43, 0.035, 1)
    links.new(cells.outputs["Fac"], ramp.inputs["Fac"])
    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Strength"].default_value = 2.5
    links.new(ramp.outputs["Color"], emission.inputs["Color"])
    surface = nodes.new("ShaderNodeOutputMaterial")
    links.new(emission.outputs[0], surface.inputs["Surface"])
    star.data.materials.append(material)
    star["radio_en_juego"] = 320.0
    star["diseno"] = "Sol gigante ambar; conveccion y manchas solares animadas en el shader CPU"
    star["sistema"] = "Centro de las siete orbitas de Helice"
    mesh = star.data
    mesh.calc_loop_triangles()
    with (output / "aureon.obj").open("w", encoding="utf-8", newline="\n") as file:
        file.write("# Aureon, original Blender mesh; Y up\nmtllib aureon.mtl\no Aureon\n")
        for vertex in mesh.vertices:
            p = vertex.co
            file.write(f"v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n")
        file.write("usemtl Photosphere\n")
        for face in mesh.loop_triangles:
            file.write("f " + " ".join(str(i + 1) for i in face.vertices) + "\n")
    (output / "aureon.mtl").write_text("newmtl Photosphere\nKd 1.0 0.62 0.12\nillum 0\n", encoding="utf-8")
    text = bpy.data.texts.new("create_aureon.py")
    text.write(Path(__file__).read_text(encoding="utf-8"))
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == "VIEW_3D":
                area.spaces.active.region_3d.view_distance = 3.8
                area.spaces.active.shading.color_type = "MATERIAL"
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=str(output / "aureon.blend"), compress=True)
    print(f"AUREON: {len(mesh.vertices)} vertices, {len(mesh.loop_triangles)} triangulos", flush=True)


if __name__ == "__main__":
    main()

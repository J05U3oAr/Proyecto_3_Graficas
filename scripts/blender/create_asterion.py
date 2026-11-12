"""Create Umbria's massive blue-white sun, without rendering or baking images.

Run with Blender --background --factory-startup --python scripts/blender/create_asterion.py
or open this script in Blender's Text Editor and Run Script.
"""
import argparse
import math
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
    plasma = nodes.new("ShaderNodeTexImage")
    plasma.image = bpy.data.images.load(str(output.parent / "textures/aurora-helios-ii-surface.png"), check_existing=True)
    plasma.image.filepath = "//../textures/aurora-helios-ii-surface.png"
    links.new(coordinates.outputs["UV"], plasma.inputs["Vector"])
    luminance = nodes.new("ShaderNodeRGBToBW")
    links.new(plasma.outputs["Color"], luminance.inputs["Color"])
    ramp = nodes.new("ShaderNodeValToRGB")
    ramp.color_ramp.elements[0].position = 0.25
    ramp.color_ramp.elements[0].color = (0.42, 0.60, 0.78, 1)
    ramp.color_ramp.elements[1].position = 0.90
    ramp.color_ramp.elements[1].color = (1.0, 1.0, 1.0, 1)
    links.new(luminance.outputs[0], ramp.inputs["Fac"])
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
        # Export spherical UVs per corner so the longitude seam is never
        # interpolated across the whole atlas. Geometry and triangle counts stay fixed.
        faces = []
        uv_index = 0
        for triangle in mesh.loop_triangles:
            corners = []
            for index in triangle.vertices:
                p = mesh.vertices[index].co
                x, y, z = p.x, p.z, -p.y
                corners.append([math.atan2(z, x) / math.tau % 1.0,
                                math.acos(max(-1.0, min(1.0, y / p.length))) / math.pi])
            longitudes = [uv[0] for uv, index in zip(corners, triangle.vertices)
                          if abs(mesh.vertices[index].co.z) < 0.9999]
            if max(longitudes) - min(longitudes) > 0.5:
                for uv in corners:
                    if uv[0] < 0.5:
                        uv[0] += 1.0
            for corner, index in enumerate(triangle.vertices):
                p = mesh.vertices[index].co
                if abs(p.z) > 0.9999:
                    corners[corner][0] = sum(corners[j][0] for j in range(3) if j != corner) / 2.0
            for loop, uv in zip(triangle.loops, corners):
                mesh.uv_layers.active.data[loop].uv = uv
                file.write(f"vt {uv[0]:.8f} {uv[1]:.8f}\n")
            faces.append("f " + " ".join(f"{index + 1}/{uv_index + corner + 1}"
                         for corner, index in enumerate(triangle.vertices)) + "\n")
            uv_index += 3
        file.write("usemtl Photosphere\n")
        file.writelines(faces)
    (output / "asterion.mtl").write_text(
        "newmtl Photosphere\nKd 1.0 1.0 1.0\nillum 0\n"
        "map_Kd ../textures/aurora-helios-ii-surface.png\n", encoding="utf-8")
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

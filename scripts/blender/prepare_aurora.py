"""Prepara copias de los modelos de Aurora sin modificar las escenas originales."""
import argparse
import json
import sys
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parents[2]
BODIES = [
    ('flareon', 'Flareon.blend', True),
    ('helios-ii', 'Heiios II.blend', True),
    ('kepler-186f', 'Kepler-186f.blend', False),
    ('metis', 'Metis.blend', False),
]


def connect_color(tree, source, target):
    if source.is_linked:
        tree.links.new(source.links[0].from_socket, target)
    else:
        target.default_value = source.default_value


def prepare(identifier, source, unlit, width, samples):
    directory = ROOT / 'assets/planets'
    bpy.ops.wm.open_mainfile(filepath=str(directory / 'blender' / source), use_scripts=False)
    obj = next(obj for obj in bpy.context.scene.objects if obj.type == 'MESH')
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = samples
    scene.render.bake.margin = 12
    scene.render.bake.use_selected_to_active = False
    scene.view_settings.view_transform = 'Standard'
    image = bpy.data.images.new(f'{identifier}-surface', width=width, height=width // 2, alpha=False)
    image.colorspace_settings.name = 'sRGB'
    material = obj.active_material
    tree = material.node_tree
    output = next(node for node in tree.nodes if node.type == 'OUTPUT_MATERIAL' and node.is_active_output)
    original = output.inputs['Surface'].links[0].from_node
    emission = tree.nodes.new('ShaderNodeEmission')
    emission.inputs['Strength'].default_value = 1.0
    if original.type == 'EMISSION':
        connect_color(tree, original.inputs['Color'], emission.inputs['Color'])
    elif identifier == 'flareon':
        base = tree.nodes.new('ShaderNodeMixRGB')
        base.blend_type = 'ADD'
        base.inputs[0].default_value = 1.0
        connect_color(tree, original.inputs['Base Color'], base.inputs[1])
        connect_color(tree, original.inputs['Emission Color'], base.inputs[2])
        tree.links.new(base.outputs['Color'], emission.inputs['Color'])
    else:
        connect_color(tree, original.inputs['Base Color'], emission.inputs['Color'])
    tree.links.new(emission.outputs[0], output.inputs['Surface'])
    # El relieve del material no se exporta como desplazamiento de la malla.
    for link in list(output.inputs['Displacement'].links):
        tree.links.remove(link)
    target = tree.nodes.new('ShaderNodeTexImage')
    target.image = image
    tree.nodes.active = target
    print(f'BAKE {identifier}: {obj.name}, {width}x{width // 2}', flush=True)
    bpy.ops.object.bake(type='EMIT')
    texture = directory / 'textures' / f'aurora-{identifier}-surface.png'
    image.filepath_raw = str(texture)
    image.file_format = 'PNG'
    image.save()
    original_triangles = sum(len(poly.vertices) - 2 for poly in obj.data.polygons)
    decimate = obj.modifiers.new('Copia para renderizador CPU', 'DECIMATE')
    decimate.ratio = 0.125
    bpy.ops.object.modifier_apply(modifier=decimate.name)
    mesh = obj.data
    mesh.calc_loop_triangles()
    uv = mesh.uv_layers.active.data
    runtime = directory / 'runtime'
    runtime.mkdir(exist_ok=True)
    with (runtime / f'{identifier}.obj').open('w', encoding='utf-8', newline='\n') as file:
        file.write(f'# Copia de {source}; ejes Y arriba\nmtllib {identifier}.mtl\n')
        for vertex in mesh.vertices:
            point = obj.matrix_world @ vertex.co
            file.write(f'v {point.x:.7f} {point.z:.7f} {-point.y:.7f}\n')
        for loop in uv:
            file.write(f'vt {loop.uv.x:.8f} {loop.uv.y:.8f}\n')
        file.write('usemtl Surface\n')
        for triangle in mesh.loop_triangles:
            file.write('f ' + ' '.join(f'{vertex + 1}/{loop + 1}' for vertex, loop in zip(triangle.vertices, triangle.loops)) + '\n')
    (runtime / f'{identifier}.mtl').write_text(
        f'newmtl Surface\nKd 1 1 1\nillum {0 if unlit else 2}\nmap_Kd ../textures/{texture.name}\n', encoding='utf-8')
    result = {'source': f'blender/{source}', 'original_triangles': original_triangles,
              'triangles': len(mesh.loop_triangles), 'vertices': len(mesh.vertices),
              'texture': f'textures/{texture.name}', 'unlit': unlit}
    print('READY', identifier, json.dumps(result), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--only', choices=[body[0] for body in BODIES])
    parser.add_argument('--width', type=int, default=2048)
    parser.add_argument('--samples', type=int, default=8)
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else [])
    manifest = ROOT / 'assets/planets/runtime/aurora.json'
    results = json.loads(manifest.read_text()) if manifest.exists() else {}
    for identifier, source, unlit in BODIES:
        if args.only and identifier != args.only:
            continue
        results[identifier] = prepare(identifier, source, unlit, args.width, args.samples)
        manifest.write_text(json.dumps(results, indent=2), encoding='utf-8')


if __name__ == '__main__':
    main()

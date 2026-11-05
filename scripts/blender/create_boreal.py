"""Create Boreal: an editable turquoise ice giant with a packed cloud atlas.

Run with Blender --background --python scripts/blender/create_boreal.py.
The image is artistic albedo, so both Blender and the game light the sphere.
"""
import argparse
import json
from pathlib import Path
import sys

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))
from create_planets import ROOT, aim, studio


def export(planet, output):
    mesh = planet.data
    mesh.calc_loop_triangles()
    uv = mesh.uv_layers.active.data
    with (output / 'boreal.obj').open('w', encoding='utf-8', newline='\n') as file:
        file.write('# Boreal: artistic ice giant, ImageGen albedo, Blender mesh\n'
                   'mtllib boreal.mtl\no Boreal\n')
        for vertex in mesh.vertices:
            p = planet.matrix_world @ vertex.co
            file.write(f'v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n')
        for loop in uv:
            file.write(f'vt {loop.uv.x:.8f} {loop.uv.y:.8f}\n')
        file.write('usemtl FrozenClouds\n')
        for triangle in mesh.loop_triangles:
            file.write('f ' + ' '.join(f'{v + 1}/{l + 1}'
                       for v, l in zip(triangle.vertices, triangle.loops)) + '\n')
    (output / 'boreal.mtl').write_text(
        'newmtl FrozenClouds\nKd 1 1 1\nillum 2\nmap_Kd textures/boreal-clouds.png\n', encoding='utf-8')
    return {'vertices': len(mesh.vertices), 'triangles': len(mesh.loop_triangles),
            'materials': 1, 'texture': 'textures/boreal-clouds.png'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', default='assets/planets')
    parser.add_argument('--size', type=int, default=960)
    parser.add_argument('--samples', type=int, default=32)
    parser.add_argument('--overwrite', action='store_true')
    parser.add_argument('--no-render', action='store_true')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else [])
    output = (ROOT / args.output).resolve()
    texture_path = output / 'textures/boreal-clouds.png'
    if not texture_path.is_file():
        raise FileNotFoundError(f'Falta la textura: {texture_path}')
    if not args.overwrite and any((output / p).exists()
                                  for p in ['boreal.obj', 'boreal.mtl', 'blender/boreal.blend']):
        raise FileExistsError('Boreal ya existe; usa --overwrite para regenerarlo')
    (output / 'blender').mkdir(parents=True, exist_ok=True)
    (output / 'previews').mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.mesh.primitive_uv_sphere_add(segments=128, ring_count=64, radius=1)
    planet = bpy.context.object
    planet.name = 'Boreal | gigante helado de nubes turquesa'
    planet.rotation_euler.y = -0.22
    for face in planet.data.polygons:
        face.use_smooth = True
    planet.data.uv_layers.active.name = 'FrozenClouds'
    bpy.context.view_layer.update()
    mat = bpy.data.materials.new('Boreal | bandas heladas y tormentas')
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    shader = nodes.get('Principled BSDF')
    shader.inputs['Roughness'].default_value = 0.88
    texture = nodes.new('ShaderNodeTexImage')
    texture.image = bpy.data.images.load(str(texture_path), check_existing=True)
    texture.image.pack()
    texture.image.filepath = '//../textures/boreal-clouds.png'
    texture.interpolation = 'Linear'
    # Match the game's narrow, smooth blend across the longitude seam.
    uv = nodes.new('ShaderNodeTexCoord')
    separate = nodes.new('ShaderNodeSeparateXYZ')
    links.new(uv.outputs['UV'], separate.inputs[0])
    mirror = nodes.new('ShaderNodeMath')
    mirror.operation = 'SUBTRACT'
    mirror.inputs[0].default_value = 1.0
    links.new(separate.outputs['X'], mirror.inputs[1])
    combine = nodes.new('ShaderNodeCombineXYZ')
    links.new(mirror.outputs[0], combine.inputs['X'])
    links.new(separate.outputs['Y'], combine.inputs['Y'])
    other = nodes.new('ShaderNodeTexImage')
    other.image = texture.image
    links.new(combine.outputs[0], other.inputs['Vector'])
    edge = nodes.new('ShaderNodeMath')
    edge.operation = 'MINIMUM'
    links.new(separate.outputs['X'], edge.inputs[0])
    links.new(mirror.outputs[0], edge.inputs[1])
    fade = nodes.new('ShaderNodeMapRange')
    fade.interpolation_type = 'SMOOTHSTEP'
    fade.inputs['From Min'].default_value = 0.0
    fade.inputs['From Max'].default_value = 0.035
    fade.inputs['To Min'].default_value = 0.5
    fade.inputs['To Max'].default_value = 0.0
    links.new(edge.outputs[0], fade.inputs['Value'])
    blend = nodes.new('ShaderNodeMixRGB')
    links.new(fade.outputs[0], blend.inputs[0])
    links.new(texture.outputs['Color'], blend.inputs[1])
    links.new(other.outputs['Color'], blend.inputs[2])
    links.new(blend.outputs[0], shader.inputs['Base Color'])
    planet.data.materials.append(mat)
    planet['referencia'] = 'Imagen de gigante helado turquesa aportada por el usuario'
    planet['textura'] = 'ImageGen: atlas artistico equirectangular, no mapa cientifico de Urano'
    planet['diseno'] = 'Nubes blancas de aspecto helado, bandas cian y tormentas azul petroleo'
    editable = bpy.data.collections.new('01 | Planeta editable')
    bpy.context.scene.collection.children.link(editable)
    for owner in tuple(planet.users_collection):
        owner.objects.unlink(planet)
    editable.objects.link(planet)
    stats = export(planet, output)
    studio('boreal', args.size, args.samples)
    scene = bpy.context.scene
    scene.camera.location = (0, -7.5, 0.9)
    aim(scene.camera)
    scene.camera.data.type = 'ORTHO'
    scene.camera.data.ortho_scale = 2.30
    scene.view_settings.view_transform = 'Standard'
    scene.view_settings.look = 'None'
    scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value = 0.025
    bpy.data.objects['Relleno tenue'].data.energy = 25
    presentation = bpy.data.collections.new('02 | Presentacion - ocultar para editar')
    scene.collection.children.link(presentation)
    for obj in tuple(scene.objects):
        if obj != planet:
            for owner in tuple(obj.users_collection):
                owner.objects.unlink(obj)
            presentation.objects.link(obj)
    for obj in bpy.context.selected_objects:
        obj.select_set(False)
    planet.select_set(True)
    bpy.context.view_layer.objects.active = planet
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                area.spaces.active.region_3d.view_rotation = scene.camera.rotation_euler.to_quaternion()
                area.spaces.active.region_3d.view_distance = 3.8
    scene['planeta'] = 'Boreal'
    scene.render.filepath = '//../previews/boreal.png'
    bpy.ops.wm.save_as_mainfile(filepath=str(output / 'blender/boreal.blend'),
                              compress=True, relative_remap=False)
    if not args.no_render:
        bpy.ops.render.render(write_still=True)
    stats_path = output / 'mesh_stats.json'
    catalog = json.loads(stats_path.read_text(encoding='utf-8')) if stats_path.exists() else {}
    catalog['boreal'] = stats
    stats_path.write_text(json.dumps(catalog, indent=2) + '\n', encoding='utf-8')
    print('BOREAL LISTO', json.dumps(stats), flush=True)


if __name__ == '__main__':
    main()

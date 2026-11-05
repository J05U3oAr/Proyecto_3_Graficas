"""Build Jovara from the NASA/Juno polar image, keeping the reference's fine clouds.

The polar photograph is mapped onto both hemispheres of a real sphere. The back
is an artistic continuation, not a scientific full-surface Jupiter map.
Run with Blender --background --python scripts/blender/create_jovara.py.
"""

import argparse
import json
from pathlib import Path
import sys

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))
from create_planets import ROOT, aim, studio


CREDIT = 'NASA/JPL-Caltech/SwRI/MSSS/Betsy Asher Hall/Gervasio Robles'
SOURCE = 'https://science.nasa.gov/photojournal/southern-storms-2/'


def export(planet, output):
    mesh = planet.data
    mesh.calc_loop_triangles()
    uv = mesh.uv_layers['PolarClouds'].data
    with (output / 'jovara.obj').open('w', encoding='utf-8', newline='\n') as file:
        file.write(f'# Cloud image: {CREDIT}\n# Source: {SOURCE}\nmtllib jovara.mtl\no Jovara\n')
        for vertex in mesh.vertices:
            p = planet.matrix_world @ vertex.co
            file.write(f'v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n')
        for loop in uv:
            file.write(f'vt {loop.uv.x:.8f} {loop.uv.y:.8f}\n')
        file.write('usemtl PolarClouds\n')
        for triangle in mesh.loop_triangles:
            corners = zip(triangle.vertices, triangle.loops)
            file.write('f ' + ' '.join(f'{vertex + 1}/{loop + 1}' for vertex, loop in corners) + '\n')
    (output / 'jovara.mtl').write_text(
        '# The photograph already contains lighting: retain its enhanced colors.\n'
        'newmtl PolarClouds\nKd 1 1 1\nillum 0\nmap_Kd textures/jovara-clouds.png\n', encoding='utf-8')
    return {'vertices': len(mesh.vertices), 'triangles': len(mesh.loop_triangles),
            'materials': 1, 'texture': 'textures/jovara-clouds.png'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', default='assets/planets')
    parser.add_argument('--size', type=int, default=960)
    parser.add_argument('--samples', type=int, default=32)
    parser.add_argument('--no-render', action='store_true')
    parser.add_argument('--overwrite', action='store_true')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else [])
    output = (ROOT / args.output).resolve()
    texture_path = output / 'textures/jovara-clouds.png'
    if not texture_path.is_file():
        raise FileNotFoundError(f'Falta la textura de referencia: {texture_path}')
    if not args.overwrite and any(path.exists() for path in (
            output / 'blender/jovara.blend', output / 'jovara.obj', output / 'jovara.mtl')):
        raise FileExistsError('Jovara ya existe; usa --overwrite para regenerarlo')
    (output / 'blender').mkdir(parents=True, exist_ok=True)
    (output / 'previews').mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.mesh.primitive_uv_sphere_add(segments=128, ring_count=64, radius=1)
    planet = bpy.context.object
    planet.name = 'Jovara | atmosfera polar azul y crema'
    mesh = planet.data
    uv = mesh.uv_layers.new(name='PolarClouds')
    mesh.uv_layers.active_index = len(mesh.uv_layers) - 1
    # The full NASA photograph is 997 x 1280 and contains black margins. Sample
    # an inscribed ellipse so the mesh retains cloud colors around the entire limb.
    for loop in mesh.loops:
        p = mesh.vertices[loop.vertex_index].co.normalized()
        uv.data[loop.index].uv = ((498.5 + p.x * 490.0) / 996.0,
                                  1.0 - (652.5 - p.z * 518.0) / 1279.0)
    for face in mesh.polygons:
        face.use_smooth = True
    planet.rotation_euler.x = 0.24
    bpy.context.view_layer.update()
    material = bpy.data.materials.new('Jovara | nubes polares de Juno')
    material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    nodes.clear()
    image = bpy.data.images.load(str(texture_path), check_existing=True)
    image.pack()
    image.filepath = '//../textures/jovara-clouds.png'
    texture = nodes.new('ShaderNodeTexImage')
    texture.image = image
    texture.interpolation = 'Linear'
    uv_node = nodes.new('ShaderNodeUVMap')
    uv_node.uv_map = 'PolarClouds'
    shader = nodes.new('ShaderNodeEmission')
    shader.inputs['Strength'].default_value = 1.0
    final = nodes.new('ShaderNodeOutputMaterial')
    links.new(uv_node.outputs['UV'], texture.inputs['Vector'])
    links.new(texture.outputs['Color'], shader.inputs['Color'])
    links.new(shader.outputs[0], final.inputs['Surface'])
    mesh.materials.append(material)
    planet['referencia'] = SOURCE
    planet['creditos'] = CREDIT
    planet['proyeccion'] = 'Fotografia polar sobre esfera; hemisferio posterior como continuacion artistica'
    planet_collection = bpy.data.collections.new('01 | Planeta editable')
    bpy.context.scene.collection.children.link(planet_collection)
    for owner in tuple(planet.users_collection):
        owner.objects.unlink(planet)
    planet_collection.objects.link(planet)
    stats = export(planet, output)
    studio('jovara', args.size, args.samples)
    scene = bpy.context.scene
    scene.camera.location = (0, -7.5, -1.835)
    aim(scene.camera)
    scene.camera.data.type = 'ORTHO'
    scene.camera.data.ortho_scale = 2.16
    for screen in bpy.data.screens:
        for area in screen.areas:
            if area.type == 'VIEW_3D':
                area.spaces.active.region_3d.view_rotation = scene.camera.rotation_euler.to_quaternion()
                area.spaces.active.region_3d.view_distance = 3.8
    scene.view_settings.view_transform = 'Standard'
    scene.view_settings.look = 'None'
    scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value = 0.003
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
    scene['planeta'] = 'Jovara'
    scene['creditos_imagen'] = CREDIT
    scene.render.filepath = '//../previews/jovara.png'
    bpy.ops.wm.save_as_mainfile(filepath=str(output / 'blender/jovara.blend'), compress=True, relative_remap=False)
    if not args.no_render:
        bpy.ops.render.render(write_still=True)
    stats_path = output / 'mesh_stats.json'
    catalog = json.loads(stats_path.read_text(encoding='utf-8')) if stats_path.exists() else {}
    catalog['jovara'] = stats
    stats_path.write_text(json.dumps(catalog, indent=2) + '\n', encoding='utf-8')
    print('JOVARA LISTO', json.dumps(stats), flush=True)


if __name__ == '__main__':
    main()

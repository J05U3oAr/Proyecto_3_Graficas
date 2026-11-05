"""Create editable Blender planets and color-only OBJ assets for the Rust renderer.

Run with Blender, not the system Python:
  blender --background --factory-startup --python scripts/blender/create_planets.py
Optional arguments follow Blender's -- separator; see -- --help.
"""

import argparse
import json
import math
from pathlib import Path
import random
import sys

import bpy
import bmesh
from mathutils import Vector, noise


ROOT = Path(__file__).resolve().parents[2]
TAU = math.tau
TUNNEL_AXIS = Vector((0.16, -1.0, 0.20)).normalized()
STORM_CENTER = Vector((0.16, -1.0, 0.24)).normalized()
STORM_RIGHT = STORM_CENTER.cross(Vector((0, 0, 1))).normalized()
STORM_UP = STORM_RIGHT.cross(STORM_CENTER).normalized()
SAND = [(0.20, 0.065, 0.022), (0.48, 0.22, 0.065),
        (0.73, 0.43, 0.17), (0.91, 0.66, 0.34), (1.0, 0.86, 0.60)]
STONE = [(0.075, 0.11, 0.14), (0.18, 0.25, 0.28),
         (0.34, 0.40, 0.40), (0.53, 0.56, 0.52), (0.68, 0.65, 0.55)]


def clamp(value, low=0.0, high=1.0):
    return max(low, min(high, value))


def mix(a, b, factor):
    return tuple(x + (y - x) * factor for x, y in zip(a, b))


def palette(colors, value):
    value = clamp(value) * (len(colors) - 1)
    index = min(int(value), len(colors) - 2)
    return mix(colors[index], colors[index + 1], value - index)


def linear(color):
    return tuple(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in color)


def fbm(point, octaves=4):
    total, weight, scale = 0.0, 0.0, 1.0
    for octave in range(octaves):
        amplitude = 0.5 ** octave
        total += noise.noise(point * scale, noise_basis='PERLIN_ORIGINAL') * amplitude
        weight += amplitude
        scale *= 2.07
    return total / weight


def storm_field(point):
    p = point.normalized()
    latitude = math.asin(clamp(p.z, -1, 1))
    longitude = math.atan2(p.y, p.x)
    turbulence = fbm(p * 5.0)
    bands = math.sin(latitude * 30 + 5 * math.sin(longitude * 2 + latitude * 3)
                     + turbulence * 3.5)
    radius = math.acos(clamp(p.dot(STORM_CENTER), -1, 1))
    angle = math.atan2(p.dot(STORM_UP), p.dot(STORM_RIGHT))
    spiral = math.sin(angle * 3 - radius * 40 + turbulence * 6.0
                      + 0.9 * math.sin(angle * 5 + radius * 13))
    influence = math.exp(-(radius / 0.78) ** 4)
    value = 0.56 + bands * 0.20 + turbulence * 0.22
    value = value * (1 - influence * 0.85) + (0.57 + spiral * 0.30) * influence * 0.85
    eye = math.exp(-(radius / 0.065) ** 2)
    eyewall = math.exp(-((radius - 0.10) / 0.035) ** 2)
    value = value * (1 - eye * 0.7) + eyewall * 0.22
    return clamp(value), spiral, influence


def sand_color(point, normal=None):
    return palette(SAND, storm_field(point)[0])


def stone_color(point, normal=None):
    rocky = fbm(point * 5.5)
    grain = fbm(point * 30, 2)
    color = palette(STONE, 0.52 + rocky * 0.42 + grain * 0.16)
    wall = normal is not None and normal.dot(point.normalized()) < 0.72
    seam = abs(fbm(point * 10, 3))
    if wall:
        color = mix(color, (0.035, 0.42, 0.38), 0.48)
        if seam < 0.055:
            color = mix(color, (0.18, 0.95, 0.73), 0.75)
    elif seam < 0.018:
        color = mix(color, (0.12, 0.48, 0.40), 0.36)
    return color


def material(name, roughness=0.8, glow=False, cloud=False):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    nodes.clear()
    output = nodes.new('ShaderNodeOutputMaterial')
    output.location = (640, 80)
    shader = nodes.new('ShaderNodeBsdfPrincipled')
    shader.location = (200, 80)
    shader.inputs['Roughness'].default_value = roughness
    colors = nodes.new('ShaderNodeVertexColor')
    colors.layer_name = 'planet_color'
    colors.location = (-500, 180)
    links.new(colors.outputs['Color'], shader.inputs['Base Color'])
    texture = nodes.new('ShaderNodeTexNoise')
    texture.inputs['Scale'].default_value = 75 if not cloud else 14
    texture.inputs['Detail'].default_value = 4
    texture.inputs['Roughness'].default_value = 0.7
    texture.location = (-500, -100)
    bump = nodes.new('ShaderNodeBump')
    bump.inputs['Strength'].default_value = 0.42 if not cloud else 0.20
    bump.inputs['Distance'].default_value = 0.045 if not cloud else 0.025
    bump.location = (-50, -100)
    links.new(texture.outputs['Fac'], bump.inputs['Height'])
    links.new(bump.outputs['Normal'], shader.inputs['Normal'])
    if glow:
        attribute = nodes.new('ShaderNodeAttribute')
        attribute.attribute_name = 'vein_mask'
        strength = nodes.new('ShaderNodeMath')
        strength.operation = 'MULTIPLY'
        strength.inputs[1].default_value = 2.0
        shader.inputs['Emission Color'].default_value = (*linear((0.10, 0.90, 0.68)), 1)
        links.new(attribute.outputs['Fac'], strength.inputs[0])
        links.new(strength.outputs[0], shader.inputs['Emission Strength'])
    if cloud:
        transparent = nodes.new('ShaderNodeBsdfTransparent')
        blend = nodes.new('ShaderNodeMixShader')
        blend.location = (430, 80)
        links.new(colors.outputs['Alpha'], blend.inputs[0])
        links.new(transparent.outputs[0], blend.inputs[1])
        links.new(shader.outputs[0], blend.inputs[2])
        links.new(blend.outputs[0], output.inputs['Surface'])
    else:
        links.new(shader.outputs[0], output.inputs['Surface'])
    return mat


def color_mesh(obj, color_function, cloudy=False, glow=False, phase=0.0):
    mesh = obj.data
    mesh.update()
    colors = mesh.color_attributes.new(name='planet_color', type='FLOAT_COLOR', domain='CORNER')
    mask = mesh.attributes.new(name='vein_mask', type='FLOAT', domain='CORNER') if glow else None
    for polygon in mesh.polygons:
        normal = polygon.normal
        for loop_index in polygon.loop_indices:
            point = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            color = color_function(point, normal)
            alpha = 1.0
            if cloudy:
                value, spiral, influence = storm_field(point)
                curl = math.sin(spiral * 2.0 + fbm(point * 15) * 4 + phase)
                alpha = clamp(0.13 + value * 0.24 + curl * 0.22 + influence * 0.09, 0.04, 0.66)
                eye_radius = math.acos(clamp(point.normalized().dot(STORM_CENTER), -1, 1))
                alpha *= 1 - math.exp(-(eye_radius / 0.07) ** 2) * 0.85
                color = mix(color, SAND[-1], 0.10)
            colors.data[loop_index].color = (*linear(color), alpha)
            if mask:
                wall = normal.dot(point.normalized()) < 0.72
                seam = abs(fbm(point * 10, 3))
                mask.data[loop_index].value = (1 - seam / 0.055) * 0.75 if wall and seam < 0.055 else 0
        polygon.use_smooth = True


def surface_sphere(name, radius=1.0, cloud=False):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=144, ring_count=80, radius=radius)
    obj = bpy.context.object
    obj.name = name
    for vertex in obj.data.vertices:
        p = vertex.co.normalized()
        relief = fbm(p * 12, 3) * (0.010 if cloud else 0.006)
        if cloud:
            relief += max(0, storm_field(p)[1]) * 0.007
        vertex.co *= 1 + relief
    obj.data.update()
    return obj


def build_siroco():
    surface = surface_sphere('Siroco | relieve bajo la tormenta')
    color_mesh(surface, sand_color)
    surface.data.materials.append(material('Siroco | arena y corrientes globales'))
    surface['caracteristicas'] = 'Tormenta de arena planetaria, ciclon gigante, dunas, viento continuo'
    for index, radius in enumerate((1.018, 1.035)):
        clouds = surface_sphere(f'Siroco | polvo animado {index + 1}', radius, cloud=True)
        color_mesh(clouds, sand_color, cloudy=True, phase=index * 1.7)
        clouds.data.materials.append(material(f'Siroco | velo de polvo {index + 1}', cloud=True))
        driver = clouds.driver_add('rotation_euler', 2).driver
        driver.expression = f'(frame - 1) * {0.004 + index * 0.0015}'
        clouds['exportar_al_juego'] = False
    return [surface]


def apply_modifier(obj, modifier):
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=modifier.name)


def tunnel(obj, name, radius, center, direction):
    direction = Vector(direction).normalized()
    bpy.ops.mesh.primitive_cylinder_add(vertices=48, radius=radius, depth=3.4, location=center)
    cutter = bpy.context.object
    cutter.name = name
    cutter.rotation_euler = direction.to_track_quat('Z', 'Y').to_euler()
    bpy.context.view_layer.update()
    modifier = obj.modifiers.new(name, 'BOOLEAN')
    modifier.operation = 'DIFFERENCE'
    modifier.solver = 'EXACT'
    modifier.object = cutter
    apply_modifier(obj, modifier)
    data = cutter.data
    bpy.data.objects.remove(cutter, do_unlink=True)
    bpy.data.meshes.remove(data)

def build_oquedad():
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=5, radius=1)
    planet = bpy.context.object
    planet.name = 'Oquedad | corteza con tuneles atravesables'
    for vertex in planet.data.vertices:
        point = vertex.co.copy()
        vertex.co *= 1 + fbm(point * 6) * 0.080 + fbm(point * 22, 2) * 0.025
    planet.data.update()
    cuts = [
        ('Paso central', 0.34, (0, 0, 0.10), TUNNEL_AXIS),
        ('Paso occidental', 0.23, (-0.58, 0, 0.26), TUNNEL_AXIS),
        ('Paso oriental', 0.25, (0.55, 0, -0.20), TUNNEL_AXIS),
        ('Conexion transversal', 0.23, (0, 0.04, 0.08), (1, 0.18, 0.10)),
        ('Conexion polar', 0.25, (-0.08, 0.14, 0), (0.20, 0.08, 1)),
    ]
    for name, radius, center, direction in cuts:
        tunnel(planet, name, radius, center, direction)
    if len(planet.data.polygons) < 3000:
        raise RuntimeError('Los cortes deben conservar una corteza completa alrededor de los tuneles')
    bevel = planet.modifiers.new('Bordes erosionados de las cavidades', 'BEVEL')
    bevel.width = 0.014
    bevel.segments = 2
    bevel.limit_method = 'ANGLE'
    bevel.angle_limit = 0.48
    apply_modifier(planet, bevel)
    # Warp the finished passages together, preserving connections while eroding the rims.
    for vertex in planet.data.vertices:
        point = vertex.co.copy()
        vertex.co += noise.noise_vector(point * 6) * 0.035 + noise.noise_vector(point * 18) * 0.012
    planet.data.update()
    check = bmesh.new()
    check.from_mesh(planet.data)
    if not all(edge.is_manifold for edge in check.edges):
        check.free()
        raise RuntimeError('La corteza con tuneles debe ser una malla cerrada')
    check.free()
    color_mesh(planet, stone_color, glow=True)
    # Boolean cutters can leave an empty material slot at index zero.
    planet.data.materials.clear()
    planet.data.materials.append(material('Oquedad | roca y vetas interiores', glow=True))
    planet['caracteristicas'] = 'Cinco tuneles interconectados, corteza rocosa, vetas turquesa'
    planet['tuneles_reales'] = len(cuts)
    bpy.context.view_layer.update()
    hit = planet.ray_cast(Vector((0, 0, 0.10)) - TUNNEL_AXIS * 3, TUNNEL_AXIS)[0]
    if hit:
        raise RuntimeError('El paso central debe atravesar completamente el planeta')
    return [planet]


def export_obj(objects, destination, kind):
    """Triangulate a LOD and write only v/f/usemtl/Kd understood by Model::load."""
    vertices, triangles, materials = [], [], {}
    for source in objects:
        copy = bpy.data.objects.new('_export_lod', source.data.copy())
        bpy.context.scene.collection.objects.link(copy)
        copy.matrix_world = source.matrix_world.copy()
        decimate = copy.modifiers.new('LOD para rasterizador CPU', 'DECIMATE')
        decimate.ratio = 0.34 if kind == 'siroco' else 0.30
        decimate.use_collapse_triangulate = True
        triangulate = copy.modifiers.new('Triangulos de exportacion', 'TRIANGULATE')
        depsgraph = bpy.context.evaluated_depsgraph_get()
        evaluated = copy.evaluated_get(depsgraph)
        mesh = evaluated.to_mesh()
        mesh.calc_loop_triangles()
        offset = len(vertices)
        world = copy.matrix_world
        vertices.extend(world @ vertex.co for vertex in mesh.vertices)
        color_function = sand_color if kind == 'siroco' else stone_color
        for triangle in mesh.loop_triangles:
            indices = tuple(offset + index for index in triangle.vertices)
            center = sum((vertices[index] for index in indices), Vector()) / 3
            normal = (vertices[indices[1]] - vertices[indices[0]]).cross(
                vertices[indices[2]] - vertices[indices[0]])
            # Keep tiny Boolean sliver faces so the exported mesh remains watertight.
            shading_normal = normal.normalized() if normal.length_squared > 1e-15 else center.normalized()
            color = color_function(center, shading_normal)
            quantized = tuple(round(clamp(channel) * 31) for channel in color)
            if quantized not in materials:
                materials[quantized] = f'tint_{len(materials):03d}'
            triangles.append((indices, materials[quantized]))
        evaluated.to_mesh_clear()
        data = copy.data
        bpy.data.objects.remove(copy, do_unlink=True)
        bpy.data.meshes.remove(data)
    with destination.open('w', encoding='utf-8', newline='\n') as obj:
        obj.write('# Created in Blender; color-only CPU renderer LOD\n')
        obj.write(f'mtllib {destination.stem}.mtl\no {kind}\n')
        for p in vertices:
            # Blender Z-up -> the game's Y-up, preserving winding and scale.
            obj.write(f'v {p.x:.8f} {p.z:.8f} {-p.y:.8f}\n')
        previous = None
        for indices, mat in triangles:
            if mat != previous:
                obj.write(f'usemtl {mat}\n')
                previous = mat
            obj.write('f ' + ' '.join(str(index + 1) for index in indices) + '\n')
    with destination.with_suffix('.mtl').open('w', encoding='utf-8', newline='\n') as mtl:
        for color, name in materials.items():
            mtl.write(f'newmtl {name}\nKd ' + ' '.join(f'{v / 31:.6f}' for v in color) + '\n\n')
    print(f'EXPORT {kind}: {len(vertices)} vertices, {len(triangles)} triangulos, {len(materials)} colores', flush=True)
    return {'vertices': len(vertices), 'triangles': len(triangles), 'materials': len(materials)}


def aim(obj, point=(0, 0, 0)):
    obj.rotation_euler = (Vector(point) - obj.location).to_track_quat('-Z', 'Y').to_euler()


def area(name, location, energy, color, size):
    data = bpy.data.lights.new(name, type='AREA')
    data.energy, data.color, data.shape, data.size = energy, color, 'DISK', size
    obj = bpy.data.objects.new(name, data)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = location
    aim(obj)


def studio(kind, size, samples):
    scene = bpy.context.scene
    scene.render.engine = 'CYCLES'
    scene.cycles.device = 'CPU'
    scene.cycles.samples = samples
    scene.cycles.use_denoising = True
    scene.cycles.transparent_max_bounces = 12
    scene.render.resolution_x = scene.render.resolution_y = size
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGBA'
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, 480
    scene.world = bpy.data.worlds.new('Espacio profundo')
    scene.world.use_nodes = True
    scene.world.node_tree.nodes['Background'].inputs['Color'].default_value = (0.018, 0.026, 0.045, 1)
    scene.world.node_tree.nodes['Background'].inputs['Strength'].default_value = 0.22
    camera_data = bpy.data.cameras.new('Camara de presentacion')
    camera = bpy.data.objects.new('Camara de presentacion', camera_data)
    scene.collection.objects.link(camera)
    camera.location = (1.25, -7.5, 1.7)
    camera_data.lens = 78
    aim(camera)
    scene.camera = camera
    area('Luz principal', (-3.5, -4.5, 5), 650 if kind == 'siroco' else 900,
         (1.0, 0.84, 0.64) if kind == 'siroco' else (0.78, 0.89, 1.0), 4.0)
    area('Relleno tenue', (4, -2, 0), 75 if kind == 'siroco' else 120, (0.58, 0.74, 1.0), 3.0)
    area('Contraluz', (1.5, 3, 2.5), 1100, (1.0, 0.53, 0.20) if kind == 'siroco' else (0.20, 1.0, 0.78), 2.7)
    # Tiny distant emissive meshes keep the silhouette and tunnel exits legible.
    rng = random.Random(812)
    star_material = bpy.data.materials.new('Estrellas de presentacion')
    star_material.use_nodes = True
    shader = star_material.node_tree.nodes.get('Principled BSDF')
    shader.inputs['Base Color'].default_value = (0.5, 0.7, 1.0, 1)
    shader.inputs['Emission Color'].default_value = (0.45, 0.65, 1.0, 1)
    shader.inputs['Emission Strength'].default_value = 3
    for index in range(90):
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=rng.uniform(0.003, 0.009),
                                            location=(rng.uniform(-5, 5), 4, rng.uniform(-4, 4)))
        star = bpy.context.object
        star.name = f'Estrella de estudio {index:03d}'
        star.data.materials.append(star_material)
    for screen in bpy.data.screens:
        for view in screen.areas:
            if view.type == 'VIEW_3D':
                view.spaces.active.region_3d.view_distance = 4.3
                view.spaces.active.region_3d.view_rotation = camera.rotation_euler.to_quaternion()
                view.spaces.active.shading.type = 'MATERIAL'
    scene.frame_set(1)


def generate(kind, output, size, samples, render):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.preferences.filepaths.save_version = 0
    objects = build_siroco() if kind == 'siroco' else build_oquedad()
    planet_collection = bpy.data.collections.new('01 | Planeta editable')
    bpy.context.scene.collection.children.link(planet_collection)
    for obj in tuple(bpy.context.scene.objects):
        for owner in tuple(obj.users_collection):
            owner.objects.unlink(obj)
        planet_collection.objects.link(obj)
    stats = export_obj(objects, output / f'{kind}.obj', kind)
    planet_objects = set(bpy.context.scene.objects)
    studio(kind, size, samples)
    presentation = bpy.data.collections.new('02 | Presentacion - ocultar para editar')
    bpy.context.scene.collection.children.link(presentation)
    for obj in set(bpy.context.scene.objects) - planet_objects:
        for owner in tuple(obj.users_collection):
            owner.objects.unlink(obj)
        presentation.objects.link(obj)
    for obj in bpy.context.selected_objects:
        obj.select_set(False)
    objects[0].select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    scene = bpy.context.scene
    scene['planeta'] = kind
    scene['exportacion_obj'] = f'../{kind}.obj'
    scene['nota'] = 'OBJ de menor detalle para el rasterizador Rust. Luces y estrellas solo para presentacion.'
    scene.render.filepath = f'//../previews/{kind}.png'
    bpy.ops.wm.save_as_mainfile(filepath=str(output / 'blender' / f'{kind}.blend'),
                              compress=True, relative_remap=False)
    if render:
        bpy.ops.render.render(write_still=True)
    return stats


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', default='assets/planets')
    parser.add_argument('--only', choices=['siroco', 'oquedad'])
    parser.add_argument('--size', type=int, default=960)
    parser.add_argument('--samples', type=int, default=40)
    parser.add_argument('--no-render', action='store_true')
    parser.add_argument('--overwrite', action='store_true')
    args = parser.parse_args(sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else [])
    output = (ROOT / args.output).resolve()
    for directory in (output, output / 'blender', output / 'previews'):
        directory.mkdir(parents=True, exist_ok=True)
    kinds = [args.only] if args.only else ['siroco', 'oquedad']
    for kind in kinds:
        if not args.overwrite and any(path.exists() for path in (
                output / 'blender' / f'{kind}.blend', output / f'{kind}.obj', output / f'{kind}.mtl')):
            raise FileExistsError(f'{kind} ya existe; usa --overwrite para regenerarlo')
    stats_file = output / 'mesh_stats.json'
    results = json.loads(stats_file.read_text(encoding='utf-8')) if stats_file.exists() else {}
    for kind in kinds:
        print(f'CREANDO {kind}', flush=True)
        results[kind] = generate(kind, output, args.size, args.samples, not args.no_render)
    stats_file.write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
    print('PLANETAS LISTOS', json.dumps(results), flush=True)


if __name__ == '__main__':
    main()

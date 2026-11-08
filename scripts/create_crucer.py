"""Build the reference-inspired C-137 saucer using only Python's standard library.

Run: python scripts/create_crucer.py
Coordinates match the game: +Y up, +Z nose. The OBJ is triangulated, its MTL
contains the clear canopy's dissolve, and named parts remain editable in a DCC.
No downloaded mesh, external textures, Blender installation or runtime generator.
"""

from math import cos, sin, pi, sqrt
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'assets' / 'ships'
TAU = 2 * pi

MATERIALS = {
    'Hull': ((0.48, 0.49, 0.44), 1),
    'HullLight': ((0.53, 0.54, 0.49), 1),
    'HullDark': ((0.43, 0.45, 0.40), 1),
    'Belly': ((0.19, 0.21, 0.19), 1),
    'Rim': ((0.27, 0.29, 0.26), 1),
    'Metal': ((0.36, 0.38, 0.35), 1),
    'Rubber': ((0.10, 0.12, 0.11), 1),
    'Engine': ((0.32, 0.34, 0.32), 1),
    'EngineRib': ((0.41, 0.43, 0.40), 1),
    'EngineInside': ((0.15, 0.17, 0.16), 1),
    'Yellow': ((0.91, 0.69, 0.14), 1),
    'StripeEdge': ((0.45, 0.35, 0.10), 1),
    'Lamp': ((0.94, 0.88, 0.39), 1),
    'Seat': ((0.47, 0.28, 0.17), 1),
    'Cushion': ((0.58, 0.36, 0.23), 1),
    'SeatSeam': ((0.34, 0.21, 0.14), 1),
    'Dashboard': ((0.26, 0.31, 0.29), 1),
    'Screen': ((0.25, 0.65, 0.58), 1),
    'Glass': ((0.69, 0.83, 0.85), 0.22),
    'GlassReflection': ((0.90, 0.96, 0.95), 0.43),
    'Sticker': ((0.55, 0.78, 0.77), 1),
    'Cream': ((0.80, 0.78, 0.59), 1),
    'Red': ((0.65, 0.23, 0.15), 1),
}


def add(a, b):
    return tuple(x + y for x, y in zip(a, b))


def mul(a, scale):
    return tuple(x * scale for x in a)


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0])


def unit(a):
    return mul(a, 1 / sqrt(sum(x * x for x in a)))


class Mesh:
    def __init__(self):
        self.vertices = []
        self.parts = []
        self.current = None

    def part(self, name, material):
        self.current = []
        self.parts.append((name, material, self.current))

    def vertex(self, p):
        self.vertices.append(p)
        return len(self.vertices)

    def face(self, indices):
        for i in range(1, len(indices) - 1):
            tri = (indices[0], indices[i], indices[i + 1])
            a, b, c = (self.vertices[j - 1] for j in tri)
            normal = cross(add(b, mul(a, -1)), add(c, mul(a, -1)))
            if sum(x * x for x in normal) > 1e-16:
                self.current.append(tri)

    def polygon(self, points):
        self.face([self.vertex(p) for p in points])

    def grid(self, rings, closed=True):
        indices = [[self.vertex(p) for p in ring] for ring in rings]
        count = len(indices[0])
        for low, high in zip(indices, indices[1:]):
            for j in range(count if closed else count - 1):
                k = (j + 1) % count
                self.face([low[j], high[j], high[k], low[k]])
        return indices

    def lathe(self, name, material, center, axis, profile, segments=48, caps=False):
        self.part(name, material)
        axis = unit(axis)
        right = unit(cross(axis, (0, 1, 0) if abs(axis[1]) < 0.9 else (0, 0, 1)))
        up = cross(axis, right)
        rings = [[add(add(center, mul(axis, height)),
                      add(mul(right, radius * cos(TAU * j / segments)),
                          mul(up, radius * sin(TAU * j / segments))))
                  for j in range(segments)] for height, radius in profile]
        indices = self.grid(rings)
        if caps:
            self.face(list(reversed(indices[0])))
            self.face(indices[-1])

    def cylinder(self, name, material, a, b, radius, segments=16):
        delta = add(b, mul(a, -1))
        self.lathe(name, material, a, delta,
                   [(0, radius), (sqrt(sum(x * x for x in delta)), radius)],
                   segments, caps=True)

    def tube(self, name, material, points, radius, segments=8):
        # Shared-ring swept mesh, so transparent/opaque frame seams do not overlap.
        self.part(name, material)
        rings = []
        for i, point in enumerate(points):
            tangent = unit(add(points[min(i + 1, len(points) - 1)],
                               mul(points[max(i - 1, 0)], -1)))
            right = unit(cross(tangent, (0, 1, 0) if abs(tangent[1]) < .9 else (1, 0, 0)))
            up = cross(tangent, right)
            rings.append([add(point, add(mul(right, radius * cos(TAU * j / segments)),
                                         mul(up, radius * sin(TAU * j / segments))))
                          for j in range(segments)])
        self.grid(rings)

    def box(self, name, material, center, size, bevel=.025):
        self.part(name, material)
        hx, hy, hz = (x / 2 for x in size)
        b = min(bevel, hx * .8, hy * .8, hz * .8)
        rings = []
        for height, inset in [(-hy, b), (-hy + b, 0), (hy - b, 0), (hy, b)]:
            x, z = hx - inset, hz - inset
            c = max(.001, b - inset)
            outline = [(-x + c, -z), (x - c, -z), (x, -z + c), (x, z - c),
                       (x - c, z), (-x + c, z), (-x, z - c), (-x, -z + c)]
            rings.append([add(center, (u, height, v)) for u, v in outline])
        indices = self.grid(rings)
        self.face(list(reversed(indices[0])))
        self.face(indices[-1])

    def ellipsoid(self, name, material, center, radii, segments=24, rows=12):
        self.part(name, material)
        self.grid([[add(center, (radii[0] * sin(pi * i / rows) * cos(TAU * j / segments),
                                  radii[1] * cos(pi * i / rows),
                                  radii[2] * sin(pi * i / rows) * sin(TAU * j / segments)))
                    for j in range(segments)] for i in range(rows + 1)])

    def write(self):
        OUTPUT.mkdir(parents=True, exist_ok=True)
        lines = ['# Crucer espacial: reference-inspired original procedural mesh',
                 '# +Y up / +Z forward; regenerate with python scripts/create_crucer.py',
                 'mtllib Crucer_espacial.mtl']
        lines.extend('v ' + ' '.join(f'{x:.6f}' for x in p) for p in self.vertices)
        for name, material, faces in self.parts:
            lines.extend([f'o {name}', f'usemtl {material}'])
            lines.extend('f ' + ' '.join(map(str, face)) for face in faces)
        (OUTPUT / 'Crucer_espacial.obj').write_text('\n'.join(lines) + '\n', encoding='utf-8')
        lines = ['# Materials for Crucer espacial; dissolve is read by the CPU renderer.']
        for name, (color, opacity) in MATERIALS.items():
            lines.extend([f'newmtl {name}', 'Kd ' + ' '.join(map(str, color)),
                          f'd {opacity}', 'illum 2', ''])
        (OUTPUT / 'Crucer_espacial.mtl').write_text('\n'.join(lines), encoding='utf-8')
        print(f'Crucer espacial: {len(self.vertices)} vertices, '
              f'{sum(len(p[2]) for p in self.parts)} triangles, {len(self.parts)} parts')


# Upper hull: broad flat cone, rolled outer edge, and a shallow dark belly.
PROFILE = [(0, -.53), (.72, -.53), (1.35, -.47), (1.90, -.34),
           (2.16, -.23), (2.22, -.16), (2.18, -.07), (1.91, .10),
           (1.55, .30), (1.23, .43), (0, .43)]


def hull_y(radius):
    upper = [(1.23, .43), (1.55, .30), (1.91, .10), (2.18, -.07), (2.22, -.16)]
    for (r0, y0), (r1, y1) in zip(upper, upper[1:]):
        if r0 <= radius <= r1:
            return y0 + (y1 - y0) * (radius - r0) / (r1 - r0)
    return .43


def hull_point(angle, radius, lift=.007):
    return (radius * sin(angle), hull_y(radius) + lift, radius * cos(angle))


def build_hull(mesh):
    for sector in range(12):
        mesh.part(f'Hull_panel_{sector:02}', ['Hull', 'HullLight', 'Hull', 'HullDark'][sector % 4])
        for i in range(8):
            a, b = TAU * (sector + i / 8) / 12, TAU * (sector + (i + 1) / 8) / 12
            for (r0, y0), (r1, y1) in zip(PROFILE, PROFILE[1:]):
                mesh.polygon([(r0 * sin(a), y0, r0 * cos(a)),
                              (r1 * sin(a), y1, r1 * cos(a)),
                              (r1 * sin(b), y1, r1 * cos(b)),
                              (r0 * sin(b), y0, r0 * cos(b))])
    mesh.lathe('Underbody_reactor', 'Belly', (0, 0, 0), (0, 1, 0),
               [(-.57, .65), (-.55, 1.02), (-.43, 1.35)], 64, True)
    mesh.lathe('Outer_rolled_lip', 'Rim', (0, 0, 0), (0, 1, 0),
               [(-.24, 2.15), (-.20, 2.21), (-.16, 2.22)], 96)
    for index, angle in enumerate([0, .93, -.93, pi, pi - .93, pi + .93]):
        # Narrow golden straps follow the cone and curl over the saucer's edge.
        for material, width, lift in [('StripeEdge', .032, .009), ('Yellow', .020, .013)]:
            mesh.part(f'Safety_strap_{index}_{material}', material)
            for (r0, y0), (r1, y1) in zip(PROFILE[4:10], PROFILE[5:10]):
                mesh.polygon([(r0 * sin(angle - width), y0 + lift, r0 * cos(angle - width)),
                              (r1 * sin(angle - width), y1 + lift, r1 * cos(angle - width)),
                              (r1 * sin(angle + width), y1 + lift, r1 * cos(angle + width)),
                              (r0 * sin(angle + width), y0 + lift, r0 * cos(angle + width))])
        for radius in [1.42, 2.04]:
            p = hull_point(angle, radius, .023)
            mesh.ellipsoid(f'Strap_rivet_{index}_{radius}', 'Cream', p, (.018, .01, .018), 6, 3)
    for index, angle in enumerate([pi, -.9, .9]):
        x, z = .87 * sin(angle), .87 * cos(angle)
        mesh.cylinder(f'Landing_strut_{index}', 'Metal', (x, -.40, z), (x, -.61, z), .055, 10)
        mesh.ellipsoid(f'Landing_foot_{index}', 'Cream', (x, -.61, z), (.12, .09, .12), 12, 6)


def build_cockpit(mesh):
    mesh.lathe('Cockpit_floor', 'Dashboard', (0, 0, 0), (0, 1, 0),
               [(.428, 1.19), (.46, 1.19)], 64, True)
    mesh.lathe('Canopy_black_gasket', 'Rubber', (0, 0, 0), (0, 1, 0),
               [(.40, 1.19), (.425, 1.28), (.49, 1.27), (.515, 1.20)], 64)
    mesh.lathe('Canopy_metal_rim', 'Metal', (0, 0, 0), (0, 1, 0),
               [(.49, 1.27), (.52, 1.24), (.535, 1.19)], 64)
    for i in range(20):
        angle = TAU * i / 20
        mesh.ellipsoid(f'Canopy_rivet_{i}', 'EngineRib',
                       (1.25 * sin(angle), .48, 1.25 * cos(angle)), (.021, .022, .021), 6, 3)
    for i, x in enumerate([-.43, .43]):
        mesh.box(f'Seat_{i}_pedestal', 'Rubber', (x, .51, -.12), (.29, .11, .32))
        mesh.box(f'Seat_{i}_base', 'Seat', (x, .63, -.10), (.48, .20, .54), .065)
        mesh.box(f'Seat_{i}_cushion', 'Cushion', (x, .735, -.04), (.42, .09, .42), .035)
        mesh.box(f'Seat_{i}_back', 'Seat', (x, .95, -.34), (.48, .56, .17), .065)
        mesh.box(f'Seat_{i}_back_pad', 'Cushion', (x, .97, -.24), (.38, .43, .055), .025)
        mesh.box(f'Seat_{i}_headrest', 'Seat', (x, 1.275, -.32), (.29, .22, .17), .055)
        for side in [-1, 1]:
            mesh.box(f'Seat_{i}_arm_{side}', 'Seat', (x + side * .26, .79, -.06),
                     (.075, .10, .36), .022)
        for y in [.86, .98, 1.10]:
            mesh.box(f'Seat_{i}_stitch_{y}', 'SeatSeam', (x, y, -.207), (.34, .011, .008), .002)
    # Front instrument coaming with two rounded grey lobes, like the reference.
    for i, x in enumerate([-.46, .46]):
        mesh.ellipsoid(f'Dashboard_coaming_{i}', 'Dashboard', (x, .59, .68), (.52, .25, .37), 24, 10)
        mesh.box(f'Instrument_screen_{i}', 'Screen', (x, .74, .52), (.21, .05, .12), .009)
    mesh.box('Center_console', 'Rubber', (0, .61, .13), (.18, .24, .45), .035)
    for i in range(4):
        mesh.box(f'Console_button_{i}', 'Red' if i == 0 else 'Yellow',
                 (0, .74, -.01 + .08 * i), (.045, .025, .035), .008)
    # Driver's steering wheel, set vertically across the cabin.
    center = (-.43, .83, .39)
    wheel = [add(center, (.15 * cos(TAU * i / 32), .15 * sin(TAU * i / 32), 0)) for i in range(33)]
    mesh.tube('Steering_wheel', 'Rubber', wheel, .018, 6)
    mesh.cylinder('Steering_spoke', 'Metal', (-.58, .83, .39), (-.28, .83, .39), .013, 8)
    # Thin single shell: alpha is composited after the opaque interior in all views.
    mesh.part('Clear_bubble_canopy', 'Glass')
    mesh.grid([[(1.19 * cos(pi * row / 32) * sin(TAU * j / 64),
                 .52 + 1.17 * sin(pi * row / 32),
                 1.19 * cos(pi * row / 32) * cos(TAU * j / 64))
                for j in range(64)] for row in range(17)])
    arch = [(0, .52 + 1.18 * sin(pi * i / 48), 1.205 * cos(pi * i / 48)) for i in range(49)]
    mesh.tube('Canopy_center_arch', 'Rim', arch, .026, 6)
    # Small off-white reflection streaks help read the transparent bubble in space.
    for index, angle in enumerate([-.83, 2.4]):
        mesh.part(f'Glass_reflection_{index}', 'GlassReflection')
        mesh.grid([[(1.194 * cos(phi) * sin(angle + delta),
                     .52 + 1.174 * sin(phi),
                     1.194 * cos(phi) * cos(angle + delta)) for delta in [-.022, .022]]
                   for phi in [.26 + i * .035 for i in range(21)]], closed=False)


def build_engines(mesh):
    for side in [-1, 1]:
        x, y = side * 1.96, .34
        mesh.box(f'Engine_mount_{side}', 'Rim', (side * 1.63, .17, -.38), (.48, .23, .53), .06)
        mesh.lathe(f'Engine_canister_{side}', 'Engine', (x, y, 0), (0, 0, 1),
                   [(-1.03, .32), (-.98, .41), (-.87, .44), (.25, .44), (.34, .39)], 32, True)
        for z in [-.86, .18]:
            mesh.lathe(f'Engine_band_{side}_{z}', 'Rim', (x, y, z), (0, 0, 1),
                       [(-.035, .443), (0, .462), (.05, .462), (.075, .443)], 32)
        for i in range(10):
            angle = TAU * i / 10
            dx, dy = .435 * cos(angle), .435 * sin(angle)
            mesh.cylinder(f'Canister_rib_{side}_{i}', 'EngineRib',
                          (x + dx, y + dy, -.78), (x + dx, y + dy, .12), .022, 6)
        # Recessed dark front caps and ringed exhausts at the back, not solid blocks.
        mesh.lathe(f'Engine_front_rim_{side}', 'Rubber', (x, y, 0), (0, 0, 1),
                   [(.32, .39), (.36, .41), (.39, .41), (.395, .35)], 32)
        mesh.lathe(f'Engine_front_cap_{side}', 'EngineInside', (x, y, 0), (0, 0, 1),
                   [(.355, .35), (.36, .35)], 32, True)
        mesh.lathe(f'Engine_exhaust_{side}', 'Rubber', (x, y, 0), (0, 0, 1),
                   [(-1.07, .32), (-1.10, .28), (-1.13, .24)], 32)
        mesh.lathe(f'Engine_exhaust_core_{side}', 'EngineInside', (x, y, 0), (0, 0, 1),
                   [(-1.08, .24), (-1.09, .24)], 32, True)
        mesh.box(f'Engine_top_clasp_{side}', 'Cream', (x, .80, -.52), (.16, .05, .14), .018)


FONT = {
    'C': ['111', '100', '100', '100', '111'],
    '-': ['000', '000', '111', '000', '000'],
    '1': ['010', '110', '010', '010', '111'],
    '3': ['111', '001', '111', '001', '111'],
    '7': ['111', '001', '010', '010', '010'],
}


def decal(mesh, name, material, angle, radius, width, height, lift=.012, rotation=0):
    def point(u, v):
        u, v = u * cos(rotation) - v * sin(rotation), u * sin(rotation) + v * cos(rotation)
        return hull_point(angle + u / radius, radius + v, lift)
    mesh.part(name, material)
    # Subdivide labels to follow the cone rather than cutting through its crown.
    columns = max(1, int(width / .06))
    rows = max(1, int(height / .06))
    mesh.grid([[point(width * (col / columns - .5), height * (row / rows - .5))
                for col in range(columns + 1)] for row in range(rows + 1)], closed=False)


def build_details(mesh):
    for side in [-1, 1]:
        angle, radius = side * .68, 1.87
        direction = (sin(angle), .24, cos(angle))
        p = hull_point(angle, radius, .08)
        mesh.cylinder(f'Headlight_housing_{side}', 'Rubber', p, add(p, mul(direction, .14)), .14, 24)
        mesh.cylinder(f'Headlight_metal_bezel_{side}', 'Metal', add(p, mul(direction, .13)),
                      add(p, mul(direction, .175)), .126, 28)
        mesh.cylinder(f'Headlight_yellow_lens_{side}', 'Lamp', add(p, mul(direction, .176)),
                      add(p, mul(direction, .182)), .103, 32)
        for offset in [-.16, .16]:
            q = add(p, (offset * cos(angle), 0, -offset * sin(angle)))
            mesh.tube(f'Headlight_bracket_{side}_{offset}', 'Rim',
                      [add(q, (0, -.03, -.025)), add(q, (0, .23, -.055)),
                       add(q, mul(direction, .12))], .023, 8)
    # Salvaged handles on the outer rim.
    for side in [-1, 1]:
        angle = side * 1.20
        points = []
        for u, y in [(-.12, .025), (-.12, .28), (.12, .28), (.12, .025)]:
            points.append(add(hull_point(angle + u / 2.04, 2.04), (0, y, 0)))
        mesh.tube(f'Outer_handle_{side}', 'Rubber', points, .025)
        mesh.box(f'Handle_tape_{side}', 'Cream', points[1], (.08, .095, .035), .005)
    # The readable registration sticker is geometry, so it needs no image texture.
    angle, radius = -.36, 1.78
    decal(mesh, 'Registration_sticker', 'Sticker', angle, radius, .62, .31)
    pixel = .024
    for index, char in enumerate('C-137'):
        for row, line in enumerate(FONT[char]):
            for col, value in enumerate(line):
                if value == '1':
                    u = (index * 4 + col - 9) * pixel
                    v = (row - 2) * pixel
                    decal(mesh, f'Registration_ink_{index}_{row}_{col}', 'Rubber',
                          angle + u / radius, radius + v, pixel * .83, pixel * .83, .022)
    for i, (a, r, color) in enumerate([(-.58, 2.02, 'Cream'), (-.66, 1.99, 'Red'), (-.25, 2.06, 'Cream')]):
        decal(mesh, f'Warning_sticker_{i}', color, a, r, .12, .09, .015, .12)
        for row in range(2):
            decal(mesh, f'Warning_ink_{i}_{row}', 'Rubber', a, r + (row - .5) * .026,
                  .075, .009, .025)
    # A rear access hatch and worn service vents are visible during the 360° turn.
    decal(mesh, 'Rear_service_hatch_edge', 'Rim', pi - .32, 1.77, .46, .30)
    decal(mesh, 'Rear_service_hatch', 'HullDark', pi - .32, 1.77, .41, .26, .02)
    for i in range(7):
        decal(mesh, f'Rear_vent_{i}', 'Rubber', pi + .26, 1.58 + i * .047, .25, .017, .017)


def main():
    mesh = Mesh()
    build_hull(mesh)
    build_cockpit(mesh)
    build_engines(mesh)
    build_details(mesh)
    mesh.write()


if __name__ == '__main__':
    main()

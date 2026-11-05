# Primeros planetas de la galaxia

Los archivos `.blend` son editables y contienen todos sus recursos. Siroco y Oquedad
usan mallas y materiales procedurales. Jovara reproduce la referencia polar con una
textura fotográfica empaquetada dentro de su escena. Boreal añade un gigante
helado con bandas turquesa y blancas. Los cuatro están incorporados
al modo de vuelo: Siroco aparece a la izquierda al despegar, Oquedad a la derecha
y Jovara arriba, entre ellos. Boreal se ve abajo a la izquierda.

[Vista de los planetas dentro del juego](previews/en-vuelo.png).

| Planeta | Características | Archivo editable | Exportación para Rust |
| --- | --- | --- | --- |
| **Siroco** | Planeta desértico cubierto por una tormenta global. Gran ciclón, corrientes de arena y dos capas de polvo animadas a distintas velocidades. | `blender/siroco.blend` | `siroco.obj` + `siroco.mtl` |
| **Oquedad** | Mundo rocoso con cinco túneles conectados. Las aberturas son geometría real; sus bordes son irregulares y las paredes interiores tienen vetas turquesa. | `blender/oquedad.blend` | `oquedad.obj` + `oquedad.mtl` |
| **Jovara** | Centro azul petróleo, ciclones blancos y nubes beige. Recreación de la referencia de Júpiter mediante textura UV, conservando su detalle. | `blender/jovara.blend` | `jovara.obj` + `jovara.mtl` + `textures/jovara-clouds.png` |
| **Boreal** | Gigante helado turquesa, bandas blancas, remolinos oscuros y eje inclinado. Textura artística guiada por la referencia del usuario. | `blender/boreal.blend` | `boreal.obj` + `boreal.mtl` + `textures/boreal-clouds.png` |

## Abrir y editar en Blender

1. Abre cualquiera de los archivos de `blender/` desde **File > Open**.
2. La colección **01 | Planeta editable** contiene el planeta y, en Siroco, sus capas de polvo.
3. La colección **02 | Presentacion - ocultar para editar** agrupa cámara, luces y estrellas de estudio. Puedes ocultarla para trabajar cómodamente en la malla.
4. Pulsa **F12** para renderizar la cámara de presentación. Las imágenes ya generadas están en `previews/`.
5. En Siroco, reproduce la línea de tiempo para ver las dos capas de polvo girando. Sus velocidades están en los drivers de rotación Z de cada capa.

Si usas la copia portable de esta sesión, el ejecutable está en
`tmp/blender-runtime/blender-5.2.3-windows-x64/blender.exe`. Ábrelo y carga el `.blend`
desde **File > Open**; no hace falta instalar Blender para revisar los modelos.

Los modificadores booleanos de Oquedad ya están aplicados: puedes inspeccionar y
editar directamente las paredes y los pasos. El script conserva los parámetros
originales de los cinco cortes para poder regenerarlos.

## Compatibilidad con el juego

Los cuatro planetas tienen colisión continua con su malla y rotación de vuelo.
La nave puede atravesar los túneles de Oquedad; sus paredes y bordes sí bloquean
el paso. Los impactos fuertes activan la [explosión y reaparición](../effects/README.md).

Las exportaciones usan triángulos y materiales de color difuso `Kd`. Jovara añade
coordenadas UV y un mapa PNG mediante `map_Kd`; su material `illum 0` conserva la
iluminación que ya contiene la fotografía. El eje vertical se convierte de Z en Blender a Y en el juego. Los
modelos están centrados cerca del origen y tienen un radio aproximado de una unidad.

El OBJ contiene únicamente el planeta: las luces, cámara, estrellas de estudio y
capas transparentes de polvo se conservan en el `.blend`. Siroco mantiene el patrón
de la tormenta mediante colores en su superficie exportada. El juego añade su
propia nube animada alrededor de Siroco con 4,800 partículas 3D y transparencia;
sus capas de polvo giran y se mezclan respetando la profundidad de las superficies.
La nube funciona con el tiempo de vuelo incluso si la nave está detenida.
Las capas de Blender siguen siendo editables como referencia visual. OBJ/MTL
conserva la malla y los colores estáticos; la emisión de las vetas de Oquedad
se mantiene en el material de Blender.

Los OBJ de Siroco y Oquedad tienen menor detalle que las mallas editables para reducir
el trabajo del rasterizador CPU. Jovara utiliza una esfera de 16,128 triángulos con
textura UV para conservar los pequeños ciclones. `mesh_stats.json` registra los recuentos generados.

Se comprobó que los cuatro OBJ se cargan con `Model::load` y se dibujan con el
renderizador Rust actual. Las capturas `previews/siroco-cpu.png` y
`previews/oquedad-cpu.png` muestran esa versión de colores estáticos. La abertura
central de Oquedad también deja ver el fondo en ese renderizador.

## Jovara y la referencia

La superficie reproduce la fotografía **Southern Storms (PIA21641)** del polo sur
de Júpiter. Conserva la región central azul petróleo, las tormentas claras y las
nubes beige exteriores. [Ver modelo renderizado en Blender](previews/jovara.png)
o [con el rasterizador Rust](previews/jovara-cpu.png).

La imagen está empaquetada en el `.blend` y su copia PNG queda en `textures/` para
el juego. La cara posterior es una continuación artística de la referencia, que
solo muestra una vista polar. Créditos y procedencia en [textures/README.md](textures/README.md).

## Regenerar los modelos

Para recrear Boreal con su textura incluida:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/blender/generate_planets.ps1 -Planet boreal -Overwrite
```

Su generador es `scripts/blender/create_boreal.py`. La esfera tiene 16,128
triángulos, UV e iluminación sobre la geometría. La textura está empaquetada
en el `.blend`, por lo que puedes abrirlo y editarlo sin buscar imágenes externas.
[Vista en Blender](previews/boreal.png) y [vista del renderizador Rust](previews/boreal-cpu.png).

Boreal ocupa (−145, −85, −235), tiene radio 42 y gira lentamente a 0.012 rad/s.
Es una recreación artística del planeta de la referencia, no un mapa científico
de Urano. El atlas y su prompt están documentados en [textures/README.md](textures/README.md).

Generador: `scripts/blender/create_planets.py`. Se ejecuta con el Python incluido
en Blender. Se usó [Blender 5.2.3 LTS, descargado del sitio oficial](https://www.blender.org/download/).

Desde la raíz del repositorio, con Blender disponible en PATH:

```powershell
blender --background --factory-startup --python-exit-code 1 --python scripts/blender/create_planets.py -- --overwrite
```

También puedes usar `scripts/blender/generate_planets.ps1`, que busca Blender en
PATH, en su instalación habitual y en la copia portable de `tmp/blender-runtime/`:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/blender/generate_planets.ps1 -Overwrite
```

Para indicar un ejecutable instalado en otra ubicación:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/blender/generate_planets.ps1 -BlenderPath 'C:\ruta\blender.exe' -Overwrite
```

Opciones del generador Python después de `--`: `--only siroco`, `--only oquedad`,
`--samples 64`, `--size 960`, `--no-render` y `--output assets/planets`.
Sin `--overwrite`, el generador se detiene si ya existen modelos con esos nombres.

Para recrear Jovara con la textura incluida, usa su generador específico:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/blender/generate_planets.ps1 -Planet jovara -Overwrite
```

También puedes ejecutar `scripts/blender/create_jovara.py` directamente con Blender.
La opción `-Planet` del script PowerShell permite seleccionar `siroco`, `oquedad`
`jovara` o `boreal`; al omitirla, se generan los dos modelos procedurales originales.

Los nombres y parámetros son propuestas iniciales. La colección puede ampliarse
con otros seis planetas para completar la primera colección de diez.

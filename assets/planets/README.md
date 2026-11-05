# Primeros planetas de la galaxia

Dos planetas originales creados en Blender. Los archivos `.blend` son editables y no
dependen de imágenes externas: sus superficies, colores y relieves se generan con
mallas y materiales procedurales. Esta primera entrega contiene los modelos para
revisar su aspecto antes de incorporarlos al vuelo.

| Planeta | Características | Archivo editable | Exportación para Rust |
| --- | --- | --- | --- |
| **Siroco** | Planeta desértico cubierto por una tormenta global. Gran ciclón, corrientes de arena y dos capas de polvo animadas a distintas velocidades. | `blender/siroco.blend` | `siroco.obj` + `siroco.mtl` |
| **Oquedad** | Mundo rocoso con cinco túneles conectados. Las aberturas son geometría real; sus bordes son irregulares y las paredes interiores tienen vetas turquesa. | `blender/oquedad.blend` | `oquedad.obj` + `oquedad.mtl` |

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

Las exportaciones usan triángulos y materiales de color difuso `Kd`, que ya reconoce
`src/model.rs`. El eje vertical se convierte de Z en Blender a Y en el juego. Los
modelos están centrados cerca del origen y tienen un radio aproximado de una unidad.

El OBJ contiene únicamente el planeta: las luces, cámara, estrellas de estudio y
capas transparentes de polvo se conservan en el `.blend`. Siroco mantiene el patrón
de la tormenta mediante colores en su superficie exportada. La transparencia, la
animación del polvo y la emisión de las vetas requieren soporte adicional en el
renderizador del juego; OBJ/MTL conserva la malla y los colores estáticos.

Los OBJ tienen menor detalle que las mallas editables para reducir el trabajo del
rasterizador CPU. `mesh_stats.json` registra los recuentos generados.

Se comprobó que los dos OBJ se cargan con `Model::load` y se dibujan con el
renderizador Rust actual. Las capturas `previews/siroco-cpu.png` y
`previews/oquedad-cpu.png` muestran esa versión de colores estáticos. La abertura
central de Oquedad también deja ver el fondo en ese renderizador.

## Regenerar los modelos

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

Los nombres y parámetros son propuestas iniciales. La colección puede ampliarse
con los otros ocho planetas una vez definido el aspecto de estos dos.

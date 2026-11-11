# Nuevos mundos de Hélice

Tres modelos originales de fantasía espacial con cordilleras y cráteres reales.
El acabado facetado permite leer las paredes, las cimas y los cambios de estrato.
Cada carpeta contiene una escena Blender editable, un OBJ triangulado, sus
materiales MTL y `mesh_stats.json` con los recuentos del generador.
No se descargan recursos de videojuegos ni se utilizan texturas de imagen.

| Mundo | Tema | Relieve y elementos | Triángulos |
| --- | --- | --- | --- |
| Micelia | Bosque fúngico violeta y esmeralda | Cordilleras, 5 cuencas de impacto y 58 hongos gigantes; tallos y sombreros separados de la corteza | 33,240 |
| Azufria | Azufre amarillo y ácido jade | Cordilleras, 7 calderas con paredes elevadas y 7 superficies de lago | 20,816 |
| Ferrita | Cobre, óxido y cardenillo | Cordilleras estratificadas, 8 cráteres con doble borde y pico central de rebote | 20,480 |

[Micelia](../../../docs/assets/micelia.png),
[Azufria](../../../docs/assets/azufria.png),
[Ferrita](../../../docs/assets/ferrita.png) y
[vista de vuelo con los siete mundos](../../../docs/assets/vuelo.png).

## Abrir y regenerar

Abre `micelia/micelia.blend`, `azufria/azufria.blend` o `ferrita/ferrita.blend`.
Cada archivo contiene solo su mundo, materiales y una copia del generador como
texto de referencia. La corteza es una malla continua; cada hongo y cada lago
son objetos editables independientes. Los materiales emisivos de las láminas
de Micelia se conservan en Blender.

Desde la raíz del repositorio, con Blender en PATH:

```powershell
blender --background --factory-startup --python-exit-code 1 --python scripts/blender/create_helix_worlds.py -- --overwrite
```

La copia portable disponible en este checkout también permite ejecutar:

```powershell
.\tmp\blender-runtime\blender-5.2.3-windows-x64\blender.exe --background --factory-startup --python-exit-code 1 --python scripts/blender/create_helix_worlds.py -- --overwrite
```

Para regenerar un solo mundo, añade `--only micelia`, `--only azufria` o
`--only ferrita`. `--output` permite elegir otra carpeta. Sin `--overwrite`,
el script rechaza reemplazar modelos existentes.

Las semillas, paletas, perfiles de los cráteres y posiciones de sus centros están
en `WORLDS`. La corteza parte de una icosfera uniforme para conservar detalle
comparable en todas las latitudes. El radio se modifica con cordilleras continuas,
ruido suave, cuencas hundidas y bordes elevados. Los cráteres aplanan el relieve
interior; los lagos intersectan sus paredes para formar la orilla.

## Integración con el juego

| Mundo | Radio orbital alrededor de Aureon | Radio exterior | Rotación |
| --- | --- | --- | --- |
| Micelia | 1460 | 62 | 0.009 rad/s |
| Azufria | 1760 | 53 | 0.012 rad/s |
| Ferrita | 2110 | 49 | 0.010 rad/s |

Aureon está en (−380, −110, −420), tiene radio 320 y también es un modelo
editable de Blender. Los siete planetas de Hélice tienen órbitas inclinadas
separadas. Sus posiciones, velocidades orbitales y referencias de vuelo local
se actualizan con el mismo reloj. [Modelo y parámetros de Aureon](../aureon/README.md).

Los tres mundos se activan únicamente en Hélice. Sus índices se añaden al final
del catálogo para conservar los identificadores existentes. El cambio de galaxia
reconstruye las colisiones y las referencias de navegación local; el campo de
asteroides conserva espacio libre alrededor de los nuevos planetas.

Rust carga las mallas una vez, mantiene su origen geográfico y usa las mismas
caras para renderizado y colisiones. El shader rocoso aplica variación leve fija
sobre los materiales, sin desplazar las montañas ni animar la corteza. Las láminas
de Micelia usan `illum 0` para conservar su luz. Los lagos de Azufria son superficies
opacas con colisión; el ácido es una temática visual y no añade daño especial.
El material metálico de Blender se representa mediante colores e iluminación
en el rasterizador CPU, sin un modelo de reflexión física.

Las pruebas cubren montañas elevadas, cuencas profundas, contacto con la malla
rotada en cimas y fondos, activación exclusiva en Hélice, navegación local y la
ruta de cambio de destino, despegue, impacto, explosión y reaparición.

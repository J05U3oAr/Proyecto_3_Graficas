# Texturas de los planetas

## Boreal: gigante helado

`boreal-clouds.png` es un atlas equirectangular generado con la herramienta
integrada ImageGen, guiado por la imagen aportada por el usuario. Conserva la
paleta turquesa, las bandas blancas de aspecto helado y los vórtices azul oscuro.
Es una recreación artística para el juego, no una fotografía ni un mapa
científico de Urano. El prompt completo se conserva en `boreal-prompt.txt`.

La textura cubre toda la esfera con UV estándar, sin sombras direccionales
dibujadas: Blender y el juego iluminan la geometría. Está empaquetada dentro de
`blender/boreal.blend` y se conserva también aquí para el material OBJ/MTL.
El material de Blender y el muestreador del juego mezclan una franja UV de 0.035
en la unión de los extremos del atlas para suavizar la costura al rodear el planeta.

## Jovara

`jovara-clouds.png` es una conversión a PNG de la imagen **Southern Storms,
PIA21641**. Se conservan los colores y el detalle de la fotografía: no se generaron
nuevas nubes ni se retocó su contenido.

- Fuente: [NASA Photojournal, Southern Storms](https://science.nasa.gov/photojournal/southern-storms-2/).
- Archivo original: [PIA21641.jpg](https://assets.science.nasa.gov/dynamicimage/assets/science/psd/photojournal/pia/pia21/pia21641/PIA21641.jpg).
- Crédito: NASA/JPL-Caltech/SwRI/MSSS/Betsy Asher Hall/Gervasio Robles.

El modelo aplica una región elíptica de la fotografía a una esfera para evitar
los márgenes negros del archivo original. La cara visible reproduce la referencia;
la posterior reutiliza sus nubes como continuación artística. Es una adaptación
para el juego, no un mapa científico completo de Júpiter.

La textura está empaquetada en `blender/jovara.blend` y también se mantiene aquí
para que el cargador OBJ/MTL del juego pueda leerla.

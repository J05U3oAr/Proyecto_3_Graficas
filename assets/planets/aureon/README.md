# Aureon, gran sol de Hélice

Sol dorado original de radio 320, centrado en (−380, −110, −420). Ilumina el
sistema de Hélice, sus naves, sus planetas y sus asteroides. Su fotosfera tiene
convección y manchas animadas por fragmento, con corona cálida y profundidad
correcta. No utiliza una textura de imagen. Su esfera tiene 4,514 vértices y
9,024 triángulos y se carga una sola vez.

`aureon.blend` contiene la esfera editable, un material emisivo procedural y
una copia del generador como texto de referencia. `aureon.obj` y `aureon.mtl`
son las exportaciones para el motor Rust. `illum 0` mantiene la emisión propia.

```powershell
blender --background --factory-startup --python-exit-code 1 --python scripts/blender/create_aureon.py -- --overwrite
```

El script también acepta `--output` para elegir otra carpeta. Sin `--overwrite`
rechaza reemplazar la escena existente.

| Planeta | Radio orbital | Velocidad angular (rad/s) | Inclinación (rad) |
| --- | --- | --- | --- |
| Siroco | 490 | 0.0160 | 0.060 |
| Oquedad | 710 | 0.0130 | −0.080 |
| Jovara | 950 | 0.0105 | 0.070 |
| Boreal | 1190 | 0.0085 | −0.060 |
| Micelia | 1460 | 0.0068 | 0.120 |
| Azufria | 1760 | 0.0055 | −0.100 |
| Ferrita | 2110 | 0.0045 | 0.055 |

Las órbitas son trayectorias prescritas que conservan sus radios y separación.
El mismo movimiento determina imagen, velocidad de contacto y referencia de
navegación local. Los planetas mantienen sus mallas, radios y rotaciones propias.
El sol queda fuera de las referencias que pueden activar el vuelo local.

[Vista del sol](../../../docs/assets/aureon.png) y
[vuelo en su sistema](../../../docs/assets/vuelo.png).

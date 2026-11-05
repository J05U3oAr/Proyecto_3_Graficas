# Proyecto de gráficas - naves espaciales

Hangar interactivo en Rust con vistas previas 360° para las tres naves y vuelo libre por un campo estelar con los planetas Siroco, Oquedad y Jovara.

Los modelos se cargan desde archivos OBJ; el programa transforma y rasteriza los triángulos en un framebuffer propio con búfer de profundidad. `minifb` se usa para la ventana y la entrada, no para dibujar modelos 3D. El menú también se dibuja en la CPU, incluidos textos, tarjetas, estrellas y plataformas luminosas.

## Ejecutar

Instala Rust y Cargo y, desde esta carpeta, ejecuta:

```bash
cargo run --release
```

El primer inicio compila las dependencias declaradas en `Cargo.toml`.

## Controles del hangar

Haz clic en una tarjeta y pulsa **Volar** para entrar al espacio.

| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Seleccionar Guardgalaxy, Galactwar o Lightflash |
| `Tab` / `Shift+Tab` o flechas | Cambiar de selección |
| `Enter` | Volar con la nave seleccionada |
| `Espacio` o botón de pausa | Pausar / reanudar los tres giros |
| `P` | Guardar una captura del menú en `render.png` |
| `Esc` | Salir |

La animación usa tiempo transcurrido y se pausa cuando la ventana pierde el foco. Se puede redimensionar la ventana: la interfaz conserva sus proporciones y la selección con ratón se ajusta a su tamaño.

## Controles de vuelo

| Tecla | Acción |
| --- | --- |
| Mouse | Dirigir la nave: izquierda/derecha para girar, arriba/abajo para subir o bajar |
| Mouse al centro | Mantener el rumbo recto |
| `W` / `S` | Avanzar / retroceder según la dirección de la nave |
| `A` / `D` | Desplazarse lateralmente a izquierda / derecha |
| `Shift` | Aumentar el impulso y la velocidad máxima |
| `Espacio` | Frenar |
| `1`, `2`, `3` | Cambiar de nave |
| `I` | Inspeccionar la nave actual |
| `Esc` | Volver al hangar |

La nave comienza detenida y se desplaza mientras mantienes WASD. Puedes combinar teclas para moverte en diagonal sin aumentar la velocidad máxima; las teclas opuestas se cancelan. Al soltarlas, la nave desacelera suavemente hasta detenerse. `Shift` aumenta la velocidad mientras te desplazas y `Espacio` frena incluso si mantienes una tecla de movimiento.

El mouse dirige la nave sin mantener clic. La posición del cursor respecto al centro controla el giro: cuanto más lo alejas, más rápido gira; al volver al centro, mantiene el rumbo. Una pequeña zona neutra evita movimientos involuntarios. Al salir el cursor de la ventana, el giro se suaviza hasta detenerse. Para subir o bajar, apunta con el mouse y avanza con `W`.

La cámara está detrás y ligeramente por encima de la nave, mirando de la cola hacia la proa. Sigue los cambios de dirección con suavidad y la nave inclina las alas al girar. Las tres naves se dibujan en perspectiva y las estrellas se desplazan según la posición y velocidad reales, con estelas al avanzar. El HUD muestra posición y velocidad. El vuelo se pausa mientras inspeccionas la nave o la ventana pierde el foco.

Al despegar, Siroco aparece a la izquierda, Oquedad a la derecha y Jovara arriba,
entre los dos. Ocupan posiciones
fijas en el espacio: puedes acercarte, rodearlos y observar los túneles de Oquedad.
Sus modelos originales y escenas editables están en [assets/planets](assets/planets/README.md).

Jovara recrea la referencia del polo de Júpiter con su centro azul y ciclones claros.
Utiliza una textura PNG sobre una esfera 3D, con filtrado bilineal e interpolación
UV corregida por perspectiva para conservar los detalles al acercarte o rodearlo.
Su `.blend` contiene la textura empaquetada y sus créditos de NASA/Juno están
documentados junto al modelo.

Siroco está envuelto por una nube de arena de 4,800 partículas: capas suaves de polvo
y granos que circulan alrededor del planeta. La nube respeta la profundidad de la
nave y los planetas, y sigue animándose aunque estés detenido. Las tormentas y la
rotación de los planetas se pausan al inspeccionar la nave o perder el foco; la
pausa de las vistas previas del hangar no afecta al tiempo del vuelo.

## Controles de inspección


| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Elegir Guardgalaxy, Galactwar o Lightflash |
| Flechas o `W`, `A`, `S`, `D` | Orbitar la cámara |
| `Q`, `E` | Alejar o acercar |
| `M` | Cambiar entre sólido, malla y holograma |
| `R` | Reiniciar la vista de la nave seleccionada |
| `P` | Guardar `render.png` |
| `Esc` o botón de volver | Regresar al vuelo |

## Exportar vistas sin abrir una ventana

```bash
cargo run --release -- --render-menu output/hangar.png --time 4 --ship 1
cargo run --release -- --render-inspector output/lightflash.png --ship 3
cargo run --release -- --render-flight output/vuelo.png --ship 1
cargo run --release -- --render-flight output/giro.png --ship 2 --time 2 --mouse 0.65 -0.35
```

`--time` indica el instante de la animación en segundos; `--ship` elige una nave de 1 a 3. Para vuelo, `--time` simula hasta 600 segundos manteniendo `W` y `--mouse x y` mantiene una dirección normalizada entre -1 y 1 (centro: `0 0`; derecha y arriba: `0.65 -0.35`). Estas exportaciones usan la misma física y el mismo renderizador que la aplicación interactiva.

## Organización

- `src/fleet.rs`: modelos, nombres y acabados de las naves.
- `src/flight.rs`: posición, velocidad, impulso y controles de vuelo.
- `src/space.rs`: carga, posición y rotación de los planetas.
- `src/dust.rs`: nube animada de arena de Siroco y partículas con transparencia.
- `src/texture.rs`: carga y muestreo bilineal de mapas de color PNG.
- `src/renderer.rs`: rasterizado compartido, vistas del hangar y perspectiva de vuelo.
- `src/ui.rs`: composición, tipografía, interacción con las tarjetas y mapeo del ratón.
- `src/main.rs`: navegación, entradas y reloj de animación.
- `assets/ui/`: atlas de glifos integrado en el ejecutable; no requiere instalar fuentes.

Las pruebas de `cargo test` comprueban los giros completos del hangar, pausa/reanudación, selección con ventana redimensionada y animación de los tres modelos sin salir de sus tarjetas. También verifican la dirección sostenida con mouse, el vuelo sin saltos, movimiento WASD sin avance automático ni ventaja diagonal, desaceleración al soltar las teclas, turbo/freno, respuesta a distintas tasas de cuadros, cámara trasera durante giros completos, visibilidad de las tres naves y desplazamiento del campo estelar. Los planetas tienen comprobaciones de visibilidad al despegar, profundidad independiente del orden de dibujo, recorte cerca de la cámara y movimiento de la nube sin desplazar la nave.

## Integración y procedencia

La integración, el port a Rust y el menú se realizaron con asistencia de Codex.

- **Guardgalaxy**: modelo `Benatar.obj`, del proyecto [hmndzzl/Graficas_Lab4](https://github.com/hmndzzl/Graficas_Lab4). Su MTL original no define colores difusos, por lo que el hangar aplica un acabado cobrizo inspirado en el visualizador original.
- **Galactwar**: modelo `nave-lab3-PC.obj`, del proyecto [Pxdro-410/lab4-Graficas-PC](https://github.com/Pxdro-410/lab4-Graficas-PC). El ZIP no incluía el MTL referenciado, así que `Galactwar.mtl` completa los cuatro materiales que aparecen en el OBJ.
- **Lightflash**: modelo `Nave_Espacial.obj` y materiales `Sin_nombre.mtl`, del proyecto [J05U3oAr/Lab4-graficas](https://github.com/J05U3oAr/Lab4-graficas).

El renderizador C++ del último proyecto se portó a módulos Rust: carga de OBJ/MTL, validación y triangulación de caras, rasterizado baricéntrico con profundidad, iluminación difusa y exportación PNG. La ventana y los controles usan `minifb`. No se conservaron archivos `.cpp`, `.h` ni dependencias de SDL.

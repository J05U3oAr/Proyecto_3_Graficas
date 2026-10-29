# Proyecto de gráficas - naves espaciales

Hangar interactivo en Rust con vistas previas 360° para las tres naves. Un giro completo tarda 18 segundos; selecciona una nave para abrir su inspección.

Los modelos se cargan desde archivos OBJ; el programa transforma y rasteriza los triángulos en un framebuffer propio con búfer de profundidad. `minifb` se usa para la ventana y la entrada, no para dibujar modelos 3D. El menú también se dibuja en la CPU, incluidos textos, tarjetas, estrellas y plataformas luminosas.

## Ejecutar

Instala Rust y Cargo y, desde esta carpeta, ejecuta:

```bash
cargo run --release
```

El primer inicio compila las dependencias declaradas en `Cargo.toml`.

## Controles del hangar

Haz clic en una tarjeta y pulsa **Abrir nave** para inspeccionarla.

| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Seleccionar Guardgalaxy, Galactwar o Lightflash |
| `Tab` / `Shift+Tab` o flechas | Cambiar de selección |
| `Enter` | Abrir la nave seleccionada |
| `Espacio` o botón de pausa | Pausar / reanudar los tres giros |
| `P` | Guardar una captura del menú en `render.png` |
| `Esc` | Salir |

La animación usa tiempo transcurrido y se pausa cuando la ventana pierde el foco. Se puede redimensionar la ventana: la interfaz conserva sus proporciones y la selección con ratón se ajusta a su tamaño.

## Controles de inspección

| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Elegir Guardgalaxy, Galactwar o Lightflash |
| Flechas o `W`, `A`, `S`, `D` | Orbitar la cámara |
| `Q`, `E` | Alejar o acercar |
| `M` | Cambiar entre sólido, malla y holograma |
| `R` | Reiniciar la vista de la nave seleccionada |
| `P` | Guardar `render.png` |
| `Esc` o botón de volver | Regresar al hangar |

## Exportar vistas sin abrir una ventana

```bash
cargo run --release -- --render-menu output/hangar.png --time 4 --ship 1
cargo run --release -- --render-inspector output/lightflash.png --ship 3
```

`--time` indica el instante de la animación en segundos; `--ship` elige una nave de 1 a 3. Estas exportaciones usan el mismo renderizador que la aplicación interactiva.

## Organización

- `src/fleet.rs`: modelos, nombres y acabados de las naves.
- `src/renderer.rs`: pipeline compartido por tarjetas e inspección.
- `src/ui.rs`: composición, tipografía, interacción con las tarjetas y mapeo del ratón.
- `src/main.rs`: navegación, entradas y reloj de animación.
- `assets/ui/`: atlas de glifos integrado en el ejecutable; no requiere instalar fuentes.

Las pruebas de `cargo test` comprueban los giros completos, pausa/reanudación, selección con ventana redimensionada y animación de los tres modelos sin salir de sus tarjetas.

## Integración y procedencia

La integración, el port a Rust y el menú se realizaron con asistencia de Codex.

- **Guardgalaxy**: modelo `Benatar.obj`, del proyecto [hmndzzl/Graficas_Lab4](https://github.com/hmndzzl/Graficas_Lab4). Su MTL original no define colores difusos, por lo que el hangar aplica un acabado cobrizo inspirado en el visualizador original.
- **Galactwar**: modelo `nave-lab3-PC.obj`, del proyecto [Pxdro-410/lab4-Graficas-PC](https://github.com/Pxdro-410/lab4-Graficas-PC). El ZIP no incluía el MTL referenciado, así que `Galactwar.mtl` completa los cuatro materiales que aparecen en el OBJ.
- **Lightflash**: modelo `Nave_Espacial.obj` y materiales `Sin_nombre.mtl`, del proyecto [J05U3oAr/Lab4-graficas](https://github.com/J05U3oAr/Lab4-graficas).

El renderizador C++ del último proyecto se portó a módulos Rust: carga de OBJ/MTL, validación y triangulación de caras, rasterizado baricéntrico con profundidad, iluminación difusa y exportación PNG. La ventana y los controles usan `minifb`. No se conservaron archivos `.cpp`, `.h` ni dependencias de SDL.

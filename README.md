# Beyond the Interstellar

Exploración espacial en Rust con un menú principal, un hangar con selección lateral y vista 3D de la nave, y un mapa de galaxias. El destino elegido configura los planetas, sus posiciones, el fondo y la decoración del vuelo.

Los modelos se cargan desde archivos OBJ; el programa transforma y rasteriza los triángulos en un framebuffer propio con búfer de profundidad. `minifb` se usa para la ventana y la entrada, no para dibujar modelos 3D. El menú también se dibuja en la CPU, incluidos textos, tarjetas, estrellas y plataformas luminosas.

## Ejecutar

Instala Rust y Cargo y, desde esta carpeta, ejecuta:

```bash
cargo run --release
```

El primer inicio compila las dependencias declaradas en `Cargo.toml`. Rayon reparte la rasterización de planetas, la mezcla del polvo y el cálculo por píxel del agujero negro entre los núcleos disponibles. El pipeline gráfico y la física siguen implementados en el proyecto; Rayon no proporciona modelos, cámaras ni funciones de dibujo.

## Menú principal

La aplicación inicia en **Beyond the Interstellar**, con el título grande sobre la nebulosa y una flecha junto a la opción seleccionada:

- **Seleccionar nave:** abre el hangar.
- **Elegir destino:** abre directamente el mapa con la nave seleccionada; al iniciar se usa Guardgalaxy.
- **Controles:** muestra las teclas de navegación, vuelo e inspección.
- **Instrucciones:** explica cómo preparar la nave, elegir un destino y explorar.
- **Salir:** cierra el juego.

Usa flechas arriba/abajo, `W/S`, `Tab` o `Shift+Tab` para cambiar de opción y `Enter` para activarla. El ratón también permite seleccionar y hacer clic. En las pantallas de ayuda, `Esc`, `Enter` o el botón de volver regresan al inicio. En la portada, `Esc` selecciona **Salir** y `Enter` confirma.

## Controles del hangar

Selecciona una nave en la columna izquierda. Su nombre aparece como título principal y la vista central muestra el modelo girando; la ficha derecha contiene datos reales de su geometría y los parámetros compartidos de vuelo. Pulsa **Elegir destino** para continuar al mapa.

Los menús usan el fondo procedural adaptado de `referencia.rs`: una nebulosa con ruido fractal, franjas de polvo oscuro y estrellas de semilla fija. Se genera una sola vez y se conserva en memoria; el mapa utiliza una variación violeta de la misma nebulosa. No necesita imágenes externas.

| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Seleccionar Guardgalaxy, Galactwar o Crucer espacial |
| `Tab` / `Shift+Tab` o flechas | Cambiar de selección |
| `Enter` | Continuar al selector de galaxias |
| `Espacio` o botón de pausa | Pausar / reanudar el giro de la nave principal |
| `P` | Guardar una captura del menú en `render.png` |
| `Esc` o **Menú principal** | Volver al inicio |

La animación usa tiempo transcurrido y se pausa cuando la ventana pierde el foco. Se puede redimensionar la ventana: la interfaz conserva sus proporciones y la selección con ratón se ajusta a su tamaño.

## Selector de galaxias

La pantalla **Destinos** permite elegir tres regiones explorables. El mapa ocupa toda la ventana y gira lentamente de forma automática. La descripción aparece únicamente al seleccionar un destino mediante clic o teclado; se puede cerrar con la cruz o haciendo clic en el espacio vacío. Las regiones reutilizan los modelos existentes con distintas combinaciones y posiciones; no representan planetas nuevos.

Las galaxias del mapa tienen ilustraciones procedurales diferentes: Aurora es una espiral violeta inclinada; Hélice tiene dos brazos azules y un núcleo dorado con barra; Umbría se observa de canto, con un disco azulado y una franja oscura de polvo. Se tomaron como referencias visuales `galaxia1.png` y `galaxia2.png`; las imágenes no se cargan en el juego. El color se calcula por píxel una sola vez, se almacena en memoria y se dibuja con filtrado bilineal y mezcla de luz. Su movimiento lento continúa durante la selección.

| Destino | Planetas | Decoración |
| --- | --- | --- |
| Aurora | Siroco, Oquedad, Jovara y Boreal | Campo estelar y tormenta de arena; distribución original |
| Hélice | Siroco y Jovara | Fondo cálido y 42 asteroides decorativos |
| Umbría | Oquedad y Boreal | Fondo violeta y 64 asteroides decorativos |

Los asteroides se generan con una semilla fija por región y no tienen colisión. El agujero negro está presente en los tres destinos. Las mallas y texturas de los planetas se cargan una sola vez; al confirmar otro destino se actualizan las transformaciones y se reconstruyen sus colisiones antes del despegue. El acercamiento a un planeta conserva la misma escena de vuelo.

| Control | Acción |
| --- | --- |
| Clic sobre una galaxia, `1–3`, flechas o `Tab` / `Shift+Tab` | Elegir destino y abrir su descripción |
| Rueda del ratón o `+` / `-` | Acercar o alejar el mapa |
| `WASD` | Desplazar el mapa |
| `R` | Restaurar encuadre |
| `Enter` | Mostrar la descripción; si ya está abierta, iniciar el viaje |
| **Viajar** | Iniciar el viaje hacia el destino seleccionado |
| `Esc` o **Cambiar nave** | Regresar al hangar |

El viaje dura 2.8 segundos y muestra aceleración con estelas radiales, acercamiento a la nebulosa y un destello. Termina mezclándose con el primer cuadro real del destino. La física del vuelo comienza al finalizar la transición. `Esc` permite cancelar el viaje; la animación se pausa si la ventana pierde el foco.

La organización del mapa toma como referencia la navegación y la información contextual del [mapa galáctico descrito por Hello Games](https://www.nomanssky.com/atlas-rises-update/). Sus gráficos y distribución se dibujan mediante código propio.

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
| Clic izquierdo / derecho | Con Crucer espacial: colocar el portal amarillo / verde |
| `Esc` | Volver al selector de galaxias |

La nave comienza detenida y se desplaza mientras mantienes WASD. Puedes combinar teclas para moverte en diagonal sin aumentar la velocidad máxima; las teclas opuestas se cancelan. Al soltarlas, la nave desacelera suavemente hasta detenerse. `Shift` aumenta la velocidad mientras te desplazas y `Espacio` frena incluso si mantienes una tecla de movimiento.

El mouse dirige la nave sin mantener clic. La posición del cursor respecto al centro controla el giro: cuanto más lo alejas, más rápido gira; al volver al centro, mantiene el rumbo. Una pequeña zona neutra evita movimientos involuntarios. Al salir el cursor de la ventana, el giro se suaviza hasta detenerse. Para subir o bajar, apunta con el mouse y avanza con `W`.

La cámara está detrás y ligeramente por encima de la nave, mirando de la cola hacia la proa. Sigue los cambios de dirección con suavidad y la nave inclina las alas al girar. Las tres naves se dibujan en perspectiva y las estrellas se desplazan según la posición y velocidad reales, con estelas al avanzar. El HUD muestra posición y velocidad. El vuelo se pausa mientras inspeccionas la nave o la ventana pierde el foco.

**Portales de Crucer espacial:** apunta orientando la nave y usa clic izquierdo
para colocar el amarillo o derecho para el verde, a 32 unidades frente a la proa.
Coloca los dos en lugares separados y atraviesa cualquiera para salir por el otro,
conservando la rapidez y adaptando el rumbo a la salida. Otro clic recoloca solo
ese color. Cuando están enlazados, el interior muestra en tiempo real el mundo
desde la salida, con perspectiva que responde al movimiento de la cámara y el
borde animado de cada color. El HUD muestra si están enlazados. Se eliminan al
cambiar de nave, regresar a destinos o reaparecer; inspeccionar pausa y conserva
los portales.
Usan las imágenes amarilla y verde proporcionadas por el usuario, incluido el GIF
animado. [Controles, reglas y procedencia de las imágenes](assets/portals/README.md).

Al despegar en Aurora, Siroco aparece a la izquierda, Oquedad a la derecha y Jovara arriba,
entre los dos. Boreal, el nuevo gigante helado turquesa, aparece abajo a la izquierda.
Sus bandas blancas y remolinos azul oscuro giran lentamente sobre una esfera 3D,
con iluminación y textura UV inspirada en la referencia del usuario. Ocupan posiciones
fijas en el espacio: puedes acercarte, rodearlos y observar los túneles de Oquedad.
Sus modelos originales y escenas editables están en [assets/planets](assets/planets/README.md).

Los cuatro planetas tienen colisión con su superficie real. Los túneles de
Oquedad siguen abiertos y sus paredes también chocan con la nave. Los roces suaves
eliminan el movimiento hacia el terreno y permiten deslizarte o retroceder.
Un impacto fuerte destruye la nave y muestra fuego, fragmentos y humo; después de
dos segundos reapareces en el origen. Frena con `Espacio` antes de aproximarte.
[Ver la explosión y sus parámetros](assets/effects/README.md).

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

El agujero negro es un objeto tridimensional alcanzable, visible arriba a la
derecha al despegar. Puedes rodear su centro esférico y su disco naranja animado.
Al entrar a menos de 320 unidades empieza a atraer la nave, incluso sin pulsar
WASD. La fuerza aumenta al acercarte: usa WASD y `SHIFT` para intentar escapar.
Si cruzas su horizonte, la nave es absorbida y reaparece en el punto inicial.
El HUD muestra la distancia y avisa de la atracción. Puedes ver
[capturas y parámetros](assets/deep-space/README.md).

## Controles de inspección


| Tecla | Acción |
| --- | --- |
| `1`, `2`, `3` | Elegir Guardgalaxy, Galactwar o Crucer espacial |
| Flechas o `W`, `A`, `S`, `D` | Orbitar la cámara |
| `Q`, `E` | Alejar o acercar |
| `M` | Cambiar entre sólido, malla y holograma |
| `R` | Reiniciar la vista de la nave seleccionada |
| `P` | Guardar `render.png` |
| `Esc` o botón de volver | Regresar al vuelo |

## Exportar vistas sin abrir una ventana

```bash
cargo run --release -- --render-home output/inicio.png
cargo run --release -- --render-home output/controles.png --home-section controles
cargo run --release -- --render-home output/instrucciones.png --home-section instrucciones
cargo run --release -- --render-menu output/hangar.png --time 4 --ship 1
cargo run --release -- --render-galaxies output/atlas.png --galaxy 2
cargo run --release -- --render-galaxies output/detalle.png --galaxy 2 --details --time 8
cargo run --release -- --render-travel output/viaje.png --galaxy 2 --time 1.4
cargo run --release -- --render-inspector output/crucer-espacial.png --ship 3
cargo run --release -- --render-flight output/vuelo.png --ship 1
cargo run --release -- --render-flight output/portales.png --ship 3 --portal-demo --time 0.35
cargo run --release -- --render-flight output/portal-boreal.png --ship 3 --portal-demo --target boreal --time 0.35
cargo run --release -- --render-flight output/umbria.png --galaxy 3
cargo run --release -- --render-flight output/giro.png --ship 2 --time 2 --mouse 0.65 -0.35
cargo run --release -- --render-planet output/boreal.png --target boreal
cargo run --release -- --render-flight output/impacto.png --near-planet boreal --time 1.05
```

`--time` indica el instante de la animación en segundos; `--ship` elige una nave de 1 a 3. Para vuelo, `--time` simula hasta 600 segundos manteniendo `W` y `--mouse x y` mantiene una dirección normalizada entre -1 y 1 (centro: `0 0`; derecha y arriba: `0.65 -0.35`). Estas exportaciones usan la misma física y el mismo renderizador que la aplicación interactiva.

`--render-planet` exporta un planeta centrado sobre fondo oscuro, usando el mismo
renderizador del vuelo. Admite `--target boreal`, `jovara`, `siroco` u `oquedad`;
por defecto muestra Boreal. `--time` permite revisar su rotación sin mover la nave.

`--galaxy 1`, `2` o `3` elige Aurora, Hélice o Umbría. Aurora es el valor predeterminado. Las opciones de acercamiento y vista de planeta requieren que ese planeta exista en el destino elegido.

## Rendimiento con Rayon

Los triángulos se agrupan en franjas de 32 filas. Cada tarea modifica exclusivamente sus píxeles y profundidades, manteniendo el orden de dibujo y sin mutex por píxel. El polvo también conserva el orden de transparencia dentro de cada franja. Los lotes pequeños usan una ruta secuencial para evitar el costo de distribuir tareas. El agujero negro calcula filas independientes en paralelo.

Para medir el renderizado de una escena fija después de un cuadro de calentamiento:

```powershell
cargo run --release -- --render-flight output/medicion.png --benchmark-frames 180
cargo run --release -- --render-flight output/agujero.png --near-black-hole --benchmark-frames 60
```

El resultado indica milisegundos por cuadro y FPS teóricos de renderizado: excluye la carga inicial, la escritura PNG, la ventana y la actualización de la física. No equivale a una garantía de FPS durante todo el juego. Para comparar con un único hilo, ejecuta el mismo comando con `RAYON_NUM_THREADS=1`; al quitar esa variable, Rayon utiliza su configuración automática. La aplicación interactiva conserva el límite de 60 FPS.

La integración utiliza [iteradores y fragmentos de memoria independientes de Rayon](https://docs.rs/rayon/latest/rayon/slice/trait.ParallelSliceMut.html). Las pruebas comparan la imagen y profundidad de las rutas secuencial y paralela, además de la transparencia ordenada del polvo.

`--near-planet nombre` inicia la exportación frente al planeta indicado para
revisar el acercamiento, la colisión y la explosión usando la física del juego.
Admite los cuatro nombres de planeta y es una alternativa a `--near-black-hole`.

## Organización

- `src/fleet.rs`: modelos, nombres y acabados de las naves.
- `src/home.rs`: título del juego, menú principal, controles e instrucciones.
- `src/galaxy.rs`: catálogo de destinos, distribuciones y transformación del mapa.
- `src/galaxy_art.rs`: ilustraciones procedurales de galaxias con núcleos, brazos y polvo.
- `src/menus.rs`: hangar lateral, vista principal y atlas de galaxias.
- `src/nebula.rs`: fondo procedural basado en `referencia.rs` y variación de color del mapa.
- `src/noise.rs`: ruido reproducible y funciones de interpolación de la nebulosa.
- `src/travel.rs`: transición de viaje y mezcla con el cuadro de llegada.
- `src/flight.rs`: posición, velocidad, impulso y controles de vuelo.
- `src/portal.rs`: colocación, enlace y cruce continuo de los dos portales.
- `src/portal_art.rs`: discos en perspectiva y animación de las imágenes de portales.
- `src/collision.rs`: colisión continua con las mallas de los planetas y sus túneles.
- `src/explosion.rs`: fuego, fragmentos y humo del impacto en coordenadas del mundo.
- `src/space.rs`: carga, posición y rotación de los planetas.
- `src/dust.rs`: nube animada de arena de Siroco y partículas con transparencia.
- `src/texture.rs`: carga y muestreo bilineal de mapas de color PNG.
- `src/renderer.rs`: rasterizado compartido, vistas del hangar y perspectiva de vuelo.
- `src/raster_batch.rs`: rasterización de lotes por franjas con Rayon.
- `src/ui.rs`: composición, tipografía, interacción con las tarjetas y mapeo del ratón.
- `src/main.rs`: navegación, entradas y reloj de animación.
- `assets/ui/`: atlas de glifos integrado en el ejecutable; no requiere instalar fuentes.

Las pruebas de `cargo test` comprueban los giros completos del hangar, pausa/reanudación, selección con ventana redimensionada y animación de cada nave en la vista principal sin modificar la columna lateral. También verifican la selección del mapa al moverlo, girarlo o ampliarlo; la restauración de destinos con sus colisiones; la decoración reproducible; y la igualdad del renderizado entre uno y varios hilos. El vuelo tiene pruebas de dirección sostenida con mouse, movimiento sin saltos, WASD sin avance automático ni ventaja diagonal, desaceleración, turbo/freno, respuesta a distintas tasas de cuadros, cámara trasera y desplazamiento del campo estelar. Los planetas tienen comprobaciones de visibilidad, profundidad independiente del orden de dibujo, recorte cerca de la cámara y movimiento de la nube sin desplazar la nave.

## Integración y procedencia

La integración, el port a Rust y el menú se realizaron con asistencia de Codex.

- **Guardgalaxy**: modelo `Benatar.obj`, del proyecto [hmndzzl/Graficas_Lab4](https://github.com/hmndzzl/Graficas_Lab4). Su MTL original no define colores difusos, por lo que el hangar aplica un acabado cobrizo inspirado en el visualizador original.
- **Galactwar**: modelo `nave-lab3-PC.obj`, del proyecto [Pxdro-410/lab4-Graficas-PC](https://github.com/Pxdro-410/lab4-Graficas-PC). El ZIP no incluía el MTL referenciado, así que `Galactwar.mtl` completa los cuatro materiales que aparecen en el OBJ.
- **Crucer espacial**: modelo original `Crucer_espacial.obj` y materiales `Crucer_espacial.mtl`, construidos a partir de la imagen de referencia de la nave de Rick Sánchez de *Rick y Morty* aportada por el usuario. Reemplaza la tercera nave de la flota. Incluye casco gris de platillo, franjas amarillas, dos motores acanalados, faros, asientos marrones y cúpula transparente. [Detalles y regeneración](assets/ships/README.md).

El renderizador C++ de [J05U3oAr/Lab4-graficas](https://github.com/J05U3oAr/Lab4-graficas) se portó a módulos Rust: carga de OBJ/MTL, validación y triangulación de caras, rasterizado baricéntrico con profundidad, iluminación difusa y exportación PNG. La ventana y los controles usan `minifb`. No se conservaron archivos `.cpp`, `.h` ni dependencias de SDL.

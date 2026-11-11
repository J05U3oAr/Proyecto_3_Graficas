# Agujero negro con atracción

El agujero negro está lejos del sistema de cada galaxia y puedes alcanzarlo
con la nave. En Hélice ocupa (4400, 480, −4600), en Aurora (6100, 650, −5200)
y en Umbría (−6200, 700, −6000). Sus posiciones se definen en `src/galaxy.rs`.
El centro es una esfera y el disco es un anillo inclinado en el mundo: se
renderizan mediante intersecciones de rayos, con profundidad, desde cualquier
ángulo. Los filamentos naranjas del disco se animan con el reloj del vuelo.

## Comportamiento

- Radio del horizonte: 44 unidades. Cruzarlo absorbe la nave.
- Radio exterior del disco: 195 unidades.
- Zona de atracción de la nave: 320 unidades, con entrada suave y fuerza creciente.
- Las rocas sienten atracción a cualquier distancia; la fuerza de caída libre
  usa `720000 / (distancia² + horizonte²)`, limitada a 260 unidades/s².
- La corriente de 24 rocas de cada galaxia tiene velocidades radiales y
  tangenciales; también reciben gravedad el resto de asteroides y las 96 rocas
  de los anillos de Umbriel. Estas últimas conservan el movimiento orbital de su
  planeta y acumulan por separado su deriva hacia el agujero negro.
- Una roca se captura cuando su superficie cruza el horizonte. Deja de dibujarse
  y de colisionar; no reaparece al avanzar el tiempo ni se sustituye por otra.
- La atracción también actúa cuando sueltas WASD o usas el freno.
- Puedes intentar escapar de la zona exterior orientándote hacia fuera y usando
  WASD + SHIFT. Cerca del horizonte la atracción domina.
- Tras ser absorbida, la nave reaparece en el origen después de 1.4 segundos.
  Se eliminan los impulsos de caída y aparece un aviso de recuperación.
- El HUD muestra distancia, advertencia de atracción y estado de captura.
- La física y la animación se detienen al inspeccionar o perder el foco.

Es una mecánica de juego con fuerzas acotadas y un anillo luminoso estilizado;
no intenta simular relatividad general. El campo y la geometría comparten los
parámetros de `src/black_hole.rs`, para que el volumen visible y el horizonte de
captura coincidan. La colisión comprueba el trayecto entre pasos, incluso con
velocidades altas.

[Observación inicial](../../docs/assets/caida-0.png) y
[observación tras 40 segundos](../../docs/assets/caida-40.png).

## Exportar vistas

```powershell
cargo run --release -- --render-landmark output/agujero.png
cargo run --release -- --render-flight output/atraccion.png --near-black-hole --time 1
cargo run --release -- --render-flight output/captura.png --near-black-hole --time 4.5
cargo run --release -- --render-flight output/materia.png --observe-black-hole --time 20
```

`--near-black-hole` inicia únicamente la exportación a 250 unidades del centro.
No teletransporta la nave en el juego interactivo. `--time` mantiene W durante
la simulación, con las mismas fuerzas y captura de la aplicación.

`--observe-black-hole` coloca solo la cámara de exportación a unas 1200 unidades
del centro y conserva su posición mientras avanza el tiempo. El HUD cuenta
capturas reales. El control de tiempo de la galería de GitHub Pages compara
cinco capturas de esta misma simulación, de 0 a 40 segundos.

La integración usa velocidad Verlet a 120 Hz y acumuladores de doble precisión.
Las colisiones y el dibujo consultan el mismo estado, interpolado entre pasos.
Una caché limitada guarda puntos de control cada dos segundos y estados recientes;
permite repetir o retroceder una vista sin depender de los FPS ni del orden de
consulta. El reloj solo avanza con el vuelo: inspección, falta de foco y portales
de vista previa no consumen tiempo de simulación.

`textures/accretion.png` conserva la ilustración generada con ImageGen a partir
de la referencia del usuario como referencia artística. El juego ahora dibuja
geometría procedural; ya no utiliza esa imagen como cartel orientado a la cámara.

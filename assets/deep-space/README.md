# Agujero negro decorativo

El único escenario del espacio profundo es el agujero negro con disco naranja.
Está en la posición (12420, 2610, −20560), lejos de los planetas, y se ve arriba
a la derecha al despegar. Su ancho es de 11200 unidades del mundo.

Es una imagen emisiva orientada a la cámara y anclada al mundo. Su proyección
cambia al girar o desplazarse; la profundidad permite que la nave y los planetas
pasen por delante. Los bordes se desvanecen sobre el cielo y la sombra central
oculta el fondo. Es decoración estática, sin gravedad ni colisiones.

`textures/accretion.png` es una ilustración generada con ImageGen para este
proyecto a partir de la referencia del usuario: disco naranja inclinado, arco
luminoso, centro oscuro y un pequeño cuerpo oscuro. El efecto de lente está
representado en la imagen; no es una simulación física.

![Agujero negro al despegar](previews/en-vuelo.png)

Para exportar una vista cercana con el mismo renderizador del juego:

```powershell
cargo run --release -- --render-landmark output/agujero.png
```

La captura cercana está en `previews/accretion.png`.

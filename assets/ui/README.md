# Recursos de interfaz

`type-atlas.png` contiene glifos rasterizados de Bahnschrift, presente en Windows, en tamaños 14, 16, 18, 22, 28 y 52. `type-atlas.txt` almacena tamaño, código Unicode, región del atlas, posición respecto a la línea base y avance de cada glifo.

El archivo de fuente no se distribuye. La aplicación dibuja los glifos del atlas con mezcla alfa en Rust; no necesita fuentes instaladas para ejecutar el menú. Fondos, estrellas, tarjetas, órbitas y controles se dibujan mediante código del framebuffer.

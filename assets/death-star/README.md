# Estrella de la Muerte

Modelo original generado en este proyecto: esfera acorazada, trinchera ecuatorial
con relieve y antena de enfoque cóncava con anillos y nervaduras radiales.
46 612 triángulos y atlas de paneles de 2048 × 1024 píxeles.

Referencia visual: [Star Wars Databank — Death Star](https://www.starwars.com/databank/death-star).
Star Wars y la Estrella de la Muerte pertenecen a Lucasfilm. No se descargaron
modelos ni texturas de terceros.

Para regenerar los recursos (Python y Pillow):

```powershell
python scripts/create_death_star.py
```

En el juego, solo TIE Silencer puede llamar a la estación con **R**, apuntando
la proa a un planeta visible. El HUD muestra el objetivo antes de llamar.
No admite soles, naves destruidas, objetivos ocultos ni llamadas simultáneas.
La secuencia dura 11 segundos y la recarga 22 segundos desde la llamada.
El impacto elimina el planeta, su atmósfera, gravedad, navegación y colisiones;
en Umbriel también elimina los anillos. Se conserva la destrucción al reaparecer
y cambiar de nave, hasta cargar otra galaxia. Volver a una galaxia inicia una
visita nueva. Inspeccionar o perder el foco pausa la secuencia; Esc la abandona
y conserva la destrucción si ya ocurrió el impacto.

Exportación reproducible por la misma ruta de simulación del juego:

```powershell
cargo run --release -- --render-flight output/death-star.png --ship 2 --galaxy 1 --near-planet micelia --death-star --time 5.4
```

Tiempos: llegada 0–1.8 s, carga hasta 4.8 s, impacto a los 5.5 s y retirada
hasta los 11 s. La estación se escala a los mundos del juego para permitir
encuadrarla junto al objetivo; no representa distancias astronómicas reales.

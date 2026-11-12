"use strict";

// The three full articles remain readable if JavaScript is disabled.
const atlas = document.querySelector(".atlas");
const tablist = document.querySelector(".planet-tabs");
const tabs = Array.from(tablist.querySelectorAll("button"));
tablist.setAttribute("role", "tablist");
atlas.classList.add("enhanced");

function selectWorld(index, focus = false) {
  tabs.forEach((tab, i) => {
    const selected = i === index;
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
    document.getElementById(tab.dataset.world).hidden = !selected;
  });
  if (focus) tabs[index].focus();
}

tabs.forEach((tab, index) => {
  const panel = document.getElementById(tab.dataset.world);
  tab.setAttribute("role", "tab");
  tab.setAttribute("aria-controls", panel.id);
  panel.setAttribute("role", "tabpanel");
  panel.setAttribute("aria-labelledby", tab.id);
  panel.tabIndex = 0;
  tab.addEventListener("click", () => selectWorld(index));
  tab.addEventListener("keydown", (event) => {
    let next;
    if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    else return;
    event.preventDefault();
    selectWorld(next, true);
  });
});
selectWorld(0);

const gravityTime = document.getElementById("gravity-time");
const gravityImage = document.getElementById("gravity-image");
const gravityOutput = document.getElementById("gravity-output");
gravityTime.disabled = false;
gravityTime.addEventListener("input", () => {
  const seconds = Number(gravityTime.value);
  gravityImage.src = `assets/caida-${seconds}.png`;
  gravityImage.alt = `Observación del agujero negro y los asteroides tras ${seconds} segundos de simulación`;
  gravityOutput.value = `${seconds} s`;
  gravityTime.setAttribute("aria-valuetext", `${seconds} segundos de simulación`);
});

const jumpFrames = [
  [0.06, "Onda de salida", "Benatar inicia el salto y deja una onda roja en el punto de salida"],
  [0.25, "Salto", "Benatar avanza con dos estelas rojas sobre su trayectoria real"],
  [0.45, "Estela", "La estela roja se extiende detrás de Benatar mientras las estrellas conservan su forma"],
  [3.08, "Listo otra vez", "El salto de tres segundos termina y Benatar puede saltar otra vez inmediatamente, sin recarga"],
  [3.95, "Desvanecida", "La estela se ha desvanecido y Benatar sigue lista para otro salto, sin recarga"],
];
const jumpTime = document.getElementById("jump-time");
const jumpImage = document.getElementById("jump-image");
const jumpOutput = document.getElementById("jump-output");
jumpTime.disabled = false;
jumpTime.addEventListener("input", () => {
  const index = Number(jumpTime.value);
  const [seconds, stage, description] = jumpFrames[index];
  jumpImage.src = `assets/benatar-estela-${index}.png`;
  jumpImage.alt = description;
  jumpOutput.value = `${seconds} s · ${stage}`;
  jumpTime.setAttribute("aria-valuetext", `${seconds} segundos, ${stage}`);
});

"use strict";

const frames = [
  [0.8, "Llegada", "La Estrella de la Muerte llega a la órbita de Micelia"],
  [2.4, "Enlace", "Estrella de la Muerte desplegada, con la antena orientada hacia Micelia"],
  [4.5, "Carga", "Ocho haces verdes convergen desde la antena de la Estrella de la Muerte"],
  [5.4, "Disparo", "El superláser verde recorre el espacio hacia Micelia"],
  [5.55, "Impacto", "El impacto del superláser ilumina el lugar donde estaba Micelia"],
  [6.2, "Explosión", "Fragmentos incandescentes y una onda expansiva reemplazan al planeta"],
  [8.0, "Restos", "Los fragmentos de Micelia se dispersan por el espacio"],
  [10.5, "Retirada", "La estación se retira, dejando vacío el lugar del planeta"],
];
const slider = document.getElementById("strike-time");
const picture = document.getElementById("strike-image");
const output = document.getElementById("strike-output");
slider.disabled = false;
slider.addEventListener("input", () => {
  const index = Number(slider.value);
  const [time, stage, description] = frames[index];
  picture.src = `assets/death-star-${index}.png`;
  picture.alt = description;
  output.value = `${time} s · ${stage}`;
  slider.setAttribute("aria-valuetext", `${time} segundos, ${stage}`);
});

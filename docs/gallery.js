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

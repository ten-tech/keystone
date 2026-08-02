"use strict";

/*
 * Coque de `ks-ui` — le poste de pilotage.
 *
 * CE FICHIER NE FABRIQUE AUCUNE DONNÉE. Il demande au processus Rust l'état
 * réel de la machine et le met en scène. Toute valeur affichée vient de cet
 * état ; quand elle manque, l'écran écrit « non relevé » plutôt que d'inventer
 * un repli.
 *
 * Il n'existe volontairement aucun jeu de données de démonstration ici : une
 * maquette empaquetée en exécutable devient un produit qui affiche des chiffres
 * faux, et c'est précisément ce que ce dépôt combat.
 *
 * La coque n'a aucune permission Tauri. `window.__TAURI_INTERNALS__` est le
 * pont injecté par le runtime ; c'est l'unique point de contact entre cette
 * page et le processus Rust, et il n'expose qu'une commande.
 */

const COMMANDE = "collect_state";
const REPLI = "non relevé";

const corps = document.body;
const racine = document.documentElement;

const FORMAT_DATE = new Intl.DateTimeFormat("fr-FR", {
  dateStyle: "long",
  timeStyle: "short",
});

/** L'état renvoyé par le pont. Nul tant que la collecte n'a pas abouti. */
let etat = null;
let minuterie = null;

/* ── Le compteur d'attente ─────────────────────────────────────────────────
 * Il affiche le temps RÉELLEMENT écoulé, jamais une progression estimée : tant
 * que les collecteurs n'annoncent pas leur avancement, aucun pourcentage ne
 * serait honnête. */

function demarrerCompteur() {
  arreterCompteur();
  const debut = Date.now();
  const cible = document.getElementById("elapsed");
  const ecrire = () => {
    const secondes = Math.floor((Date.now() - debut) / 1000);
    cible.textContent = `Temps écoulé : ${secondes} s`;
  };
  ecrire();
  minuterie = window.setInterval(ecrire, 1000);
}

function arreterCompteur() {
  if (minuterie !== null) {
    window.clearInterval(minuterie);
    minuterie = null;
  }
}

/* ── Résolution des valeurs ────────────────────────────────────────────────
 * `data-mesure` porte soit un CHEMIN D'ITEM, soit une clé calculée par le
 * processus Rust en comptant de vrais items. Aucune troisième source. */

function calculees() {
  if (etat === null) {
    return {};
  }
  return {
    "count.items": String(etat.itemCount),
    "machine.name": etat.machine,
    "collected.at": horodatageLisible(etat.collectedAt),
    "security.total": String(etat.security.total),
    "security.readable": String(etat.security.readable),
    "security.unreadable": String(etat.security.unreadable),
    "volumes.count": String(etat.volumes.length),
    "software.unattributed.count": String(etat.software.unattributedPaths.length),
  };
}

/** La valeur à afficher pour une clé, ou `null` si rien ne la porte. */
function valeurDe(clef) {
  const calc = calculees();
  if (Object.prototype.hasOwnProperty.call(calc, clef)) {
    return calc[clef];
  }
  const releve = releveDe(clef);
  return releve === null ? null : releve.value;
}

/** Le relevé d'un chemin d'item, ou `null` s'il n'a pas été collecté. */
function releveDe(chemin) {
  if (etat === null) {
    return null;
  }
  return Object.prototype.hasOwnProperty.call(etat.readings, chemin)
    ? etat.readings[chemin]
    : null;
}

/** Remplit tous les emplacements `data-mesure` de la page. */
function remplirMesures() {
  for (const noeud of document.querySelectorAll("[data-mesure]")) {
    const valeur = valeurDe(noeud.dataset.mesure);
    noeud.textContent = valeur === null ? REPLI : valeur;
    noeud.classList.toggle("v-absent", valeur === null);
  }
}

/** Remplit les emplacements `data-champ`, qui pointent une prose du pont. */
function remplirChamps() {
  for (const noeud of document.querySelectorAll("[data-champ]")) {
    const [bloc, champ] = noeud.dataset.champ.split(".");
    const source = bloc === "drift" ? etat.drift : etat[bloc];
    const texte = source && typeof source[champ] === "string" ? source[champ] : "";
    noeud.textContent = texte;
  }
}

/* L'horodatage arrive en RFC 3339 — la forme que produit le noyau, et celle
 * qu'attend un script. L'écran, lui, lit une date française. */
function horodatageLisible(rfc3339) {
  const date = new Date(rfc3339);
  return Number.isNaN(date.getTime()) ? rfc3339 : FORMAT_DATE.format(date);
}

/* ── Petits constructeurs de balisage ──────────────────────────────────────
 * `textContent` partout, jamais `innerHTML` avec une valeur : les noms
 * d'applications viennent du registre, donc d'une source non fiable. */

function el(balise, classe, texte) {
  const noeud = document.createElement(balise);
  if (classe) {
    noeud.className = classe;
  }
  if (texte !== undefined) {
    noeud.textContent = texte;
  }
  return noeud;
}

function cellule(texte, classe) {
  return el("td", classe, texte);
}

/** Une ligne « chemin · valeur · finalité » pour un relevé, ou son absence. */
function ligneReleve(chemin, libelle) {
  const releve = releveDe(chemin);
  const tr = document.createElement("tr");
  const th = el("th", null, libelle);
  th.setAttribute("scope", "row");
  tr.append(th);

  if (releve === null) {
    tr.append(cellule(REPLI, "valeur v-absent"));
  } else if (!releve.readable) {
    tr.append(cellule(`illisible — ${releve.reason}`, "valeur v-illisible"));
  } else {
    tr.append(cellule(releve.value, "valeur"));
  }
  tr.append(cellule(chemin, "chemin"));
  return tr;
}

/* ── Les écrans ────────────────────────────────────────────────────────── */

function rendreDomaines() {
  const cible = document.getElementById("domaines");
  cible.replaceChildren();
  for (const domaine of etat.domains) {
    const tr = document.createElement("tr");
    const th = el("th", null, domaine.label);
    th.setAttribute("scope", "row");
    tr.append(th, cellule(String(domaine.count), "num"));
    cible.append(tr);
  }
}

/* Les quatre verdicts du modèle. « Sans objet » n'est PAS un zéro : un zéro
 * dirait qu'une confrontation a eu lieu et n'a rien trouvé. C'est très
 * exactement le défaut que `Verdict::Incomparable` existe pour empêcher, et il
 * se rejouerait ici si la page affichait un décompte à la place. */
const VERDICTS = [
  ["Non contraint", "Aucun état désiré : Keystone ne contraint que ce qui est écrit.", "unconstrained"],
  ["Conforme", "Le constaté correspond au désiré.", "compliant"],
  ["Écart", "Le constaté diffère du désiré.", "deviation"],
  ["Incomparable", "La lecture a échoué : ni conforme, ni en écart.", "incomparable"],
];

function rendreVerdicts() {
  const cible = document.getElementById("verdicts");
  cible.replaceChildren();
  const comparable = etat.drift.state === "computed";
  for (const [nom, sens, clef] of VERDICTS) {
    const tr = document.createElement("tr");
    const th = el("th", null, nom);
    th.setAttribute("scope", "row");
    const disponible = comparable || clef === "unconstrained";
    tr.append(
      th,
      cellule(sens, "but"),
      cellule(disponible ? String(etat.drift[clef]) : "sans objet", disponible ? "num" : "v-absent")
    );
    cible.append(tr);
  }
}

/* La dérive n'a de décomptes que si un état désiré existe. Tant qu'il n'y en a
 * pas, l'explication reste visible et AUCUN décompte n'est publié — pas même un
 * zéro. Le jour où une référence sera chargée, ce sont les décomptes qui
 * parleront et l'explication qui s'effacera. */
function rendreDerive() {
  const comparable = etat.drift.state === "computed";
  document.getElementById("drift-indispo").hidden = comparable;

  const tuile = document.getElementById("tile-drift");
  tuile.classList.toggle("tile-none", !comparable);
  if (!comparable) {
    return;
  }
  tuile.querySelector(".tile-none-v").textContent = `${etat.drift.deviation} en écart`;
  tuile.querySelector(".tile-c").textContent =
    `${etat.drift.compliant} conformes, ${etat.drift.incomparable} incomparables, ` +
    `${etat.drift.unconstrained} non contraints.`;
  tuile.querySelector(".badge span").textContent = "Comparé";
}

function rendreSecurite() {
  const bloc = document.getElementById("illisibles");
  const liste = document.getElementById("illisibles-liste");
  liste.replaceChildren();
  bloc.hidden = etat.security.unreadablePaths.length === 0;
  for (const chemin of etat.security.unreadablePaths) {
    const releve = releveDe(chemin);
    const ligne = el("p", "detail", `${chemin} — ${releve ? releve.reason : REPLI}`);
    liste.append(ligne);
  }

  const cible = document.getElementById("familles");
  cible.replaceChildren();
  for (const famille of etat.security.families) {
    const carte = el("section", "card");
    const entete = el("div", "card-h");
    entete.append(el("h2", null, famille.label), el("span", "spacer"));
    entete.append(el("span", "kbd", `security.${famille.key}`));

    const corpsCarte = el("div", "card-b");
    const cadre = el("div", "cadre-table");
    cadre.setAttribute("role", "group");
    cadre.setAttribute("aria-label", `Items de la famille ${famille.label}`);
    cadre.setAttribute("tabindex", "0");

    const table = document.createElement("table");
    const thead = document.createElement("thead");
    const trh = document.createElement("tr");
    for (const titre of ["Item", "Valeur constatée", "À quoi ça sert"]) {
      const th = el("th", null, titre);
      th.setAttribute("scope", "col");
      trh.append(th);
    }
    thead.append(trh);
    const tbody = document.createElement("tbody");
    for (const chemin of famille.paths) {
      const releve = releveDe(chemin);
      const tr = document.createElement("tr");
      const th = el("th", null, chemin);
      th.setAttribute("scope", "row");
      th.className = "chemin";
      if (releve === null) {
        tr.append(th, cellule(REPLI, "valeur v-absent"), cellule("", "but"));
      } else if (releve.readable) {
        tr.append(th, cellule(releve.value, "valeur"), cellule(releve.purpose, "but"));
      } else {
        tr.append(
          th,
          cellule(`illisible — ${releve.reason}`, "valeur v-illisible"),
          cellule(releve.purpose, "but")
        );
      }
      tbody.append(tr);
    }
    table.append(thead, tbody);
    cadre.append(table);
    corpsCarte.append(cadre);
    carte.append(entete, corpsCarte);
    cible.append(carte);
  }
}

function rendreMachine() {
  const cible = document.getElementById("machine-lignes");
  cible.replaceChildren();
  for (const ligne of etat.machineRows) {
    cible.append(ligneReleve(ligne.path, ligne.label));
  }
}

function rendreLogiciels() {
  const agregats = document.getElementById("logiciels-agregats");
  agregats.replaceChildren();
  for (const chemin of etat.software.aggregatePaths) {
    const releve = releveDe(chemin);
    const tr = document.createElement("tr");
    const th = el("th", null, chemin);
    th.setAttribute("scope", "row");
    th.className = "chemin";
    const affichee =
      releve.list !== null && releve.list !== undefined
        ? releve.list.join(", ")
        : releve.value;
    tr.append(th, cellule(affichee, "valeur"), cellule(releve.purpose, "but"));
    agregats.append(tr);
  }

  const liste = document.getElementById("logiciels-liste");
  liste.replaceChildren();
  for (const chemin of etat.software.unattributedPaths) {
    const releve = releveDe(chemin);
    const tr = document.createElement("tr");
    const th = el("th", null, entreCrochets(chemin));
    th.setAttribute("scope", "row");
    th.className = "chemin";
    tr.append(th, cellule(releve.value, "valeur"));
    liste.append(tr);
  }
}

/** Ce que porte un chemin entre crochets, ou le chemin s'il n'en porte pas. */
function entreCrochets(chemin) {
  const debut = chemin.indexOf("[");
  const fin = chemin.indexOf("]", debut);
  return debut === -1 || fin === -1 ? chemin : chemin.slice(debut + 1, fin);
}

/* La jauge encode la MÊME mesure que le chiffre à côté d'elle : sa longueur
 * vient de `number`, la valeur entière relevée, jamais d'une relecture de la
 * chaîne affichée. Un item sans entier n'a pas de jauge — plutôt aucune barre
 * qu'une barre arbitraire. */
function jauge(nom, releve) {
  const bloc = el("div", "jauge");
  const entete = el("div", "jauge-h");
  entete.append(el("b", null, nom));
  entete.append(el("span", "v", releve === null ? REPLI : `${releve.value} %`));
  bloc.append(entete);

  if (releve !== null && typeof releve.number === "number") {
    const barre = el("div", "meter");
    const remplissage = document.createElement("i");
    const borne = Math.max(0, Math.min(100, releve.number));
    remplissage.style.width = `${borne}%`;
    barre.append(remplissage);
    barre.setAttribute("role", "img");
    barre.setAttribute("aria-label", `${nom} : ${releve.value} pour cent occupés`);
    bloc.append(barre);
  }
  return bloc;
}

/** Le chemin du taux d'occupation d'un volume, parmi ceux qu'il porte. */
function cheminOccupation(volume) {
  return volume.paths.find((p) => p.endsWith("used_percent")) || volume.paths[0];
}

function rendreEspace() {
  const tuile = document.getElementById("tile-volumes");
  const jauges = document.getElementById("volumes-jauges");
  const lignes = document.getElementById("volumes-lignes");
  tuile.replaceChildren();
  jauges.replaceChildren();
  lignes.replaceChildren();

  if (etat.volumes.length === 0) {
    tuile.append(el("p", "tile-c v-absent", "Aucun volume relevé."));
    jauges.append(el("p", "indispo-2", "Aucun volume relevé."));
    return;
  }

  for (const volume of etat.volumes) {
    const chemin = cheminOccupation(volume);
    const releve = releveDe(chemin);
    tuile.append(jauge(volume.name, releve));
    jauges.append(jauge(volume.name, releve));

    const tr = document.createElement("tr");
    const th = el("th", null, volume.name);
    th.setAttribute("scope", "row");
    tr.append(
      th,
      cellule(releve === null ? REPLI : `${releve.value} %`, releve === null ? "v-absent" : "num"),
      cellule(chemin, "chemin")
    );
    lignes.append(tr);
  }
}

function rendreDistros() {
  const cible = document.getElementById("distros");
  cible.replaceChildren();

  if (etat.distros.length === 0) {
    const carte = el("section", "card");
    const corpsCarte = el("div", "card-b");
    corpsCarte.append(el("p", "indispo-2", "Aucune distribution WSL relevée."));
    carte.append(corpsCarte);
    cible.append(carte);
    return;
  }

  for (const distro of etat.distros) {
    const carte = el("section", "card");
    const entete = el("div", "card-h");
    entete.append(el("h2", null, distro.name), el("span", "spacer"));
    entete.append(el("span", "kbd", `virtualization.wsl[${distro.name}]`));

    const corpsCarte = el("div", "card-b");
    const cadre = el("div", "cadre-table");
    cadre.setAttribute("role", "group");
    cadre.setAttribute("aria-label", `Relevés de la distribution ${distro.name}`);
    cadre.setAttribute("tabindex", "0");

    const table = document.createElement("table");
    const thead = document.createElement("thead");
    const trh = document.createElement("tr");
    for (const titre of ["Item", "Valeur constatée", "À quoi ça sert"]) {
      const th = el("th", null, titre);
      th.setAttribute("scope", "col");
      trh.append(th);
    }
    thead.append(trh);
    const tbody = document.createElement("tbody");
    for (const chemin of distro.paths) {
      const releve = releveDe(chemin);
      const tr = document.createElement("tr");
      const th = el("th", null, chemin.slice(chemin.indexOf("]") + 2));
      th.setAttribute("scope", "row");
      th.className = "chemin";
      tr.append(th, cellule(releve.value, "valeur"), cellule(releve.purpose, "but"));
      tbody.append(tr);
    }
    table.append(thead, tbody);
    cadre.append(table);
    corpsCarte.append(cadre);
    carte.append(entete, corpsCarte);
    cible.append(carte);
  }
}

function rendre() {
  remplirMesures();
  remplirChamps();
  rendreDomaines();
  rendreVerdicts();
  rendreDerive();
  rendreSecurite();
  rendreMachine();
  rendreLogiciels();
  rendreEspace();
  rendreDistros();

  /* `srcdoc` plutôt qu'une injection dans le document : le rapport reste un
   * document séparé, dans un cadre `sandbox` sans aucune permission. Même si
   * l'échappement de `ks_cli::rapport` cédait, rien ne s'exécuterait là. */
  document.getElementById("report").srcdoc = etat.html;
}

/* ── Navigation ────────────────────────────────────────────────────────── */

function allerA(vue) {
  for (const bouton of document.querySelectorAll(".nav")) {
    if (bouton.dataset.view === vue) {
      bouton.setAttribute("aria-current", "page");
    } else {
      bouton.removeAttribute("aria-current");
    }
  }
  for (const section of document.querySelectorAll(".view")) {
    section.classList.toggle("on", section.id === `view-${vue}`);
  }
  document.querySelector(".main").scrollTop = 0;
}

for (const bouton of document.querySelectorAll(".nav")) {
  bouton.addEventListener("click", () => allerA(bouton.dataset.view));
}

/* ── Rail repliable ────────────────────────────────────────────────────────
 * Un <button>, et pas un div : il doit être atteignable au clavier, annoncer
 * son état par aria-expanded et désigner ce qu'il commande par aria-controls.
 * Ctrl+B parce que c'est la convention là où ce geste existe déjà, et qu'on ne
 * réinvente pas un raccourci connu (loi de Jakob).
 *
 * L'état survit au rechargement. localStorage est un stockage LOCAL : aucune
 * requête, donc aucune entorse au principe P5. */

function appliquerRail(replie) {
  racine.dataset.rail = replie ? "replie" : "deplie";
  const bouton = document.getElementById("fold");
  bouton.setAttribute("aria-expanded", String(!replie));
  bouton.setAttribute(
    "aria-label",
    replie ? "Déplier le rail de navigation" : "Replier le rail de navigation"
  );
  bouton.title = `${replie ? "Déplier le rail" : "Replier le rail"} (Ctrl+B)`;
  try {
    localStorage.setItem("ks-rail", replie ? "replie" : "deplie");
  } catch (ignore) {
    /* Un stockage indisponible ne doit pas coûter la navigation. */
  }
}

document
  .getElementById("fold")
  .addEventListener("click", () => appliquerRail(racine.dataset.rail !== "replie"));

try {
  appliquerRail(localStorage.getItem("ks-rail") === "replie");
} catch (ignore) {
  appliquerRail(false);
}

/* ── Le repli du « pourquoi » de l'anneau ──────────────────────────────── */

const boutonPourquoi = document.getElementById("unfold-posture");
boutonPourquoi.addEventListener("click", () => {
  const bloc = document.getElementById("posture-why");
  const ouvert = bloc.hidden;
  bloc.hidden = !ouvert;
  boutonPourquoi.setAttribute("aria-expanded", String(ouvert));
  boutonPourquoi.textContent = ouvert ? "Replier" : "Pourquoi";
});

/* ── Palette de commandes ──────────────────────────────────────────────────
 * Parité affichée avec la CLI (P4) : chaque écran montre sa commande `ks`.
 * La recherche d'items porte sur les relevés réels — c'est la même table que
 * celle qu'affichent les écrans, donc la même valeur. */

const scrim = document.getElementById("scrim");
const entree = document.getElementById("pinput");
const resultats = document.getElementById("presults");
let choix = 0;

function ecrans() {
  return Array.from(document.querySelectorAll(".nav")).map((bouton) => ({
    genre: "ecran",
    vue: bouton.dataset.view,
    libelle: bouton.querySelector(".lbl").textContent,
    cli: bouton.dataset.cli,
  }));
}

/** L'écran qui porte un chemin d'item — déduit du chemin, pas deviné. */
function ecranDe(chemin) {
  if (chemin.startsWith("security.")) {
    return "security";
  }
  if (chemin.startsWith("space.")) {
    return "space";
  }
  if (chemin.startsWith("virtualization.")) {
    return "wsl";
  }
  if (chemin.startsWith("inventory.software")) {
    return "software";
  }
  return "machine";
}

function filtrer(requete) {
  const q = requete.trim().toLowerCase();
  const vus = ecrans().filter(
    (e) => q === "" || e.libelle.toLowerCase().includes(q) || e.cli.includes(q)
  );
  const items =
    etat === null
      ? []
      : Object.values(etat.readings)
          .filter(
            (r) =>
              q !== "" &&
              (r.path.toLowerCase().includes(q) || r.value.toLowerCase().includes(q))
          )
          .slice(0, 40)
          .map((r) => ({ genre: "item", releve: r }));
  return { vus, items };
}

function rendrePalette() {
  const { vus, items } = filtrer(entree.value);
  resultats.replaceChildren();
  const plats = [];

  if (vus.length > 0) {
    resultats.append(el("p", "psec", "Écrans"));
    for (const e of vus) {
      const bouton = el("button", "pitem");
      bouton.type = "button";
      bouton.setAttribute("role", "option");
      bouton.append(el("span", null, e.libelle), el("span", "cli", e.cli));
      bouton.addEventListener("click", () => {
        allerA(e.vue);
        fermerPalette();
      });
      resultats.append(bouton);
      plats.push(bouton);
    }
  }

  if (items.length > 0) {
    resultats.append(el("p", "psec", "Items relevés"));
    for (const i of items) {
      const bouton = el("button", "pitem");
      bouton.type = "button";
      bouton.setAttribute("role", "option");
      const valeur = i.releve.readable
        ? i.releve.value
        : `illisible — ${i.releve.reason}`;
      bouton.append(
        el("span", "chemin", i.releve.path),
        el("span", "cli", valeur)
      );
      bouton.addEventListener("click", () => {
        allerA(ecranDe(i.releve.path));
        fermerPalette();
      });
      resultats.append(bouton);
      plats.push(bouton);
    }
  }

  if (plats.length === 0) {
    resultats.append(
      el("p", "pvide", "Aucun écran ni item relevé ne correspond à cette recherche.")
    );
  }

  choix = Math.min(choix, Math.max(0, plats.length - 1));
  for (const [rang, bouton] of plats.entries()) {
    bouton.classList.toggle("sel", rang === choix);
    bouton.setAttribute("aria-selected", String(rang === choix));
  }
  return plats;
}

function ouvrirPalette() {
  scrim.classList.add("on");
  entree.value = "";
  choix = 0;
  rendrePalette();
  window.setTimeout(() => entree.focus(), 40);
}

function fermerPalette() {
  scrim.classList.remove("on");
}

document.getElementById("open-palette").addEventListener("click", ouvrirPalette);

scrim.addEventListener("click", (evenement) => {
  if (evenement.target === scrim) {
    fermerPalette();
  }
});

entree.addEventListener("input", () => {
  choix = 0;
  rendrePalette();
});

entree.addEventListener("keydown", (evenement) => {
  const plats = Array.from(resultats.querySelectorAll(".pitem"));
  if (evenement.key === "ArrowDown" || evenement.key === "ArrowUp") {
    evenement.preventDefault();
    const pas = evenement.key === "ArrowDown" ? 1 : -1;
    choix = Math.max(0, Math.min(plats.length - 1, choix + pas));
    rendrePalette();
    const actif = resultats.querySelector(".sel");
    if (actif) {
      actif.scrollIntoView({ block: "nearest" });
    }
  } else if (evenement.key === "Enter" && plats[choix]) {
    evenement.preventDefault();
    plats[choix].click();
  }
});

window.addEventListener("keydown", (evenement) => {
  const modificateur = evenement.ctrlKey || evenement.metaKey;
  if (modificateur && evenement.key.toLowerCase() === "k") {
    evenement.preventDefault();
    ouvrirPalette();
  } else if (modificateur && evenement.key.toLowerCase() === "b") {
    evenement.preventDefault();
    appliquerRail(racine.dataset.rail !== "replie");
  } else if (evenement.key === "Escape") {
    fermerPalette();
  }
});

/* ── Le pont ───────────────────────────────────────────────────────────── */

function appeler() {
  const pont = window.__TAURI_INTERNALS__;
  if (!pont || typeof pont.invoke !== "function") {
    return Promise.reject(new Error("pont IPC absent"));
  }
  return pont.invoke(COMMANDE, {});
}

async function collecter() {
  corps.dataset.state = "collecting";
  demarrerCompteur();

  try {
    etat = await appeler();
    rendre();
    corps.dataset.state = "ready";
  } catch (echec) {
    /* Le message destiné à l'utilisateur et le détail technique sont deux
     * champs distincts jusqu'à l'écran : l'un se lit, l'autre se copie dans un
     * rapport. Jamais un code d'erreur nu en guise de phrase. */
    const attendu =
      echec !== null &&
      typeof echec === "object" &&
      typeof echec.message === "string" &&
      typeof echec.detail === "string";

    document.getElementById("failure-message").textContent = attendu
      ? echec.message
      : "La coque n'a pas pu joindre le lecteur d'état de Keystone.";
    document.getElementById("failure-detail").textContent = attendu
      ? `detail : ${echec.detail}`
      : `detail : ${String(echec)}`;
    corps.dataset.state = "failed";
  } finally {
    arreterCompteur();
  }
}

for (const id of ["recollect", "relire"]) {
  document.getElementById(id).addEventListener("click", () => {
    void collecter();
  });
}

void collecter();

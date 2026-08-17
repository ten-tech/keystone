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
 * serait honnête.
 *
 * L'AFFICHAGE ET L'ANNONCE SONT DEUX CHOSES. Le premier bat la seconde ; la
 * seconde ne parle que par paliers. Une région live rafraîchie toutes les
 * secondes est correcte au sens strict de la spécification, et produit un flux
 * verbal ininterrompu — soit l'exact contraire de ce que ce produit défend. Le
 * calme est la fonctionnalité, y compris pour qui écoute l'écran.
 *
 * Les paliers : 10 s, puis toutes les 30 s. Assez pour dire que la lecture
 * avance sur un poste lent, jamais assez pour couvrir autre chose. */

const PALIERS_ANNONCE = [10, 30];
const PERIODE_ANNONCE = 30;

/** Le palier atteint à `secondes`, ou `null` si aucun ne l'est exactement. */
function palierAtteint(secondes) {
  if (PALIERS_ANNONCE.includes(secondes)) {
    return secondes;
  }
  return secondes > PERIODE_ANNONCE && secondes % PERIODE_ANNONCE === 0
    ? secondes
    : null;
}

function demarrerCompteur() {
  arreterCompteur();
  const debut = Date.now();
  const cible = document.getElementById("elapsed");
  const annonce = document.getElementById("elapsed-annonce");
  annonce.textContent = "";
  const ecrire = () => {
    const secondes = Math.floor((Date.now() - debut) / 1000);
    cible.textContent = `Temps écoulé : ${secondes} s`;
    const palier = palierAtteint(secondes);
    if (palier !== null) {
      annonce.textContent = `Lecture en cours depuis ${palier} secondes.`;
    }
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
  nommerLeRail();
}

/* Le nom accessible des entrées du rail.
 *
 * LE RAIL REPLIÉ MASQUE LES LIBELLÉS EN `display: none`, et un descendant en
 * `display: none` est exclu du calcul du nom accessible (accname, étape 2F).
 * Sans `aria-label`, cinq boutons sur neuf deviennent des boutons SANS NOM dès
 * qu'on replie le rail, et les quatre autres n'annoncent qu'un nombre nu.
 *
 * L'attribut statique d'index.html donne le nom de base ; cette fonction le
 * recompose avec le décompte et son unité — « Sécurité, N items relevés »
 * plutôt que « N ». Un chiffre sans nom n'apprend rien à qui n'a pas l'écran
 * sous les yeux, et le contexte est justement ce que le repli fait perdre.
 *
 * Elle tourne à chaque remplissage : le décompte change, le nom suit. */
function nommerLeRail() {
  for (const bouton of document.querySelectorAll(".nav")) {
    const libelle = bouton.querySelector(".lbl");
    if (libelle === null) {
      continue;
    }
    const pastille = bouton.querySelector(".tag, .tag-muet");
    let nom = libelle.textContent.trim();
    if (pastille !== null) {
      const valeur = pastille.textContent.trim();
      /* Les pastilles muettes (« sans objet », « non collecté ») portent déjà
       * une phrase : leur coller une unité en ferait un charabia. */
      const unite = pastille.dataset.unite;
      if (valeur !== "") {
        nom =
          unite !== undefined && valeur !== REPLI
            ? `${nom}, ${valeur} ${unite}`
            : `${nom}, ${valeur}`;
      }
    }
    bouton.setAttribute("aria-label", nom);
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
  ["Écart", "Le constaté diffère du désiré.", "deviations"],
  ["Incomparable", "La lecture a échoué : ni conforme, ni en écart.", "incomparable"],
];

/* Le décompte d'un verdict. Les écarts arrivent en LISTE DE CHEMINS — l'écran
 * doit pouvoir en dresser la ligne à ligne —, les trois autres en décompte
 * établi par le noyau. Dans les deux cas le nombre vient du relevé, jamais
 * d'ici : ce sont les deux seules sources, et il n'en existe pas de troisième. */
function decompteDuVerdict(clef) {
  const publie = etat.drift[clef];
  return Array.isArray(publie) ? publie.length : publie;
}

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
      cellule(
        disponible ? String(decompteDuVerdict(clef)) : "sans objet",
        disponible ? "num" : "v-absent"
      )
    );
    cible.append(tr);
  }
}

/* ── Les tolérances ────────────────────────────────────────────────────────
 *
 * UN ÉCART TOLÉRÉ RESTE UN ÉCART. La tolérance s'affiche donc dans une colonne
 * de la ligne de l'écart, comme une annotation, et jamais en rangeant l'item
 * ailleurs : le verdict dit le fait constaté, la tolérance dit la politique
 * décidée. Les faire fusionner ferait sortir de l'écran ce qu'on a précisément
 * choisi d'y garder sous les yeux jusqu'à une date.
 *
 * Chaque état porte une ICÔNE ET UN LIBELLÉ, jamais la couleur seule, et les
 * libellés sont ceux de `ks diff` mot pour mot. « En vigueur » ne reçoit aucune
 * couleur d'état, et surtout pas le vert : une tolérance n'est pas un état sain.
 *
 * `surLecart` vaut `null` là où l'état ne peut pas s'y trouver : une tolérance
 * sans objet vise un item qui n'est pas en écart, une tolérance non observée
 * vise un chemin qu'aucun item ne porte. `aRevoir` vaut `null` pour la seule
 * qui n'est pas à revoir. */
const TOLERANCES = {
  inForce: {
    classe: "tol tol-vigueur",
    icone: "ks-tolere",
    surLecart: (t) =>
      `toléré ${t.daysLeft} j, jusqu'au ${dateLisible(t.expires)}`,
    aRevoir: null,
  },
  expired: {
    classe: "tol tol-echue",
    icone: "ks-echue",
    surLecart: (t) => `tolérance échue depuis ${t.daysSince} j`,
    aRevoir: (t) =>
      `échue depuis ${t.daysSince} j — l'écart est redevenu actif tout seul`,
  },
  moot: {
    classe: "tol tol-nulle",
    icone: "ks-sansobjet",
    surLecart: null,
    aRevoir: () => "l'item n'est plus en écart — la ligne peut être retirée",
  },
  notObserved: {
    classe: "tol tol-nulle",
    icone: "ks-sansobjet",
    surLecart: null,
    aRevoir: () => "aucun item observé ne porte ce chemin",
  },
};

const FORMAT_JOUR = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long" });

/** Une échéance civile, lue dans le calendrier LOCAL puis rendue en français.
 *
 * `new Date("2026-10-15")` est interprété en temps universel par la
 * spécification : à l'ouest de Greenwich, la date rendue serait celle de la
 * veille, et l'écran annoncerait une échéance qui n'est pas celle du fichier.
 * Les trois nombres sont donc posés à la main dans le calendrier local — c'est
 * la même raison qui fait lire la date du jour en heure locale côté Rust. */
function dateLisible(iso) {
  const [an, mois, jour] = String(iso).split("-").map(Number);
  const date = new Date(an, mois - 1, jour);
  return Number.isNaN(date.getTime()) ? String(iso) : FORMAT_JOUR.format(date);
}

/** Un mot accordé au nombre qui le précède. Le chiffre passe devant. */
function accorde(nombre, singulier, pluriel) {
  return nombre > 1 ? pluriel : singulier;
}

/** La pastille d'une tolérance : une icône ET une phrase, jamais l'une sans
 * l'autre.
 *
 * Aucun sens ne repose donc sur la teinte. En contraste forcé les trois se
 * confondent en une seule couleur, et il reste la forme de l'icône — dont le
 * trait est forcé, pas supprimé — et le libellé, qui dit l'état en toutes
 * lettres. */
function pastilleTolerance(tolerance, phrase) {
  const etats = TOLERANCES[tolerance.status];
  const bloc = el("span", etats.classe);
  const dessin = svgEl("svg", "ic");
  dessin.setAttribute("aria-hidden", "true");
  const usage = svgEl("use");
  usage.setAttribute("href", `#${etats.icone}`);
  dessin.append(usage);
  bloc.append(dessin, document.createTextNode(phrase));
  return bloc;
}

/* D'OÙ VIENT L'ÉTAT DÉSIRÉ, ET CE QU'IL A DONNÉ.
 *
 * Le chemin réellement lu s'affiche toujours ; quand il n'y en a pas, tous les
 * emplacements cherchés s'affichent. Un écran qui dit « aucune comparaison »
 * sans nommer le fichier attendu ne laisse aucun geste à faire. */
function rendreSourceEtatDesire() {
  const phrase = document.getElementById("drift-source-l");
  const chemins = document.getElementById("drift-source-chemins");
  const orphelins = document.getElementById("drift-nonobserves");
  chemins.replaceChildren();
  orphelins.replaceChildren();
  const source = etat.desiredState;

  if (source.state === "loaded") {
    phrase.textContent =
      `${source.declared} ${accorde(source.declared, "déclaration posée", "déclarations posées")} ` +
      `sur ${etat.itemCount} ${accorde(etat.itemCount, "item relevé", "items relevés")}.`;
    chemins.append(el("p", "detail", source.path));
  } else if (source.state === "unusable") {
    phrase.textContent =
      "Le fichier a été trouvé et refusé : rien n'a été comparé. La cause est nommée plus bas.";
    chemins.append(el("p", "detail", source.path));
  } else {
    phrase.textContent =
      "Aucun fichier trouvé. Emplacements cherchés, dans l'ordre où ils l'ont été :";
    for (const chemin of source.searched) {
      chemins.append(el("p", "detail", chemin));
    }
  }

  /* Déclarés, non observés. Deux causes, et rien ne les distingue : une faute
   * de frappe dans le chemin, ou un item qui a légitimement disparu. Les deux
   * se nomment, sans en choisir une. */
  const perdus = source.state === "loaded" ? source.notObserved : [];
  if (perdus.length === 0) {
    return;
  }
  orphelins.append(
    el(
      "p",
      "indispo-2",
      "Déclarés, non observés : une faute de frappe dans le chemin, ou un item qui a " +
        "légitimement disparu. Rien ne permet de trancher entre les deux, et Keystone " +
        "ne tranche pas à votre place."
    )
  );
  for (const chemin of perdus) {
    orphelins.append(el("p", "detail", chemin));
  }
}

/* LES ÉCARTS, ITEM PAR ITEM. Le décompte est la longueur de la liste reçue, et
 * chaque ligne vient de cette même liste : les deux ne peuvent pas se
 * contredire. Les valeurs, elles, viennent de la table des relevés — la seule
 * source des valeurs affichées. */
function rendreEcarts() {
  const carte = document.getElementById("drift-ecarts");
  const cadre = document.getElementById("drift-ecarts-cadre");
  const lignes = document.getElementById("drift-ecarts-lignes");
  lignes.replaceChildren();

  if (etat.drift.state !== "computed") {
    carte.hidden = true;
    return;
  }
  carte.hidden = false;

  const tolerances = new Map(etat.drift.tolerances.map((t) => [t.path, t]));
  const chemins = etat.drift.deviations;
  /* Seules les tolérances EN VIGUEUR se comptent ici. Une tolérance échue reste
   * sur la ligne de son écart — elle explique d'où il vient —, mais la compter
   * parmi les tolérés contredirait la ligne juste en dessous, qui dit que
   * l'écart est redevenu actif tout seul. Le défaut s'est vu à l'écran, pas au
   * code : « deux en écart, dont deux tolérés » au-dessus d'une ligne « échue ». */
  const tenues = chemins.filter((c) => {
    const tolerance = tolerances.get(c);
    return tolerance !== undefined && tolerance.status === "inForce";
  }).length;
  cadre.hidden = chemins.length === 0;

  document.getElementById("drift-ecarts-l").textContent =
    chemins.length === 0
      ? "Aucun item déclaré ne diffère de sa valeur constatée."
      : `${chemins.length} ${accorde(chemins.length, "item en écart", "items en écart")}` +
        (tenues === 0
          ? "."
          : `, dont ${tenues} ${accorde(tenues, "toléré", "tolérés")}.`);

  for (const chemin of chemins) {
    const releve = releveDe(chemin);
    const tr = document.createElement("tr");
    const th = el("th", "chemin", chemin);
    th.setAttribute("scope", "row");
    tr.append(
      th,
      cellule(releve === null || releve.desired === null ? REPLI : releve.desired, "valeur"),
      cellule(releve === null ? REPLI : releve.value, "valeur")
    );

    const tolerance = tolerances.get(chemin);
    const dire = tolerance === undefined ? null : TOLERANCES[tolerance.status].surLecart;
    if (dire === null) {
      tr.append(cellule("aucune", "v-absent"));
    } else {
      const case_ = document.createElement("td");
      case_.append(pastilleTolerance(tolerance, dire(tolerance)));
      tr.append(case_);
    }
    lignes.append(tr);
  }
}

/* LES TOLÉRANCES QUI NE COUVRENT PLUS RIEN. Elles ne décrivent pas la machine,
 * elles décrivent le fichier : échéance dépassée, tolérance devenue sans objet,
 * chemin disparu — trois manières dont un état désiré pourrit en silence. */
function rendreTolerancesARevoir() {
  const carte = document.getElementById("drift-tolerances");
  const lignes = document.getElementById("drift-tolerances-lignes");
  lignes.replaceChildren();

  const aRevoir =
    etat.drift.state === "computed"
      ? etat.drift.tolerances.filter((t) => TOLERANCES[t.status].aRevoir !== null)
      : [];
  carte.hidden = aRevoir.length === 0;

  for (const tolerance of aRevoir) {
    const tr = document.createElement("tr");
    const th = el("th", "chemin", tolerance.path);
    th.setAttribute("scope", "row");
    const pourquoi = document.createElement("td");
    pourquoi.append(
      pastilleTolerance(tolerance, TOLERANCES[tolerance.status].aRevoir(tolerance))
    );
    tr.append(
      th,
      pourquoi,
      cellule(tolerance.reason, "but"),
      cellule(dateLisible(tolerance.expires), "valeur")
    );
    lignes.append(tr);
  }
}

/* La dérive n'a de décomptes que si un fichier d'état désiré a été confronté.
 * Tant qu'il n'y en a pas, l'explication reste visible et AUCUN décompte n'est
 * publié — pas même un zéro. Dès qu'une référence est lue, ce sont les
 * décomptes qui parlent et l'explication qui s'efface. */
function rendreDerive() {
  const comparable = etat.drift.state === "computed";
  document.getElementById("drift-indispo").hidden = comparable;
  rendreSourceEtatDesire();
  rendreEcarts();
  rendreTolerancesARevoir();

  const tuile = document.getElementById("tile-drift");
  const badge = tuile.querySelector(".badge");
  tuile.classList.toggle("tile-none", !comparable);
  badge.classList.toggle("badge-muet", !comparable);
  /* L'icône suit le libellé : une pastille qui dit « comparé » sous le glyphe
   * « sans objet » se contredit elle-même. */
  badge
    .querySelector("use")
    .setAttribute("href", comparable ? "#ks-compare" : "#ks-sansobjet");

  /* La pastille du rail cesse de dire « sans objet » dès qu'une comparaison a
   * eu lieu : elle porte alors le nombre d'écarts publiés, qui est la longueur
   * de la liste reçue. Le nom accessible se recalcule avec lui. */
  const pastille = document.querySelector('[data-view="drift"] .tag, [data-view="drift"] .tag-muet');
  if (!comparable) {
    pastille.className = "tag tag-muet";
    pastille.textContent = "sans objet";
    delete pastille.dataset.unite;
    nommerLeRail();
    return;
  }
  pastille.className = "tag";
  pastille.dataset.unite = "en écart";
  pastille.textContent = String(etat.drift.deviations.length);
  nommerLeRail();

  const tenues = etat.drift.tolerances.filter((t) => t.status === "inForce").length;
  tuile.querySelector(".tile-none-v").textContent =
    `${etat.drift.deviations.length} en écart`;
  tuile.querySelector(".tile-c").textContent =
    `${etat.drift.compliant} conformes, ${etat.drift.incomparable} incomparables, ` +
    `${etat.drift.unconstrained} non contraints` +
    (tenues === 0
      ? "."
      : `, dont ${tenues} ${accorde(tenues, "écart toléré", "écarts tolérés")}.`);
  badge.querySelector("span").textContent = "Comparé";
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

/* ══ FIGURES ═══════════════════════════════════════════════════════════════
 *
 * Du SVG écrit à la main : aucune bibliothèque de graphes, aucune ressource
 * réseau, aucun jeu de données de démonstration.
 *
 * ── AUCUN CHIFFRE N'EST ÉCRIT ICI ────────────────────────────────────────
 *
 * Une part de figure ne reçoit jamais un nombre : elle reçoit une LISTE DE
 * CHEMINS et compte ce qu'on lui donne, ou elle lit un décompte que le
 * processus Rust a établi en comptant de vrais items. Il n'existe pas de
 * troisième source, et le test
 * `aucune_figure_ne_publie_un_nombre_ecrit_a_la_main` en fait une barrière :
 * écrire `valeur: 34` dans ce fichier fait échouer la suite.
 *
 * La conséquence est que la longueur d'un segment et la ligne de tableau qui
 * lui répond viennent littéralement de la même liste. Elles ne peuvent pas se
 * contredire.
 *
 * ── LA GÉOMÉTRIE EST EN PIXELS RÉELS ─────────────────────────────────────
 *
 * Pas de `viewBox` mise à l'échelle : elle redimensionnerait aussi le texte,
 * qui sortirait de l'échelle typographique à chaque largeur de fenêtre. Les
 * figures se redessinent donc quand la largeur de leur conteneur change — ce
 * qui couvre aussi le repliage du rail, et l'apparition d'un écran resté
 * masqué, dont la largeur vaut zéro tant qu'il n'est pas affiché.
 *
 * Les distances viennent des jetons, jamais d'un nombre choisi ici.
 */

/* L'espace de noms SVG se LIT sur une balise du document au lieu de s'écrire :
 * il contient une URL, et le garde-fou P5 refuse le préfixe d'une URL sans
 * exception — y compris dans un espace de noms XML, où il ne désigne pourtant
 * aucune ressource à charger. Le document en porte déjà : le sprite d'icônes. */
const NS_SVG = document.querySelector("svg").namespaceURI;

const bulle = document.getElementById("fig-bulle");

/** Un élément SVG, avec sa classe. */
function svgEl(balise, classe) {
  const noeud = document.createElementNS(NS_SVG, balise);
  if (classe) {
    /* `className` d'un élément SVG est en lecture seule côté script : c'est un
     * `SVGAnimatedString`, pas une chaîne. */
    noeud.setAttribute("class", classe);
  }
  return noeud;
}

/** La valeur en pixels d'un jeton de géométrie. */
function jeton(nom) {
  return Number.parseFloat(getComputedStyle(racine).getPropertyValue(nom));
}

/** Une part de figure : un libellé, une couleur, et des CHEMINS d'items.
 *
 * La valeur est le nombre de chemins, donc un décompte d'items réellement
 * relevés. Les chemins restent attachés à la part : c'est eux que le tableau
 * équivalent redéroule, et non un second décompte. */
function part(libelle, classe, chemins) {
  return { libelle, classe, chemins, valeur: chemins.length };
}

/** La part d'un verdict, dont le décompte vient du noyau.
 *
 * `ks_core` compte les verdicts par un `match` exhaustif sans bras `_` ; la
 * page ne recompte pas, elle lit. */
function partDuVerdict(libelle, classe, clef) {
  return { libelle, classe, chemins: null, valeur: etat.drift[clef] };
}

/** Le total d'une figure : la somme de ses parts, jamais un nombre écrit. */
function totalDe(parts) {
  return parts.reduce((somme, p) => somme + p.valeur, 0);
}

/** Le tracé d'un rectangle dont seules certaines extrémités sont arrondies.
 *
 * L'extrémité qui porte la donnée s'arrondit ; celle qui touche la ligne de
 * base ou un segment voisin reste d'équerre. Un `rx` sur `<rect>` arrondit les
 * quatre coins, ce qui creuserait un losange entre deux segments empilés. */
function trace(x, y, largeur, hauteur, rGauche, rDroite) {
  const borne = Math.min(hauteur / 2, largeur / 2);
  const g = Math.max(0, Math.min(rGauche, borne));
  const d = Math.max(0, Math.min(rDroite, borne));
  return [
    `M${x + g},${y}`,
    `H${x + largeur - d}`,
    d > 0 ? `A${d},${d} 0 0 1 ${x + largeur},${y + d}` : "",
    `V${y + hauteur - d}`,
    d > 0 ? `A${d},${d} 0 0 1 ${x + largeur - d},${y + hauteur}` : "",
    `H${x + g}`,
    g > 0 ? `A${g},${g} 0 0 1 ${x},${y + hauteur - g}` : "",
    `V${y + g}`,
    g > 0 ? `A${g},${g} 0 0 1 ${x + g},${y}` : "",
    "Z",
  ].join(" ");
}

/* ── L'infobulle ──────────────────────────────────────────────────────────
 * Elle enrichit et ne conditionne rien : la valeur qu'elle montre est déjà
 * dans la légende et dans le tableau. La valeur passe devant le libellé —
 * à ce moment-là le lecteur tient la série, c'est le nombre qu'il cherche. */

function montrerBulle(cible, valeur, libelle) {
  bulle.replaceChildren(
    el("b", null, String(valeur)),
    document.createTextNode(` ${libelle}`)
  );
  bulle.hidden = false;
  const marque = cible.getBoundingClientRect();
  const propre = bulle.getBoundingClientRect();
  const marge = jeton("--sp-2");
  const gauche = Math.min(
    Math.max(marge, marque.left + marque.width / 2 - propre.width / 2),
    window.innerWidth - propre.width - marge
  );
  const haut = marque.top - propre.height - marge;
  bulle.style.left = `${gauche}px`;
  bulle.style.top = `${haut < marge ? marque.bottom + marge : haut}px`;
}

function masquerBulle() {
  bulle.hidden = true;
}

/** Rend un groupe interactif : survol ET focus, avec le même contenu. */
function rendreAtteignable(groupe, valeur, libelle, nomAccessible) {
  groupe.setAttribute("tabindex", "0");
  groupe.setAttribute("role", "img");
  groupe.setAttribute("aria-label", nomAccessible);
  for (const evenement of ["pointerenter", "focus"]) {
    groupe.addEventListener(evenement, () =>
      montrerBulle(groupe, valeur, libelle)
    );
  }
  for (const evenement of ["pointerleave", "blur"]) {
    groupe.addEventListener(evenement, masquerBulle);
  }
}

/** La toile d'une figure : un SVG vide, à la largeur de son conteneur. */
function toile(figure, hauteur, nom) {
  const plot = figure.querySelector(".fig-plot");
  plot.replaceChildren();
  const dessin = svgEl("svg", "fig-svg");
  dessin.setAttribute("height", String(hauteur));
  dessin.setAttribute("role", "group");
  dessin.setAttribute("aria-label", nom);
  plot.append(dessin);
  return dessin;
}

/** La figure qu'on ne trace pas, et qui dit pourquoi.
 *
 * Elle porte la hachure du dessin technique — « zone non levée » — et non un
 * cadre vide : une figure absente doit se voir comme absente, jamais se
 * confondre avec une figure dont toutes les valeurs seraient nulles. */
function figureVide(figure, titre, phrase) {
  const plot = figure.querySelector(".fig-plot");
  const bloc = el("p", "fig-vide");
  bloc.append(el("b", null, titre), document.createTextNode(phrase));
  plot.replaceChildren(bloc);
  figure.querySelector(".legende").replaceChildren();
}

/* La pastille qui répond à chaque remplissage. La correspondance est ÉCRITE,
 * et non dérivée du nom de la classe : une classe fabriquée par découpage de
 * chaîne n'apparaît nulle part dans les fichiers, donc aucune vérification ne
 * peut la voir — ni celle qui contrôle que le contraste forcé vise des classes
 * réelles, ni un simple `grep` en revue. */
const PUCES = {
  "fig-s1": "puce-s1",
  "fig-s2": "puce-s2",
  "fig-s3": "puce-s3",
  "fig-s4": "puce-s4",
  "fig-s5": "puce-s5",
  "fig-other": "puce-other",
  "fig-nonlevee": "puce-nonlevee",
};

/** La légende : présente dès deux parts, et elle porte la valeur. */
function rendreLegende(figure, parts) {
  const liste = figure.querySelector(".legende");
  liste.replaceChildren();
  for (const p of parts) {
    const ligne = document.createElement("li");
    ligne.append(
      el("span", `puce ${PUCES[p.classe]}`),
      document.createTextNode(p.libelle),
      el("b", null, String(p.valeur))
    );
    liste.append(ligne);
  }
}

/* ── La barre empilée ─────────────────────────────────────────────────────
 * Une part-à-tout sur un total connu. Jamais un camembert : deux angles
 * proches ne se comparent pas, deux longueurs sur une même ligne, si.
 *
 * UNE PART À ZÉRO NE DISPARAÎT PAS. Elle garde un repère sur la ligne, parce
 * qu'un segment absent se lit comme une catégorie absente — or zéro écart est
 * un résultat, et c'en est même un bon. */
function metriqueBarre() {
  return {
    hauteur: jeton("--sp-5"),
    /* La bande dépasse la barre en haut et en bas : c'est là que le repère
     * d'un compte à zéro trouve la place d'être vu. Un SVG rogne ce qui
     * déborde de sa boîte, donc la place se réserve, elle ne se prend pas. */
    marge: jeton("--sp-1"),
    ecart: jeton("--gap-fill"),
    rayon: jeton("--sp-1"),
  };
}

function barreEmpilee(figure, parts, nom) {
  const plot = figure.querySelector(".fig-plot");
  const largeur = plot.clientWidth;
  const total = totalDe(parts);
  if (largeur === 0 || total === 0) {
    return false;
  }

  const { hauteur, marge, ecart, rayon } = metriqueBarre();
  const bande = hauteur + marge * 2;
  const cible = jeton("--sp-5");
  const dessin = toile(figure, bande, nom);
  const echelle = largeur / total;

  let cumul = 0;
  for (const [rang, p] of parts.entries()) {
    const debut = cumul * echelle;
    const fin = (cumul + p.valeur) * echelle;
    cumul += p.valeur;

    const groupe = svgEl("g", "fig-groupe");
    if (p.valeur === 0) {
      /* LE REPÈRE DU ZÉRO. Un segment de longueur nulle ne se dessine pas, et
       * une catégorie qui ne se dessine pas se lit comme une catégorie qui
       * n'existe pas — or zéro écart est un résultat, et c'en est un bon. Le
       * repère traverse donc toute la bande à l'endroit exact où le segment
       * aurait commencé, et la légende en donne le compte en toutes lettres. */
      const repere = svgEl("rect", "fig-zero");
      repere.setAttribute("x", String(Math.max(0, debut - ecart / 2)));
      repere.setAttribute("y", "0");
      repere.setAttribute("width", String(ecart));
      repere.setAttribute("height", String(bande));
      groupe.append(repere);
    } else {
      const gauche = rang === 0 ? debut : debut + ecart / 2;
      const droite = rang === parts.length - 1 ? fin : fin - ecart / 2;
      const marque = svgEl("path", `fig-marque ${p.classe}`);
      marque.setAttribute(
        "d",
        trace(
          gauche,
          marge,
          Math.max(1, droite - gauche),
          hauteur,
          rang === 0 ? rayon : 0,
          rang === parts.length - 1 ? rayon : 0
        )
      );
      groupe.append(marque);
    }

    /* La cible déborde la marque : un segment étroit reste attrapable. */
    const zone = svgEl("rect", "fig-cible");
    const milieu = (debut + fin) / 2;
    const large = Math.max(cible, fin - debut);
    zone.setAttribute("x", String(Math.max(0, milieu - large / 2)));
    zone.setAttribute("y", "0");
    zone.setAttribute("width", String(large));
    zone.setAttribute("height", String(bande));
    groupe.append(zone);

    rendreAtteignable(
      groupe,
      p.valeur,
      p.libelle,
      `${p.libelle} : ${p.valeur} sur ${total}`
    );
    dessin.append(groupe);
  }

  rendreLegende(figure, parts);
  return true;
}

/* ── Les barres horizontales ──────────────────────────────────────────────
 * Horizontales et non verticales : les libellés de familles sont longs, et une
 * colonne les coucherait ou les tronquerait. Triées par longueur, parce que
 * l'ordre alphabétique d'une famille n'apprend rien.
 *
 * Les gouttières se MESURENT sur le texte rendu au lieu d'être choisies : une
 * marge trop courte rogne un libellé, une marge trop large vole la place des
 * barres, et ni l'une ni l'autre ne se voit dans le fichier. */
function barresHorizontales(figure, lignes, nom) {
  const plot = figure.querySelector(".fig-plot");
  const largeur = plot.clientWidth;
  if (largeur === 0 || lignes.length === 0) {
    return false;
  }

  const hauteur = jeton("--sp-5");
  const ecart = jeton("--gap-fill");
  const rayon = jeton("--sp-1");
  const respiration = jeton("--sp-3");
  const pas = hauteur + respiration;
  const dessin = toile(figure, lignes.length * pas - respiration, nom);

  /* Première passe : poser les textes pour les mesurer. */
  const noms = [];
  const valeurs = [];
  for (const [rang, ligne] of lignes.entries()) {
    const y = rang * pas + hauteur / 2;
    const nomLigne = svgEl("text", "fig-etiq");
    nomLigne.setAttribute("y", String(y));
    nomLigne.setAttribute("dominant-baseline", "middle");
    nomLigne.setAttribute("text-anchor", "end");
    nomLigne.textContent = ligne.nom;
    dessin.append(nomLigne);
    noms.push(nomLigne);

    const valeur = svgEl("text", "fig-valeur");
    valeur.setAttribute("y", String(y));
    valeur.setAttribute("dominant-baseline", "middle");
    valeur.textContent = String(totalDe(ligne.parts));
    dessin.append(valeur);
    valeurs.push(valeur);
  }

  const mesure = (noeuds) =>
    noeuds.reduce((max, n) => Math.max(max, n.getComputedTextLength()), 0);
  const gouttiereG = mesure(noms) + respiration;
  const gouttiereD = mesure(valeurs) + respiration;
  const piste = Math.max(1, largeur - gouttiereG - gouttiereD);
  const plafond = lignes.reduce((max, l) => Math.max(max, totalDe(l.parts)), 0);
  if (plafond === 0) {
    return false;
  }
  const echelle = piste / plafond;

  for (const n of noms) {
    n.setAttribute("x", String(gouttiereG - respiration));
  }

  for (const [rang, ligne] of lignes.entries()) {
    const y = rang * pas;
    const derniere = ligne.parts.filter((p) => p.valeur > 0).length - 1;
    let cumul = 0;
    let vue = 0;
    for (const p of ligne.parts) {
      if (p.valeur === 0) {
        continue;
      }
      const debut = gouttiereG + cumul * echelle;
      const fin = gouttiereG + (cumul + p.valeur) * echelle;
      cumul += p.valeur;
      const gauche = vue === 0 ? debut : debut + ecart / 2;
      const droite = vue === derniere ? fin : fin - ecart / 2;

      const groupe = svgEl("g", "fig-groupe");
      const marque = svgEl("path", `fig-marque ${p.classe}`);
      marque.setAttribute(
        "d",
        trace(
          gauche,
          y,
          Math.max(1, droite - gauche),
          hauteur,
          0,
          vue === derniere ? rayon : 0
        )
      );
      if (p.motif) {
        marque.setAttribute("fill", `url(#${p.motif})`);
      }
      groupe.append(marque);

      const zone = svgEl("rect", "fig-cible");
      zone.setAttribute("x", String(debut));
      zone.setAttribute("y", String(y));
      zone.setAttribute("width", String(Math.max(1, fin - debut)));
      zone.setAttribute("height", String(hauteur));
      groupe.append(zone);

      rendreAtteignable(
        groupe,
        p.valeur,
        `${p.libelle} — ${ligne.nom}`,
        `${ligne.nom}, ${p.libelle} : ${p.valeur}`
      );
      dessin.append(groupe);
      vue += 1;
    }
    valeurs[rang].setAttribute(
      "x",
      String(gouttiereG + totalDe(ligne.parts) * echelle + respiration)
    );
  }
  return true;
}

/** Le motif de hachure, déposé dans la figure qui l'emploie.
 *
 * Hachure à 45°, pas de sept pixels, trait de 1,15 px : les valeurs exactes du
 * motif `ks-nonreleve` de la marque, qui veut dire « zone non levée » en
 * dessin technique. Elle est déposée DANS le SVG qui s'en sert, et non dans un
 * bloc commun : un motif est une ressource référencée par identifiant, et deux
 * figures qui partageraient le même identifiant se disputeraient le motif. */
function deposerHachure(dessin, identifiant) {
  const defs = svgEl("defs", null);
  const motif = svgEl("pattern", null);
  motif.setAttribute("id", identifiant);
  motif.setAttribute("width", "7");
  motif.setAttribute("height", "7");
  motif.setAttribute("patternUnits", "userSpaceOnUse");
  motif.setAttribute("patternTransform", "rotate(45)");
  const fond = svgEl("rect", "fig-hachure-fond");
  fond.setAttribute("width", "7");
  fond.setAttribute("height", "7");
  const trait = svgEl("line", "fig-hachure-trait");
  trait.setAttribute("y2", "7");
  motif.append(fond, trait);
  defs.append(motif);
  dessin.prepend(defs);
}

/* ── Figure : ce qui se déclare, et ce qui ne se déclare pas ───────────────
 * La question à laquelle le tableau des domaines ne répond pas : pourquoi une
 * fraction seulement des items relevés peut finir dans le fichier d'état
 * désiré. La réponse tient dans la nature de chaque item, et elle est visible
 * d'un coup d'œil sur une barre empilée. */
function rendreFigureNatures() {
  const figure = document.getElementById("fig-natures");
  const couleurs = {
    reglage: "fig-s1",
    objectif: "fig-s2",
    mesure: "fig-s3",
    constat: "fig-s4",
  };
  const parts = etat.natures.map((n) =>
    part(n.label, couleurs[n.key] || "fig-other", n.paths)
  );
  if (parts.length === 0) {
    figureVide(
      figure,
      "Aucun item relevé",
      "La figure attend un relevé : sans items, il n'y a pas de répartition, " +
        "et une barre vide ressemblerait à une répartition nulle."
    );
    return;
  }
  if (!barreEmpilee(figure, parts, "Répartition des items relevés par nature")) {
    return;
  }

  /* L'accolade des déclarables. C'est elle qui porte le propos de la figure :
   * la couleur dit l'identité, l'annotation dit l'enjeu. Elle est tracée après
   * la barre, dans le même dessin, et n'est jamais portée par la teinte. */
  const dessin = figure.querySelector(".fig-svg");
  const declarables = etat.natures.filter((n) => n.declarable);
  const comptes = declarables.reduce((somme, n) => somme + n.paths.length, 0);
  if (comptes === 0) {
    return;
  }
  const respiration = jeton("--sp-2");
  const { hauteur, marge } = metriqueBarre();
  const largeur = figure.querySelector(".fig-plot").clientWidth;
  const fin = (comptes / totalDe(parts)) * largeur;
  const base = hauteur + marge * 2 + respiration;

  const accolade = svgEl("path", "fig-accolade");
  accolade.setAttribute(
    "d",
    `M0,${base - respiration / 2} V${base} H${fin} V${base - respiration / 2}`
  );
  /* LA CHAÎNE COMPLÈTE, ET PAS SEULEMENT SON PREMIER MAILLON. La barre explique
   * pourquoi une fraction seulement des items relevés est déclarable ; il reste
   * la marche suivante, celle qui surprend en lisant le fichier produit — un
   * item déclarable dont la lecture a été refusée ne s'écrit pas non plus, faute
   * de valeur à écrire. Les deux nombres se comptent sur les mêmes listes de
   * chemins que les segments, donc ils ne peuvent pas les contredire. */
  const refuses = declarables
    .flatMap((n) => n.paths)
    .filter((chemin) => !lisible(chemin)).length;
  const note = svgEl("text", "fig-note");
  note.setAttribute("x", "0");
  note.setAttribute("y", String(base + respiration * 2));
  const court = `${comptes} items sur ${totalDe(parts)} ont vocation à être déclarés`;
  note.textContent =
    refuses === 0 ? court : `${court}, dont ${refuses} illisibles`;
  dessin.append(accolade, note);

  /* Une étiquette ne se fait jamais rogner : on la MESURE, et on retombe sur la
   * phrase courte si la longue ne tient pas. Le nombre d'illisibles reste alors
   * lisible sur l'écran de posture et dans son tableau — rien n'est gaté. */
  if (note.getComputedTextLength() > largeur) {
    note.textContent = court;
  }
  dessin.setAttribute("height", String(base + respiration * 3));
}

/* LES TABLEAUX ÉQUIVALENTS NE DÉPENDENT PAS DE LA MISE EN PAGE. Une figure ne
 * se dessine que si son conteneur a une largeur — un écran masqué n'en a pas —
 * alors que son équivalent textuel, lui, doit exister dès que l'état est là.
 * Les deux sont donc rendus séparément, à partir des mêmes listes de chemins. */
function rendreTableNatures() {
  const lignes = document.getElementById("natures-lignes");
  lignes.replaceChildren();
  for (const n of etat.natures) {
    const tr = document.createElement("tr");
    const th = el("th", null, n.label);
    th.setAttribute("scope", "row");
    tr.append(
      th,
      cellule(n.declarable ? "oui" : "non", n.declarable ? "valeur" : "v-absent"),
      cellule(String(n.paths.length), "num"),
      cellule(n.meaning, "but")
    );
    lignes.append(tr);
  }
}

/* ── Figure : les verdicts ────────────────────────────────────────────────
 * « Écart » est un ÉTAT, pas une série : chaque part porte son libellé et sa
 * valeur dans la légende, et l'icône du tableau reste la sienne. Le vert et le
 * magenta redoublent le mot, ils ne le remplacent pas.
 *
 * Tant qu'aucun état désiré n'est chargé, la figure NE TRACE RIEN. Il n'y a
 * pas zéro écart : il n'y a pas de comparaison, et une barre entièrement grise
 * laisserait croire qu'une confrontation a eu lieu. */
function rendreFigureVerdicts() {
  const figure = document.getElementById("fig-verdicts");
  if (etat.drift.state !== "computed") {
    figureVide(
      figure,
      "Pas de barre : rien n'a été comparé",
      "Une part-à-tout suppose un tout confronté à une référence. Sans état " +
        "désiré, les trois verdicts de comparaison sont sans objet, et le " +
        "seul décompte réel — les items lus sans contrainte déclarée — n'est " +
        "pas une part de quoi que ce soit. Le tableau ci-dessous les publie " +
        "tels quels."
    );
    return;
  }
  /* Les écarts arrivent en liste de chemins : la part les COMPTE, exactement
   * comme les figures de répartition. Les trois autres lisent un décompte que
   * le noyau a établi par un `match` exhaustif. Aucune troisième source. */
  const parts = [
    partDuVerdict("Conformes", "fig-s5", "compliant"),
    part("Écarts", "fig-s4", etat.drift.deviations),
    partDuVerdict("Incomparables", "fig-s2", "incomparable"),
    partDuVerdict("Non contraints", "fig-other", "unconstrained"),
  ];
  barreEmpilee(figure, parts, "Répartition des items relevés par verdict");
}

/* ── Figure : les items relevés par famille ───────────────────────────────
 * Une seule série, donc une seule couleur : colorer chaque barre selon sa
 * longueur redirait en teinte ce que la longueur dit déjà, et dépenserait le
 * seul canal libre pour rien.
 *
 * La part dont la lecture a été refusée n'est ni un manque ni un zéro : elle
 * porte la hachure, pas une couleur d'alerte. */
/** Le relevé de ce chemin a-t-il abouti ? Une absence n'est pas une lecture. */
function lisible(chemin) {
  const releve = releveDe(chemin);
  return releve !== null && releve.readable;
}

/** Les familles de la posture, triées par nombre d'items décroissant. */
function famillesTriees() {
  return etat.security.families
    .map((f) => ({
      nom: f.label,
      parts: [
        part("lus", "fig-s1", f.paths.filter(lisible)),
        part("lecture refusée", "fig-nonlevee", f.paths.filter((c) => !lisible(c))),
      ],
    }))
    .sort((a, b) => totalDe(b.parts) - totalDe(a.parts));
}

function rendreFigureFamilles() {
  const figure = document.getElementById("fig-familles");
  const lignes = famillesTriees();
  for (const ligne of lignes) {
    ligne.parts[1].motif = "fig-hachure-familles";
  }

  if (lignes.length === 0) {
    figureVide(
      figure,
      "Aucune famille relevée",
      "Le collecteur de posture n'a produit aucun item sur ce poste."
    );
    return;
  }
  if (
    !barresHorizontales(
      figure,
      lignes,
      "Items relevés par famille de la posture de sécurité"
    )
  ) {
    return;
  }
  deposerHachure(figure.querySelector(".fig-svg"), "fig-hachure-familles");

  const tous = etat.security.families.flatMap((f) => f.paths);
  rendreLegende(figure, [
    part("lus", "fig-s1", tous.filter(lisible)),
    part("lecture refusée", "fig-nonlevee", tous.filter((c) => !lisible(c))),
  ]);
}

function rendreTableFamilles() {
  const corpsTable = document.getElementById("familles-compte");
  corpsTable.replaceChildren();
  for (const ligne of famillesTriees()) {
    const tr = document.createElement("tr");
    const th = el("th", null, ligne.nom);
    th.setAttribute("scope", "row");
    const refuses = ligne.parts[1].valeur;
    tr.append(
      th,
      cellule(String(totalDe(ligne.parts)), "num"),
      cellule(String(ligne.parts[0].valeur), "num"),
      cellule(String(refuses), refuses === 0 ? "num v-absent" : "num v-illisible")
    );
    corpsTable.append(tr);
  }
}

/* Les figures se redessinent quand la largeur de leur conteneur change. C'est
 * ce qui les rend justes après un repliage du rail, un changement d'écran — un
 * conteneur masqué a une largeur nulle — ou un redimensionnement de fenêtre. */
const FIGURES = {
  "fig-natures": rendreFigureNatures,
  "fig-verdicts": rendreFigureVerdicts,
  "fig-familles": rendreFigureFamilles,
};

const largeursRendues = new Map();

function rendreFigures() {
  if (etat === null) {
    return;
  }
  masquerBulle();
  for (const [id, rendu] of Object.entries(FIGURES)) {
    const plot = document.getElementById(id).querySelector(".fig-plot");
    if (plot.clientWidth === 0) {
      continue;
    }
    largeursRendues.set(id, plot.clientWidth);
    rendu();
  }
}

const observateur = new ResizeObserver((entrees) => {
  for (const entree of entrees) {
    const figure = entree.target.closest(".fig");
    const large = Math.round(entree.contentRect.width);
    if (etat === null || large === 0 || largeursRendues.get(figure.id) === large) {
      continue;
    }
    largeursRendues.set(figure.id, large);
    masquerBulle();
    FIGURES[figure.id]();
  }
});

for (const id of Object.keys(FIGURES)) {
  observateur.observe(document.getElementById(id).querySelector(".fig-plot"));
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
  rendreTableNatures();
  rendreTableFamilles();
  largeursRendues.clear();
  rendreFigures();

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

/* Une première passe avant même la collecte : les pastilles muettes portent
 * déjà leur phrase, et le rail peut être replié dès l'ouverture. */
nommerLeRail();

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

/* L'élément qui a ouvert la palette. Le focus lui revient à CHAQUE sortie —
 * Échap, clic sur le voile, sélection d'un résultat. Sans cela il retombe au
 * début du document, et qui navigue au clavier recommence sa traversée. */
let declencheur = null;

/* Les conteneurs qui doivent devenir inertes pendant que la feuille est
 * ouverte. `aria-modal="true"` PROMET qu'ils ne sont plus atteignables ; sans
 * ce geste, la promesse est fausse et le focus s'en va tabuler sur le rail,
 * sous un voile translucide, donc invisible. */
const FONDS = ["app", "waiting", "failure"];

/* `inert` est vérifié, pas supposé : la coque tourne dans la WebView2 du
 * poste, dont la version n'est pas celle du navigateur de développement.
 * L'attribut est soutenu depuis Chromium 102 ; là où il ne l'est pas, le
 * gestionnaire de `Tab` plus bas prend le relais, et il reste posé dans les
 * deux cas — deux barrières valent mieux qu'une promesse. */
const INERT_SOUTENU = "inert" in HTMLElement.prototype;

/** Ce qui peut recevoir le focus dans la feuille, dans l'ordre du document.
 *
 * Tout y est visible tant que la feuille est ouverte : le champ, et les
 * résultats que le rendu vient de poser. Rien à filtrer. */
function focusablesDeLaPalette() {
  return Array.from(
    scrim.querySelectorAll("input, button:not([disabled]), [tabindex]:not([tabindex='-1'])")
  );
}

function fondsInertes(inertes) {
  for (const id of FONDS) {
    const noeud = document.getElementById(id);
    if (noeud === null) {
      continue;
    }
    if (INERT_SOUTENU) {
      /* `inert` retire à la fois le focus ET l'arbre d'accessibilité : rien
       * d'autre n'est nécessaire, et poser `aria-hidden` par-dessus dirait
       * deux fois la même chose. */
      noeud.inert = inertes;
      continue;
    }
    /* Sans `inert`, la même intention se joue en deux gestes incomplets :
     * `aria-hidden` pour les lecteurs d'écran, et le gestionnaire de `Tab`
     * plus bas pour le focus, qu'`aria-hidden` ne retire pas. */
    if (inertes) {
      noeud.setAttribute("aria-hidden", "true");
    } else {
      noeud.removeAttribute("aria-hidden");
    }
  }
}

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

/* Replie un texte sur sa forme sans accent ni casse, pour la recherche.
 *
 * **Mesuré, pas supposé** : la palette comparait les chaînes telles quelles, et
 * taper « derive » ne trouvait rien du tout — « Aucun écran ni item relevé ne
 * correspond à cette recherche » — parce que l'écran s'appelle « Dérive ». Sur
 * un produit dont TOUT le vocabulaire est accentué (Dérive, Sécurité, Espace
 * disque, Mises à jour), exiger l'accent exact revient à demander à
 * l'utilisateur de connaître l'orthographe de ce qu'il cherche avant de pouvoir
 * le chercher.
 *
 * `NFD` sépare la lettre de son signe diacritique, et la plage `U+0300..U+036F`
 * est celle des diacritiques combinatoires — on retire donc l'accent sans
 * toucher aux lettres. Les chemins d'items, eux, n'en portent pas : le repli est
 * sans effet sur eux, ce qui est la bonne raison de l'appliquer partout plutôt
 * que d'y mettre une condition.
 */
function replier(texte) {
  return texte
    .normalize("NFD")
    .replace(/[̀-ͯ]/gu, "")
    .toLowerCase();
}

function filtrer(requete) {
  const q = replier(requete.trim());
  const vus = ecrans().filter(
    (e) => q === "" || replier(e.libelle).includes(q) || replier(e.cli).includes(q)
  );
  const items =
    etat === null
      ? []
      : Object.values(etat.readings)
          .filter(
            (r) => q !== "" && (replier(r.path).includes(q) || replier(r.value).includes(q))
          )
          .slice(0, 40)
          .map((r) => ({ genre: "item", releve: r }));
  return { vus, items };
}

/* Une SECTION de la liste : un `role="group"` qui porte son intitulé.
 *
 * Un `role="listbox"` n'admet que des options et des groupes. Les intertitres
 * étaient posés en enfants directs, ce que le modèle de contenu ARIA ne
 * prévoit pas — le lecteur d'écran se retrouve avec du texte là où il attend
 * une option, et le décompte annoncé (« 3 sur 12 ») cesse d'être fiable.
 *
 * L'intitulé visible est marqué `aria-hidden` : c'est le nom du groupe qui le
 * porte à l'oreille, l'annoncer deux fois n'ajoute rien. */
function sectionDeResultats(titre) {
  const groupe = el("div", "pgroupe");
  groupe.setAttribute("role", "group");
  groupe.setAttribute("aria-label", titre);
  const intitule = el("p", "psec", titre);
  intitule.setAttribute("aria-hidden", "true");
  groupe.append(intitule);
  return groupe;
}

function rendrePalette() {
  const { vus, items } = filtrer(entree.value);
  resultats.replaceChildren();
  const plats = [];

  if (vus.length > 0) {
    const groupe = sectionDeResultats("Écrans");
    for (const e of vus) {
      const bouton = el("button", "pitem");
      bouton.type = "button";
      bouton.setAttribute("role", "option");
      bouton.append(el("span", null, e.libelle), el("span", "cli", e.cli));
      bouton.addEventListener("click", () => {
        allerA(e.vue);
        fermerPalette();
      });
      groupe.append(bouton);
      plats.push(bouton);
    }
    resultats.append(groupe);
  }

  if (items.length > 0) {
    const groupe = sectionDeResultats("Items relevés");
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
      groupe.append(bouton);
      plats.push(bouton);
    }
    resultats.append(groupe);
  }

  if (plats.length === 0) {
    resultats.append(
      el("p", "pvide", "Aucun écran ni item relevé ne correspond à cette recherche.")
    );
  }

  /* LE FOCUS NE QUITTE JAMAIS LE CHAMP : ce sont les flèches qui déplacent la
   * sélection. Ce patron n'existe que si `aria-activedescendant` désigne
   * l'option courante par son `id` — sans lui, un lecteur d'écran n'annonce
   * rien du tout pendant la navigation aux flèches, et les options n'avaient
   * même pas d'`id` à désigner. */
  choix = Math.min(choix, Math.max(0, plats.length - 1));
  for (const [rang, bouton] of plats.entries()) {
    bouton.id = `popt-${rang}`;
    bouton.classList.toggle("sel", rang === choix);
    bouton.setAttribute("aria-selected", String(rang === choix));
  }
  entree.setAttribute("aria-expanded", String(plats.length > 0));
  if (plats.length > 0) {
    entree.setAttribute("aria-activedescendant", plats[choix].id);
  } else {
    entree.removeAttribute("aria-activedescendant");
  }
  return plats;
}

function ouvrirPalette() {
  if (scrim.classList.contains("on")) {
    return;
  }
  /* Mémorisé AVANT que quoi que ce soit bouge : c'est à cet élément-là que le
   * focus doit revenir, et à aucun autre. */
  declencheur =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  scrim.classList.add("on");
  entree.value = "";
  choix = 0;
  rendrePalette();
  /* Le focus part avant que le fond devienne inerte : l'ordre inverse
   * laisserait le focus sur un élément qu'on vient de rendre inaccessible. */
  entree.focus();
  fondsInertes(true);
}

function fermerPalette() {
  if (!scrim.classList.contains("on")) {
    return;
  }
  scrim.classList.remove("on");
  entree.setAttribute("aria-expanded", "false");
  entree.removeAttribute("aria-activedescendant");
  fondsInertes(false);
  if (declencheur !== null && declencheur.isConnected) {
    declencheur.focus();
  }
  declencheur = null;
}

document.getElementById("open-palette").addEventListener("click", ouvrirPalette);

scrim.addEventListener("click", (evenement) => {
  if (evenement.target === scrim) {
    fermerPalette();
  }
});

/* L'enfermement du focus, posé sur la feuille elle-même.
 *
 * `inert` fait déjà le travail là où il existe ; ce gestionnaire est la seconde
 * barrière, et la seule là où il manque. Il ne coûte rien quand `inert` opère :
 * le fond n'est alors plus dans l'ordre de tabulation, et la boucle se referme
 * de toute façon sur les deux mêmes bornes. */
scrim.addEventListener("keydown", (evenement) => {
  if (evenement.key !== "Tab") {
    return;
  }
  const bornes = focusablesDeLaPalette();
  if (bornes.length === 0) {
    return;
  }
  const premier = bornes[0];
  const dernier = bornes[bornes.length - 1];
  if (evenement.shiftKey && document.activeElement === premier) {
    evenement.preventDefault();
    dernier.focus();
  } else if (!evenement.shiftKey && document.activeElement === dernier) {
    evenement.preventDefault();
    premier.focus();
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

"use strict";

/*
 * Coque de `ks-ui`.
 *
 * Ce fichier ne fabrique aucune donnée. Il demande au processus Rust l'état
 * réel de la machine, et affiche le HTML que `ks_cli::rapport::construire`
 * renvoie — le même que celui qu'écrit `ks report`, sans retouche.
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

const corps = document.body;
const ligneEtat = document.getElementById("state-line");
const compteur = document.getElementById("elapsed");
const cadre = document.getElementById("report");
const bouton = document.getElementById("recollect");
const messageEchec = document.getElementById("failure-message");
const detailEchec = document.getElementById("failure-detail");

const FORMAT_DATE = new Intl.DateTimeFormat("fr-FR", {
  dateStyle: "long",
  timeStyle: "short",
});

let minuterie = null;

/* Le compteur affiche le temps RÉELLEMENT écoulé, jamais une progression
 * estimée : tant que les collecteurs n'annoncent pas leur avancement, aucun
 * pourcentage ne serait honnête. */
function demarrerCompteur() {
  arreterCompteur();
  const debut = Date.now();
  compteur.textContent = "Temps écoulé : 0 s";
  minuterie = window.setInterval(() => {
    const secondes = Math.floor((Date.now() - debut) / 1000);
    compteur.textContent = `Temps écoulé : ${secondes} s`;
  }, 1000);
}

function arreterCompteur() {
  if (minuterie !== null) {
    window.clearInterval(minuterie);
    minuterie = null;
  }
}

/* L'horodatage arrive en RFC 3339 — la forme que produit le noyau, et celle
 * qu'attend un script. L'écran, lui, lit une date française. */
function horodatageLisible(rfc3339) {
  const date = new Date(rfc3339);
  return Number.isNaN(date.getTime()) ? rfc3339 : FORMAT_DATE.format(date);
}

function appeler() {
  const pont = window.__TAURI_INTERNALS__;
  if (!pont || typeof pont.invoke !== "function") {
    return Promise.reject(new Error("pont IPC absent"));
  }
  return pont.invoke(COMMANDE, {});
}

async function collecter() {
  corps.dataset.state = "collecting";
  bouton.disabled = true;
  ligneEtat.textContent = "Collecte en cours";
  demarrerCompteur();

  try {
    const etat = await appeler();

    /* `srcdoc` plutôt qu'une injection dans le document : le rapport reste un
     * document séparé, dans un cadre `sandbox` sans aucune permission. Même si
     * l'échappement de `ks_cli::rapport` cédait, rien ne s'exécuterait là. */
    cadre.srcdoc = etat.html;

    ligneEtat.textContent =
      `${etat.itemCount} items · ${etat.machine} · ` +
      `collecte du ${horodatageLisible(etat.collectedAt)}`;
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

    messageEchec.textContent = attendu
      ? echec.message
      : "La coque n'a pas pu joindre le lecteur d'état de Keystone.";
    detailEchec.textContent = attendu
      ? `detail : ${echec.detail}`
      : `detail : ${String(echec)}`;
    ligneEtat.textContent = "Lecture interrompue";
    corps.dataset.state = "failed";
  } finally {
    arreterCompteur();
    bouton.disabled = false;
  }
}

bouton.addEventListener("click", () => {
  void collecter();
});

void collecter();

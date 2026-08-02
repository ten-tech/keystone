//! # `ks-ui` — la coque de bureau de Keystone
//!
//! ## Ce qu'elle affiche, et ce qu'elle n'affichera jamais
//!
//! Elle affiche le **rapport réel de cette machine** : le HTML que produit
//! [`ks_cli::rapport::construire`] à partir d'un [`Inventory::collect_all`]
//! réellement exécuté. Le même code, le même HTML, le même inventaire que
//! `ks report`.
//!
//! Elle n'affiche **jamais** `design/keystone-cockpit.html`. Cette maquette est
//! peuplée de valeurs inventées — un nom de poste qui n'existe pas, un nombre
//! d'items qui n'a jamais été relevé, une posture calculée sur rien. Tant qu'elle
//! reste un fichier dans `design/`, personne ne s'y trompe ; empaquetée en
//! exécutable avec une icône dans la barre des tâches, elle deviendrait un
//! produit qui affiche des chiffres faux, c'est-à-dire exactement le défaut que
//! tout ce dépôt combat. Le test `aucune_valeur_de_maquette_ne_figure_dans_la_coque`
//! en fait une barrière plutôt qu'une intention.
//!
//! ## Non privilégiée, et en lecture seule
//!
//! `ks-ui` tourne avec les droits de l'utilisateur, comme `ks` (SEC-01). Elle
//! **ne s'installe pas en service**, elle n'appelle pas `ks-broker`, et elle ne
//! sait rien écrire : sa seule commande lit l'inventaire et rend une chaîne.
//!
//! Ce qui est désactivé dans `tauri.conf.json`, et pourquoi :
//!
//! * **Aucune capacité déclarée.** Le dossier `capabilities/` n'existe pas :
//!   sans lui, aucune commande de greffon (`fs`, `shell`, `dialog`, `process`,
//!   `http`…) n'est atteignable depuis la page. Seule la commande définie ici
//!   l'est. Aucun greffon n'est d'ailleurs enregistré dans le `Builder`.
//! * **`withGlobalTauri: false`.** `window.__TAURI__` n'est pas injecté ; la
//!   page ne voit que le pont IPC brut, dont elle n'appelle qu'un verbe.
//! * **`assetProtocol.enable: false`.** Le protocole `asset:` sert à lire des
//!   fichiers arbitraires du disque depuis la page. On n'en lit aucun.
//! * **`dragDropEnabled: false`.** Rien ne se dépose dans cette fenêtre : elle
//!   ne prend aucun fichier en entrée.
//! * **`freezePrototype: true`.** Gèle les prototypes JavaScript au démarrage,
//!   ce qui ferme la classe d'attaques par *prototype pollution* sur une page
//!   qui, de toute façon, ne charge rien de tiers.
//!
//! Ce qui reste actif, et pourquoi :
//!
//! * **La CSP** est en `default-src 'none'` : aucune requête réseau ne peut
//!   partir de la page (principe P5). `script-src 'self'` n'autorise que
//!   `app.js`, embarqué dans le binaire. `connect-src ipc: http://ipc.localhost`
//!   est le strict nécessaire au pont IPC de Tauri sous Windows.
//! * **`style-src 'unsafe-inline'`** est la seule concession, et elle est
//!   documentée : le rapport porte sa feuille de style dans un bloc `<style>`,
//!   et un cadre `srcdoc` hérite de la politique de son parent. La barrière
//!   réelle n'est donc pas la CSP mais l'attribut `sandbox` du cadre, posé sans
//!   aucune permission : origine opaque, aucun script exécutable, aucun accès au
//!   pont IPC. Un `<style>` hostile qui franchirait l'échappement de
//!   `ks_cli::rapport` n'y obtiendrait rien de plus qu'un rapport laid.
//!
//! ## L'attente
//!
//! La collecte demande environ trois secondes. La commande s'exécute donc hors
//! du fil de l'interface, qui affiche pendant ce temps ce qui se passe et le
//! temps réellement écoulé — pas une barre de progression, qu'aucun collecteur
//! ne sait alimenter aujourd'hui.

#![forbid(unsafe_code)]
// La console n'a rien à dire en production, et une fenêtre noire derrière
// l'application aurait tout l'air d'une anomalie. En dev elle reste, parce que
// c'est là que les messages du runtime servent.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chrono::Utc;
use ks_cli::rapport;
use ks_collectors::Inventory;
use serde::Serialize;

/// L'état de la machine, tel que la coque doit l'afficher.
///
/// Ces structures ne sont que sérialisées, jamais relues : il n'y a donc pas de
/// champ inconnu à accepter ou à refuser. Le renommage est explicite parce que
/// la page le lit en camelCase et qu'un renommage implicite est un contrat non
/// écrit.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    /// Le rapport HTML autonome, tel que `ks report` l'écrirait.
    html: String,
    /// Le nom relevé sur cette machine, pas un nom d'exemple.
    machine: String,
    /// Le nombre d'items réellement observés.
    item_count: usize,
    /// L'instant de la collecte, en RFC 3339.
    collected_at: String,
}

/// Un échec de collecte, tel que l'écran doit le rendre.
///
/// Deux champs, délibérément : `message` se lit, `detail` se copie. Une seule
/// chaîne concaténée obligerait à montrer le code technique à qui n'en a que
/// faire, ou à le perdre pour qui en a besoin.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Failure {
    /// Ce qui s'est passé, ce que ça implique, ce qu'on peut faire.
    message: String,
    /// Le détail technique, destiné à être recopié dans un rapport.
    detail: String,
}

/// Lit l'état de la machine. Bloquant, environ trois secondes.
fn collect_blocking() -> Snapshot {
    let inventory = Inventory::collect_all();
    let machine = rapport::nom_machine(&inventory.items);
    let collected_at = Utc::now().to_rfc3339();
    let html = rapport::construire(&inventory.items, &machine, &collected_at);

    Snapshot {
        html,
        machine,
        item_count: inventory.items.len(),
        collected_at,
    }
}

/// La seule commande exposée à la page.
///
/// Elle lit, elle ne prend aucun argument, et elle ne peut donc rien recevoir de
/// la page qui influerait sur ce qu'elle regarde.
///
/// La collecte part sur un fil dédié : posée sur le fil de l'interface, elle
/// figerait la fenêtre pendant toute sa durée — la barre de titre comprise, ce
/// que Windows finit par signaler comme une application qui ne répond plus. Le
/// `spawn_blocking` a une seconde vertu : une panique d'un collecteur revient
/// ici en `Err` au lieu d'emporter la fenêtre, et l'écran peut la dire. C'est
/// aussi pourquoi le profil de release de ce workspace ne porte pas
/// `panic = "abort"`.
#[tauri::command]
async fn collect_state() -> Result<Snapshot, Failure> {
    tauri::async_runtime::spawn_blocking(collect_blocking)
        .await
        .map_err(|e| Failure {
            message: "La lecture de l'état de la machine s'est interrompue avant la fin. \
                      L'écran ne montre donc rien : ce qui est affiché serait partiel, et \
                      un inventaire partiel présenté comme complet induit en erreur. \
                      Aucune écriture n'a eu lieu ; relancer la collecte reprend la lecture \
                      depuis le début."
                .to_owned(),
            detail: e.to_string(),
        })
}

fn main() {
    tauri::Builder::default()
        // Aucun greffon n'est enregistré, et c'est la position par défaut : un
        // greffon ajouté ici est une surface ajoutée sur un poste de travail.
        .invoke_handler(tauri::generate_handler![collect_state])
        .run(tauri::generate_context!())
        // `expect` dans `main` : à ce stade il n'y a pas de fenêtre pour dire
        // quoi que ce soit, et rien à annuler.
        .expect("le runtime Tauri n'a pas pu démarrer");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La page, telle qu'elle est embarquée dans le binaire.
    const INDEX: &str = include_str!("../web/index.html");
    const SCRIPT: &str = include_str!("../web/app.js");
    const STYLE: &str = include_str!("../web/app.css");
    const CONFIG: &str = include_str!("../tauri.conf.json");

    #[test]
    fn aucune_valeur_de_maquette_ne_figure_dans_la_coque() {
        // La raison d'être de cette application : afficher le rapport réel, et
        // jamais `design/keystone-cockpit.html`. La maquette est peuplée de
        // valeurs inventées ; empaquetée en exécutable, elle deviendrait un
        // produit qui affiche des chiffres faux.
        //
        // Ce test échoue le jour où quelqu'un recopie un extrait de la maquette
        // dans la coque « pour voir le rendu », et l'y oublie.
        for inventee in ["WKS-ORION-04", "ORION", "keystone-cockpit"] {
            for (nom, source) in [
                ("index.html", INDEX),
                ("app.js", SCRIPT),
                ("app.css", STYLE),
            ] {
                assert!(
                    !source.contains(inventee),
                    "« {inventee} » vient de la maquette et figure dans {nom} : \
                     la coque afficherait une valeur que personne n'a relevée"
                );
            }
        }
    }

    #[test]
    fn la_coque_ne_contacte_aucun_serveur() {
        // Principe P5, « Local, point final ». La règle vaut pour les polices
        // autant que pour les scripts : une police distante raconte au serveur
        // qui l'héberge qu'on a ouvert l'application, quand, et depuis où.
        for interdit in ["http://", "https://", "//fonts.", "cdn.", "@import"] {
            for (nom, source) in [
                ("index.html", INDEX),
                ("app.js", SCRIPT),
                ("app.css", STYLE),
            ] {
                assert!(
                    !source.contains(interdit),
                    "{nom} contient « {interdit} », donc une dépendance externe"
                );
            }
        }
    }

    #[test]
    fn le_rapport_saffiche_dans_un_cadre_sans_aucune_permission() {
        // `sandbox` sans valeur, c'est-à-dire sans un seul `allow-*` : origine
        // opaque, aucun script exécutable, aucun accès au pont IPC. C'est la
        // barrière réelle derrière l'échappement de `ks_cli::rapport`. Poser
        // `allow-scripts` ou `allow-same-origin` la retirerait, et un `<script>`
        // qui aurait franchi l'échappement s'exécuterait dans la coque.
        assert!(
            INDEX.contains("sandbox=\"\""),
            "le cadre du rapport doit être `sandbox` sans permission"
        );
        assert!(
            !INDEX.contains("allow-scripts") && !INDEX.contains("allow-same-origin"),
            "aucune permission ne se rend au cadre du rapport"
        );
        assert!(
            SCRIPT.contains("srcdoc"),
            "le rapport reste un document séparé, il ne s'injecte pas dans la page"
        );
    }

    #[test]
    fn la_page_nexecute_aucun_script_en_ligne() {
        // La CSP est en `script-src 'self'` : un script en ligne serait bloqué
        // au chargement, donc l'interface resterait figée sur son état
        // d'attente. Mieux vaut le voir ici qu'à l'écran.
        assert!(
            !INDEX.contains("<script>"),
            "aucun script en ligne : la CSP les refuse"
        );
        assert!(
            INDEX.contains("<script src=\"app.js\">"),
            "le script se charge depuis un fichier embarqué"
        );
    }

    #[test]
    fn la_configuration_refuse_tout_par_defaut() {
        assert!(
            CONFIG.contains("default-src 'none'"),
            "la CSP part de zéro, elle n'ouvre que ce qu'il faut"
        );
        assert!(
            CONFIG.contains("\"withGlobalTauri\": false"),
            "l'API globale de Tauri n'est pas injectée dans la page"
        );
        assert!(
            CONFIG.contains("\"enable\": false"),
            "le protocole `asset:` reste fermé : la page ne lit aucun fichier"
        );
        assert!(
            CONFIG.contains("\"dragDropEnabled\": false"),
            "la fenêtre ne prend aucun fichier en entrée"
        );
    }

    #[test]
    fn aucune_capacite_nest_accordee_a_la_page() {
        // Sans dossier `capabilities/`, aucune commande de greffon n'est
        // atteignable depuis la page : ni `fs`, ni `shell`, ni `process`. Le
        // jour où quelqu'un en crée un, ce test le force à venir écrire ici
        // pourquoi — et à passer par une ADR.
        let capacites = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        assert!(
            !capacites.exists(),
            "un dossier `capabilities/` est apparu : chaque permission accordée \
             à la page est une surface, et celle-ci n'en réclame aucune"
        );
    }

    #[test]
    fn letat_expose_a_lecran_porte_les_chiffres_de_la_machine() {
        // La coque n'affiche que ce que le noyau a relevé : le nombre d'items,
        // le nom de la machine et l'instant de la collecte viennent tous les
        // trois de la même structure, produite par le même passage.
        let etat = Snapshot {
            html: "<!doctype html>".to_owned(),
            machine: "POSTE-DE-TEST".to_owned(),
            item_count: 115,
            collected_at: "2026-08-02T12:00:00Z".to_owned(),
        };
        let json = serde_json::to_string(&etat).expect("une structure sans type exotique");
        assert!(json.contains("\"itemCount\":115"));
        assert!(json.contains("\"collectedAt\""));
        assert!(json.contains("\"machine\":\"POSTE-DE-TEST\""));
    }

    #[test]
    fn un_echec_separe_ce_qui_se_lit_de_ce_qui_se_copie() {
        // Jamais de code d'erreur nu en guise de phrase : le technique vit dans
        // `detail`, et l'écran le range à part.
        let echec = Failure {
            message: "La lecture s'est interrompue avant la fin.".to_owned(),
            detail: "task panicked".to_owned(),
        };
        let json = serde_json::to_string(&echec).expect("une structure sans type exotique");
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"detail\""));
    }
}

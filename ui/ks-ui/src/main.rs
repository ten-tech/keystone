//! # `ks-ui` — la coque de bureau de Keystone
//!
//! ## Ce qu'elle affiche, et la règle qui gouverne tout le reste
//!
//! Elle affiche **l'état réel de cette machine** : les items que produit
//! [`Inventory::collect_all`], réellement exécuté sur ce poste, mis en scène
//! dans le poste de pilotage décrit par `docs/02-BRIEF-DESIGN.md`.
//!
//! **Aucun nombre affiché n'est inventé.** Chaque valeur de l'écran vient d'un
//! item relevé, ou l'écran dit qu'elle n'est pas collectée — avec la raison, et
//! ce qui la rendrait disponible. Jamais de valeur par défaut, jamais de zéro
//! qui ressemble à une mesure. Le module [`cockpit`] porte cette règle dans le
//! typage, et non dans la vigilance ; ses tests l'éprouvent par falsification.
//!
//! Trois éléments du poste de pilotage ne sont pas calculables aujourd'hui, et
//! l'écran le dit plutôt que de les remplir :
//!
//! * **l'anneau de posture** — une posture composite se calcule contre une
//!   référence, et aucun état désiré n'est chargé (P6, D2-02) ;
//! * **la dérive** — sans état désiré, il n'y a pas zéro écart, il n'y a pas de
//!   comparaison ; c'est exactement ce que
//!   [`ks_core::item::Verdict::Incomparable`] existe pour empêcher ;
//! * **les mises à jour** — le domaine D3 n'est pas collecté.
//!
//! Ce qui est réel se remplit vraiment : la posture de sécurité et ses items
//! illisibles comptés et nommés, l'espace par volume, les distributions WSL,
//! l'inventaire logiciel avec son taux d'attribution, la machine et son système.
//!
//! Le rapport tabulaire n'a pas disparu : il est devenu un écran du cockpit,
//! puisqu'il est déjà juste et déjà testé. Il reste rendu dans un cadre
//! `sandbox` sans aucune permission.
//!
//! `design/keystone-cockpit.html` **reste la maquette de conception**, et la
//! coque n'en recopie aucune valeur : le test
//! `aucune_valeur_de_maquette_ne_figure_dans_la_coque` en fait une barrière
//! plutôt qu'une intention.
//!
//! ## Non privilégiée, et en lecture seule
//!
//! `ks-ui` tourne avec les droits de l'utilisateur, comme `ks` (SEC-01). Elle
//! **ne s'installe pas en service**, elle n'appelle pas `ks-broker`, et elle ne
//! sait rien écrire : sa seule commande lit l'inventaire et rend une structure.
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

mod cockpit;

use chrono::Utc;
use cockpit::Cockpit;
use ks_cli::rapport;
use ks_collectors::Inventory;
use serde::Serialize;

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
fn collect_blocking() -> Cockpit {
    let inventory = Inventory::collect_all();
    let machine = rapport::nom_machine(&inventory.items);
    let collected_at = Utc::now().to_rfc3339();
    let html = rapport::construire(&inventory.items, &machine, &collected_at);

    Cockpit::build(&inventory.items, &machine, &collected_at, html)
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
async fn collect_state() -> Result<Cockpit, Failure> {
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

    /// Les trois fichiers du frontend, nommés pour les messages d'échec.
    const FRONTEND: [(&str, &str); 3] = [
        ("index.html", INDEX),
        ("app.js", SCRIPT),
        ("app.css", STYLE),
    ];

    #[test]
    fn aucune_valeur_de_maquette_ne_figure_dans_la_coque() {
        // La raison d'être de cette application : afficher l'état réel, et
        // jamais les valeurs de `design/keystone-cockpit.html`. La maquette
        // garde le balisage et le style — c'est son rôle — mais elle est
        // peuplée de chiffres que personne n'a relevés : un nom de poste qui
        // n'existe pas, une posture calculée sur rien, des distributions
        // imaginaires, des gigaoctets récupérables sur un disque qu'aucun
        // collecteur ne sait encore attribuer.
        //
        // Ce test échoue le jour où quelqu'un recopie un extrait de la maquette
        // dans la coque « pour voir le rendu », et l'y oublie.
        for inventee in [
            "WKS-ORION-04",
            "ORION",
            "keystone-cockpit",
            "TB4-BUREAU",
            "ubuntu-dev",
            "debian-ci",
            "win11-test",
            "cuda-smoke",
            "docker-hello",
            "build-ref",
            "vpn-up",
            "KB5062001",
            "KB5061999",
            "570.12",
            "566.36",
            "1,86 To",
            "47,2 Go",
            "41,2 Go",
            "312 items",
            "Windows.old",
            "node_modules",
        ] {
            for (nom, source) in FRONTEND {
                assert!(
                    !source.contains(inventee),
                    "« {inventee} » vient de la maquette et figure dans {nom} : \
                     la coque afficherait une valeur que personne n'a relevée"
                );
            }
        }
    }

    #[test]
    fn aucun_chiffre_de_mesure_nest_ecrit_dans_le_balisage() {
        // La barrière centrale de cet écran, et celle qu'il fallait poser avant
        // d'écrire une ligne de cockpit : une mesure ne s'écrit jamais dans le
        // balisage, elle arrive du pont.
        //
        // Chaque emplacement de valeur porte `data-mesure`, et **doit être vide
        // dans le fichier** : ce que la page affiche vient du relevé, ou reste
        // « non relevé ». Éprouvée par falsification — écrire une valeur entre
        // les balises d'un de ces emplacements fait échouer ce test.
        let mut emplacements = 0;
        let mut reste = INDEX;
        while let Some(depart) = reste.find("data-mesure=\"") {
            emplacements += 1;
            let apres = &reste[depart..];
            let fin_balise = apres
                .find('>')
                .unwrap_or_else(|| panic!("balise `data-mesure` non fermée : {}", &apres[..60]));
            let contenu = &apres[fin_balise + 1..];
            assert!(
                contenu.starts_with("</"),
                "un emplacement de mesure porte déjà une valeur dans le balisage : « {} »",
                &contenu[..contenu.len().min(40)]
            );
            reste = &apres[fin_balise..];
        }
        assert!(
            emplacements >= 10,
            "seulement {emplacements} emplacements de mesure : la barrière ne barre plus rien"
        );

        // Et le second visage du même défaut : une valeur écrite en dur dans
        // une phrase, avec son unité. C'est la forme sous laquelle la maquette
        // porte ses chiffres (« 72 % », « 47,2 Go », « 312 items »).
        for (nom, source) in [("index.html", INDEX), ("app.js", SCRIPT)] {
            for unite in [
                "%", "Go", "To", "Gio", "Tio", "Mio", "Kio", "items", "item(s)", "écarts", "écart",
            ] {
                let mut reste = source;
                while let Some(pos) = reste.find(unite) {
                    let avant = reste[..pos].trim_end_matches([' ', '\u{a0}']);
                    assert!(
                        !avant.ends_with(|c: char| c.is_ascii_digit()),
                        "{nom} porte une mesure écrite à la main juste avant « {unite} » : \
                         « {} »",
                        &reste[pos.saturating_sub(20)..(pos + unite.len()).min(reste.len())]
                    );
                    reste = &reste[pos + unite.len()..];
                }
            }
        }
    }

    #[test]
    fn chaque_entree_du_rail_designe_un_ecran_existant() {
        // Un rail qui pointe vers une vue absente laisse un écran blanc, sans
        // rien dire. Le contrôle vaut dans les deux sens : une vue sans entrée
        // de rail est un écran qu'on ne peut plus atteindre.
        let vues: Vec<&str> = motifs(INDEX, "data-view=\"");
        let sections: Vec<&str> = motifs(INDEX, "id=\"view-");
        assert!(vues.len() >= 8, "seulement {} entrées de rail", vues.len());
        for vue in &vues {
            assert!(
                sections.contains(vue),
                "l'entrée de rail « {vue} » ne désigne aucune section"
            );
        }
        for section in &sections {
            assert!(
                vues.contains(section),
                "la vue « {section} » n'est atteignable depuis aucune entrée de rail"
            );
        }
    }

    #[test]
    fn aucun_bouton_de_navigation_ne_peut_rester_sans_nom_accessible() {
        // LA BARRIÈRE D'ACCESSIBILITÉ CENTRALE DU RAIL.
        //
        // Le rail replié masque `.lbl` et `.tag-muet` en `display: none`. Un
        // descendant en `display: none` est EXCLU du calcul du nom accessible
        // (spécification accname, étape 2F) : sans `aria-label`, cinq boutons
        // sur neuf deviennent des boutons sans nom dès qu'on replie le rail,
        // et les quatre autres n'annoncent qu'un nombre nu.
        //
        // Le défaut est invisible à l'écran — le rail déplié se lit très bien —
        // et invisible au code, l'attribut manquant ne cassant rien. D'où ce
        // test. Éprouvé par falsification : retirer un seul `aria-label`
        // d'index.html le fait échouer, en nommant le bouton fautif.
        let mut boutons = 0;
        for balise in balises_ouvrantes(INDEX, "<button") {
            if !balise.contains("class=\"nav\"") {
                continue;
            }
            boutons += 1;
            let vue = valeur_dattribut(balise, "data-view").unwrap_or("sans vue");
            let nom = valeur_dattribut(balise, "aria-label").unwrap_or_else(|| {
                panic!(
                    "l'entrée de rail « {vue} » n'a pas d'`aria-label` : repliée, \
                     elle devient un bouton sans nom (accname, étape 2F)"
                )
            });
            assert!(
                !nom.trim().is_empty(),
                "l'entrée de rail « {vue} » porte un `aria-label` vide, \
                 ce qui ne vaut pas mieux qu'aucun"
            );
        }
        assert!(
            boutons >= 9,
            "seulement {boutons} entrées de rail examinées : la barrière ne barre plus rien"
        );

        // Et le nom se RECALCULE avec le décompte : « Sécurité, 38 items
        // relevés » plutôt que « 38 ». L'attribut statique est la position de
        // repli tant que la collecte n'a pas abouti, pas la version finale.
        assert!(
            SCRIPT.contains("bouton.setAttribute(\"aria-label\", nom)"),
            "app.js ne repose plus le nom accessible des entrées de rail"
        );
        assert!(
            SCRIPT.matches("nommerLeRail();").count() >= 2,
            "le nom accessible du rail ne se recalcule plus quand le décompte change"
        );
        assert!(
            INDEX.contains("data-unite=\""),
            "les décomptes du rail n'ont plus d'unité : un nombre nu n'apprend rien"
        );
    }

    #[test]
    fn la_palette_tient_la_promesse_de_aria_modal() {
        // `aria-modal="true"` annonce que rien d'autre n'est atteignable. La
        // palette le déclarait sans le tenir : en tabulant depuis le dernier
        // résultat, le focus atteignait les boutons du rail, sous un voile
        // translucide — donc invisible pour qui navigue au clavier. Et la
        // fermeture ne rendait jamais le focus à l'élément déclencheur : il
        // retombait au début du document.
        assert!(
            INDEX.contains("aria-modal=\"true\""),
            "la palette ne se déclare plus modale"
        );
        assert!(
            SCRIPT.contains("declencheur.focus()"),
            "le focus ne revient plus à l'élément qui a ouvert la palette"
        );
        assert!(
            SCRIPT.contains("fondsInertes(true)") && SCRIPT.contains("fondsInertes(false)"),
            "le fond ne devient plus inerte pendant que la feuille est ouverte"
        );
        // `inert` est VÉRIFIÉ, pas supposé : la WebView2 du poste n'est pas le
        // navigateur de développement. Là où il manque, le gestionnaire de
        // `Tab` referme la boucle, et il reste posé dans les deux cas.
        assert!(
            SCRIPT.contains("\"inert\" in HTMLElement.prototype"),
            "le soutien de `inert` est supposé au lieu d'être vérifié"
        );
        assert!(
            SCRIPT.contains("evenement.key !== \"Tab\""),
            "plus de repli d'enfermement du focus là où `inert` manque"
        );
    }

    #[test]
    fn aucun_anneau_de_focus_nest_retire_sans_remplacant() {
        // « Jamais de retrait d'anneau sans alternative clavier » : la règle
        // valait pour le fichier entier, et elle était tenue partout SAUF sur
        // le champ de la palette, c'est-à-dire l'élément qui reçoit le focus à
        // l'ouverture. Le remplaçant se dessine vers l'intérieur, la feuille
        // étant en `overflow: hidden`.
        //
        // Le contrôle porte sur les DÉCLARATIONS, pas sur le texte du fichier :
        // sa première version refusait aussi le commentaire qui explique
        // pourquoi la déclaration a disparu, ce qui aurait forcé à taire la
        // raison pour satisfaire la barrière.
        for ligne in STYLE.lines() {
            let regle = ligne.trim_start();
            assert!(
                !regle.starts_with("outline: none") && !regle.starts_with("outline:none"),
                "app.css retire un anneau de focus : « {regle} » — \
                 aucun retrait sans remplaçant explicite"
            );
        }
        assert!(
            STYLE.contains(".palette input:focus-visible"),
            "le champ de la palette n'a plus d'anneau de focus qui lui soit propre"
        );
    }

    #[test]
    fn la_palette_designe_loption_courante_au_lecteur_decran() {
        // Le focus reste sur le champ pendant que les flèches déplacent une
        // sélection parmi des `role="option"`. Ce patron s'appelle `combobox`,
        // et il n'existe que si `aria-activedescendant` désigne l'option
        // courante par son `id` — sans quoi rien n'est annoncé pendant la
        // navigation aux flèches. Les options n'avaient pas d'`id`.
        for attendu in [
            "role=\"combobox\"",
            "aria-controls=\"presults\"",
            "aria-expanded=\"false\"",
        ] {
            assert!(
                INDEX.contains(attendu),
                "la palette ne déclare plus « {attendu} » : le patron combobox est incomplet"
            );
        }
        // La POSE de l'attribut, pas sa simple mention : la première version de
        // ce contrôle cherchait « aria-activedescendant » n'importe où dans le
        // fichier, et la falsification l'a traversée sans bruit — le retrait
        // (`removeAttribute`) et le commentaire suffisaient à le satisfaire.
        // Une barrière qu'on n'a pas essayé de franchir ne prouve rien.
        assert!(
            SCRIPT.contains("entree.setAttribute(\"aria-activedescendant\", plats[choix].id)"),
            "l'option courante n'est plus désignée au lecteur d'écran"
        );
        assert!(
            SCRIPT.contains("bouton.id = `popt-${rang}`"),
            "les options n'ont plus d'`id` à désigner"
        );
        // Un `role="listbox"` n'admet que des options et des groupes : les
        // intertitres de section vivent DANS un groupe, jamais en enfants
        // directs de la liste.
        assert!(
            SCRIPT.contains("groupe.setAttribute(\"role\", \"group\")"),
            "les sections de résultats ne sont plus des groupes"
        );
        assert!(
            !SCRIPT.contains("resultats.append(el(\"p\", \"psec\""),
            "un intertitre est reposé en enfant direct du `listbox`, \
             ce que le modèle de contenu ARIA n'admet pas"
        );
    }

    #[test]
    fn le_compteur_dattente_nannonce_pas_chaque_seconde() {
        // Correct au sens strict, et pourtant contraire à la thèse du produit :
        // une région live rafraîchie toutes les secondes produit un flux verbal
        // ininterrompu. L'affichage bat la seconde ; l'annonce parle par
        // paliers. Deux éléments, une seule mesure.
        let visible = balise_portant(INDEX, "id=\"elapsed\"");
        assert!(
            !visible.contains("aria-live") && !visible.contains("role=\"status\""),
            "le compteur visible est redevenu une région live : « {visible} »"
        );
        let annonce = balise_portant(INDEX, "id=\"elapsed-annonce\"");
        assert!(
            annonce.contains("role=\"status\"") && annonce.contains("aria-live=\"polite\""),
            "plus de région annoncée séparée : « {annonce} »"
        );
        assert!(
            SCRIPT.contains("PALIERS_ANNONCE"),
            "l'annonce d'attente n'est plus bornée à des paliers"
        );
    }

    #[test]
    fn la_jauge_garde_sa_frontiere_en_contraste_force() {
        // `forced-color-adjust: none` sauve le remplissage et emporte le
        // conteneur : sa piste est un blanc à 7 % qui ne survit à aucune des
        // deux palettes système. Sans contour, une jauge à faible taux devient
        // indiscernable d'une jauge absente.
        let bloc = STYLE
            .split_once("forced-colors: active")
            .expect("le bloc de contraste forcé a disparu")
            .1;
        let regle = bloc
            .split_once(".meter {")
            .expect("aucune règle `.meter` dans le bloc de contraste forcé")
            .1;
        let corps = &regle[..regle.find('}').unwrap_or(regle.len())];
        assert!(
            corps.contains("border: 1px solid CanvasText"),
            "la jauge n'a plus de contour en contraste forcé : « {corps} »"
        );
    }

    #[test]
    fn aucune_taille_de_texte_ne_sort_de_lechelle_typographique() {
        // Le brief §3.1 déclare huit paliers ; la coque en rendait quatorze,
        // dont un 9,5 px sur la pastille du rail replié — le plus petit texte
        // de toute l'interface. Une taille écrite en dur est une taille que
        // personne ne compare à l'échelle.
        for (jeton, valeur) in [
            ("--fs-display", "34px"),
            ("--fs-h1", "22px"),
            ("--fs-h2", "16px"),
            ("--fs-body", "14px"),
            ("--fs-sm", "13px"),
            ("--fs-mono", "12.5px"),
            ("--fs-label", "11px"),
        ] {
            assert!(
                STYLE.contains(&format!("{jeton}: {valeur};")),
                "le palier {jeton} ne vaut plus {valeur} : \
                 l'échelle de la coque a divergé de design/tokens.css"
            );
        }
        for ligne in STYLE.lines() {
            let regle = ligne.trim_start();
            if !regle.starts_with("font:") && !regle.starts_with("font-size:") {
                continue;
            }
            let mut reste = regle;
            while let Some(pos) = reste.find("px") {
                assert!(
                    !reste[..pos].ends_with(|c: char| c.is_ascii_digit()),
                    "une taille de texte est écrite en dur, hors de l'échelle : « {regle} »"
                );
                reste = &reste[pos + 2..];
            }
        }
    }

    #[test]
    fn le_bouton_de_depliage_touche_ce_quil_commande() {
        // Le bouton « Pourquoi » flottait sous l'anneau, centré, séparé par
        // 280 px de cercle vide du panneau qu'il ouvre. Rien ne le rattachait à
        // ce qu'il commande, et `aria-controls` ne rattache rien à l'œil : il
        // désigne, il ne rapproche pas.
        //
        // La règle posée ici est structurelle et se vérifie : l'élément
        // commandé est le PREMIER élément qui suit le bouton dans le document.
        // Elle vaut pour tout bouton de dépliage, présent ou futur.
        //
        // Éprouvée par falsification : glisser un paragraphe entre le bouton et
        // son panneau fait échouer ce test, en nommant le panneau fautif.
        let mut boutons = 0;
        for balise in balises_ouvrantes(INDEX, "<button") {
            let Some(commande) = valeur_dattribut(balise, "aria-controls") else {
                continue;
            };
            // Le rail est commandé depuis la barre supérieure, à l'autre bout
            // du document : c'est une commande de trame, pas un dépliage de
            // contenu, et l'adjacence n'aurait aucun sens pour lui.
            if commande == "rail" {
                continue;
            }
            boutons += 1;

            let apres_bouton = INDEX
                .split_once(balise)
                .unwrap_or_else(|| panic!("balise introuvable : {balise}"))
                .1;
            let ferme = apres_bouton
                .find("</button>")
                .unwrap_or_else(|| panic!("bouton « {commande} » non fermé"));
            let suite = apres_bouton[ferme + "</button>".len()..].trim_start();

            assert!(
                suite.starts_with('<'),
                "du texte sépare le bouton de « {commande} » : « {} »",
                &suite[..suite.len().min(60)]
            );
            let ouvrante = &suite[..suite.find('>').unwrap_or(suite.len())];
            assert!(
                valeur_dattribut(ouvrante, "id") == Some(commande),
                "le bouton n'est pas suivi du bloc « {commande} » qu'il commande, \
                 mais de « {ouvrante} » : un dépliage se lit par la proximité, \
                 `aria-controls` désigne sans rapprocher"
            );
        }
        assert!(
            boutons >= 1,
            "aucun bouton de dépliage examiné : la barrière ne barre plus rien"
        );

        // ET LE PANNEAU DOIT POUVOIR SE REPLIER. `[hidden] { display: none }`
        // vient de la feuille de l'agent utilisateur : toute déclaration
        // `display` posée dans une règle de classe l'emporte sur elle, quel que
        // soit l'ordre. Le défaut s'est produit — passer `.why` en `display:
        // grid` pour lui donner deux colonnes a suffi à le rendre indépliable,
        // alors que `hidden`, `aria-expanded` et le libellé du bouton disaient
        // tous les trois le contraire. Il ne se voyait qu'à l'écran.
        let mut replaces = 0;
        for balise in balises_ouvrantes(INDEX, "<div") {
            if !balise.contains(" hidden") {
                continue;
            }
            replaces += 1;
            for classe in valeur_dattribut(balise, "class")
                .unwrap_or_default()
                .split_whitespace()
            {
                let regle = format!(".{classe} {{");
                let pose_display = STYLE
                    .split(&regle)
                    .skip(1)
                    .any(|bloc| bloc[..bloc.find('}').unwrap_or(0)].contains("display:"));
                assert!(
                    !pose_display || STYLE.contains(&format!(".{classe}[hidden]")),
                    "« .{classe} » impose un `display` sans rendre `[hidden]` : \
                     le bloc reste affiché quoi qu'en disent `hidden`, \
                     `aria-expanded` et le libellé du bouton"
                );
            }
        }
        assert!(
            replaces >= 1,
            "aucun bloc repliable examiné : la barrière ne barre plus rien"
        );
    }

    #[test]
    fn la_vue_densemble_ne_fige_ni_sa_mesure_ni_ses_colonnes() {
        // Le défaut de mise en page que cet écran portait, sous ses deux
        // formes, et les deux se lisent dans la feuille de style.
        //
        // 1. UNE MESURE DE 45 CARACTÈRES. Le panneau d'explication était borné
        //    à 46ch — la borne basse de la lisibilité — et rendu dans une
        //    colonne de 390 px devant une moitié d'écran vide. La règle du
        //    dossier de design est 45 à 75 caractères ; on vise le haut de la
        //    bande, parce que la place existe.
        //
        //    La borne est écrite ici en `ch`, l'unité de la déclaration, et non
        //    en caractères : `ch` vaut la largeur du zéro, plus large que
        //    l'avance moyenne d'une police proportionnelle. Mesuré dans la
        //    WebView2 du poste, 62ch rend de 66 à 74 caractères par ligne. La
        //    bande admise ci-dessous est donc celle des `ch` qui retombent dans
        //    la bande des caractères — un test ne peut pas mesurer un rendu.
        //
        // 2. DEUX COLONNES FIGÉES. La rangée de tuiles ne se recomposait qu'une
        //    fois, à 1180 px, et sur la largeur de la FENÊTRE — alors que le
        //    rail replié rend 156 px que la fenêtre ne voit pas passer. Les
        //    paliers portent désormais sur la largeur réellement disponible.
        //
        // Éprouvée par falsification : ramener `.why` à 46ch, ou retirer le
        // palier à quatre colonnes, fait échouer ce test.
        let regle = STYLE
            .split_once(".why {")
            .expect("plus de règle `.why` : le panneau d'explication a disparu")
            .1;
        let corps = &regle[..regle.find('}').unwrap_or(regle.len())];
        let mesure: u32 = corps
            .split_once("max-width:")
            .and_then(|(_, reste)| reste.trim_start().split_once("ch"))
            .and_then(|(n, _)| n.trim().parse().ok())
            .unwrap_or_else(|| panic!("`.why` n'a plus de mesure en `ch` : « {corps} »"));
        assert!(
            (56..=70).contains(&mesure),
            "l'explication est bornée à {mesure}ch, hors de la bande de 56 à 70 \
             qui rend 60 à 75 caractères par ligne : trop étroite, elle rejoue \
             la colonne de 390 px ; trop large, elle cesse de se suivre à l'œil"
        );

        assert!(
            STYLE.contains("container-type: inline-size"),
            "la colonne de contenu n'est plus un conteneur de requête : \
             les paliers retomberaient sur la largeur de la fenêtre, \
             qui ignore si le rail est replié"
        );
        let paliers = STYLE.matches("@container contenu").count();
        assert!(
            paliers >= 2,
            "seulement {paliers} palier(s) de recomposition : la vue d'ensemble \
             doit passer de quatre tuiles à deux, puis à une seule"
        );
        for colonnes in ["repeat(2, minmax(0, 1fr))", "repeat(4, minmax(0, 1fr))"] {
            assert!(
                STYLE.contains(colonnes),
                "le palier « {colonnes} » a disparu de la rangée de tuiles"
            );
        }
    }

    /// La balise ouvrante qui porte un motif, du `<` qui la commence au `>`.
    fn balise_portant<'a>(source: &'a str, motif: &str) -> &'a str {
        let pos = source
            .find(motif)
            .unwrap_or_else(|| panic!("aucune balise ne porte « {motif} »"));
        let debut = source[..pos]
            .rfind('<')
            .expect("un attribut hors de toute balise");
        let fin = source[debut..]
            .find('>')
            .unwrap_or_else(|| panic!("balise non fermée autour de « {motif} »"));
        &source[debut..debut + fin + 1]
    }

    /// Toutes les balises ouvrantes d'un nom donné, attributs compris.
    fn balises_ouvrantes<'a>(source: &'a str, ouverture: &str) -> Vec<&'a str> {
        let mut sortie = Vec::new();
        let mut reste = source;
        while let Some(pos) = reste.find(ouverture) {
            let apres = &reste[pos..];
            let fin = apres
                .find('>')
                .unwrap_or_else(|| panic!("balise non fermée : {}", &apres[..60.min(apres.len())]));
            sortie.push(&apres[..=fin]);
            reste = &apres[fin..];
        }
        sortie
    }

    /// La valeur d'un attribut dans une balise ouvrante, s'il y figure.
    fn valeur_dattribut<'a>(balise: &'a str, attribut: &str) -> Option<&'a str> {
        let motif = format!("{attribut}=\"");
        let pos = balise.find(&motif)? + motif.len();
        let fin = balise[pos..].find('"')?;
        Some(&balise[pos..pos + fin])
    }

    /// Extrait les valeurs d'attribut qui suivent chaque occurrence d'un motif.
    fn motifs<'a>(source: &'a str, motif: &str) -> Vec<&'a str> {
        let mut sortie = Vec::new();
        let mut reste = source;
        while let Some(pos) = reste.find(motif) {
            let apres = &reste[pos + motif.len()..];
            if let Some(fin) = apres.find('"') {
                let valeur = &apres[..fin];
                if !sortie.contains(&valeur) {
                    sortie.push(valeur);
                }
            }
            reste = apres;
        }
        sortie
    }

    #[test]
    fn linterface_reste_lisible_sans_mouvement_et_en_contraste_force() {
        // Deux exigences non négociables du dossier de design, et deux que
        // l'on perd sans s'en apercevoir : une animation qui ignore
        // `prefers-reduced-motion`, et une interface dont la structure repose
        // sur des surfaces qui deviennent toutes identiques en contraste forcé.
        assert!(
            STYLE.contains("prefers-reduced-motion"),
            "aucune prise en compte du mouvement réduit"
        );
        assert!(
            STYLE.contains("forced-colors: active"),
            "aucune prise en compte du contraste imposé par le système"
        );
        // Rien ne clignote jamais dans Keystone : la seule animation infinie
        // tolérée est la rotation de l'anneau d'attente, à vitesse constante.
        assert_eq!(
            STYLE.matches("infinite").count(),
            1,
            "une animation infinie de plus est apparue : rien ne clignote dans Keystone"
        );
        // Les chiffres de télémétrie s'alignent, sans quoi la mise en page
        // saute quand une valeur change.
        assert!(
            STYLE.contains("tabular-nums"),
            "les chiffres de télémétrie doivent être tabulaires"
        );
    }

    #[test]
    fn aucun_emoji_dans_linterface_produit() {
        // Les états passent par icône vectorielle ET libellé. C'est un interdit
        // contractuel du brief, pas une préférence.
        for (nom, source) in FRONTEND {
            for c in source.chars() {
                let point = u32::from(c);
                let emoji = (0x0001_F300..=0x0001_FAFF).contains(&point)
                    || (0x2600..=0x27BF).contains(&point)
                    || point == 0xFE0F;
                assert!(!emoji, "{nom} contient un émoji : « {c} »");
            }
        }
    }

    #[test]
    fn la_coque_ne_contacte_aucun_serveur() {
        // Principe P5, « Local, point final ». La règle vaut pour les polices
        // autant que pour les scripts : une police distante raconte au serveur
        // qui l'héberge qu'on a ouvert l'application, quand, et depuis où.
        //
        // C'est aussi pourquoi le grain de la maquette n'a pas été repris : il
        // s'obtient par un SVG en `data:` dont l'espace de noms XML contient
        // une URL, et une exception dans un garde-fou est une exception qui
        // reste.
        for interdit in ["http://", "https://", "//fonts.", "cdn.", "@import"] {
            for (nom, source) in FRONTEND {
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
        // Et aucun gestionnaire d'événement en attribut : `onclick=` est un
        // script en ligne déguisé, que la CSP refuse tout autant.
        assert!(
            !INDEX.contains("onclick="),
            "les gestionnaires s'attachent depuis app.js, jamais en attribut"
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
        let etat = Cockpit::build(
            &[],
            "POSTE-DE-TEST",
            "2026-08-03T12:00:00Z",
            "<!doctype html>".to_owned(),
        );
        let json = serde_json::to_string(&etat).expect("une structure sans type exotique");
        assert!(json.contains("\"itemCount\":0"));
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

//! Rapport HTML autonome (Phase 0.5).
//!
//! ## Autonome veut dire autonome
//!
//! **Aucune ressource réseau.** Ni CDN, ni police distante, ni image externe,
//! ni requête d'aucune sorte. Le principe P5 — « Local, point final » — ne
//! souffre pas d'exception pour un rapport : un fichier qui appelle un serveur
//! pour s'afficher raconte à ce serveur qu'on l'a ouvert, quand, et depuis où.
//!
//! Les polices sont donc celles du système, déclarées dans la même pile de repli
//! que `design/tokens.css`. Embarquer Inter en base64 doublerait le poids du
//! fichier pour un gain d'apparence, et la Phase 0 n'a pas ce budget.
//!
//! ## Ce que le rapport ne fait pas
//!
//! Pas un graphique. Ce n'est pas une privation : la règle du projet veut qu'un
//! graphique porte toujours son équivalent en tableau, et à ce volume de données
//! le tableau **est** la meilleure représentation. Un histogramme de 107 items
//! n'apprendrait rien de plus et coûterait une couche à vérifier.
//!
//! Rien ne clignote, aucun sens n'est porté par la couleur seule : chaque état
//! porte un libellé lisible, et la couleur ne fait que le redoubler.

use ks_core::{Domain, Item, ItemValue};

/// Échappe ce qui part dans le document.
///
/// Les valeurs viennent du registre et de noms de fichiers : autant dire d'un
/// input non fiable. Une application dont le nom contiendrait `<script>`
/// injecterait du code dans le rapport, qui est destiné à être ouvert et
/// parfois transmis.
fn echapper(brut: &str) -> String {
    let mut sortie = String::with_capacity(brut.len());
    for c in brut.chars() {
        match c {
            '&' => sortie.push_str("&amp;"),
            '<' => sortie.push_str("&lt;"),
            '>' => sortie.push_str("&gt;"),
            '"' => sortie.push_str("&quot;"),
            '\'' => sortie.push_str("&#39;"),
            _ => sortie.push(c),
        }
    }
    sortie
}

/// Nom lisible d'un domaine, pour les titres de section.
const fn titre_domaine(d: Domain) -> &'static str {
    match d {
        Domain::Inventory => "Inventaire",
        Domain::Configuration => "Configuration",
        Domain::Updates => "Mises à jour",
        Domain::Space => "Espace",
        Domain::Security => "Posture de sécurité",
        Domain::Backup => "Sauvegarde",
        Domain::Identity => "Identité",
        Domain::Profiles => "Profils",
        Domain::Virtualization => "Virtualisation",
        Domain::Peripherals => "Périphériques",
        Domain::DevEnv => "Environnement de développement",
    }
}

/// Ordre d'affichage des domaines. La posture d'abord : c'est ce qu'on vient
/// vérifier.
const ORDRE: &[Domain] = &[
    Domain::Security,
    Domain::Inventory,
    Domain::Virtualization,
    Domain::Space,
    Domain::Configuration,
    Domain::Updates,
    Domain::Backup,
    Domain::Identity,
    Domain::Profiles,
    Domain::Peripherals,
    Domain::DevEnv,
];

/// Classe CSS d'une valeur, pour la teinter **en plus** de son libellé.
///
/// La couleur ne porte jamais seule : « illisible » et « absent » restent
/// écrits en toutes lettres dans la cellule. Un lecteur daltonien lit la même
/// information qu'un autre, et un rapport imprimé en noir et blanc aussi.
const fn classe_valeur(v: &ItemValue) -> &'static str {
    match v {
        ItemValue::Illisible { .. } => "v-illisible",
        ItemValue::Absent => "v-absent",
        ItemValue::Bool(true) => "v-vital",
        _ => "v-normale",
    }
}

/// Nom de la machine, tel qu'il doit figurer en tête du rapport.
///
/// Il se lit dans l'inventaire lui-même, jamais par un appel séparé : un
/// rapport qui titrerait sur une machine et listerait les items d'une autre
/// serait indétectable à la lecture.
///
/// Quand l'item n'a pas pu être relevé, le rapport se titre « poste » plutôt
/// que d'inventer un nom — un inventaire qui invente est pire qu'un inventaire
/// incomplet.
///
/// Cette fonction a deux appelants — la commande `ks report` et la coque
/// `ks-ui` — et c'est la raison de son existence : la même recherche écrite
/// deux fois finit par diverger sur le nom du chemin.
#[must_use]
pub fn nom_machine(items: &[Item]) -> String {
    items
        .iter()
        .find(|i| i.path == "inventory.host.name")
        .map_or_else(|| "poste".to_owned(), |i| i.observed.to_string())
}

/// Rend un horodatage RFC 3339 lisible par un humain, à l'heure locale.
///
/// Le rapport affichait la forme brute — `2026-08-02T22:33:00.021721100+00:00`.
/// Deux défauts dans une seule chaîne. D'abord la voix du projet veut que le
/// détail technique vive dans un champ à part, jamais à l'écran. Ensuite, et
/// c'est le pire, la coque de bureau affiche au-dessus la même date en heure
/// locale : l'utilisateur lisait « 00:33 » et « 22:33 » à deux centimètres
/// d'écart, pour le même instant. Une contradiction apparente sur un outil dont
/// toute la thèse est qu'on peut le croire.
///
/// La valeur machine n'est pas perdue pour autant : elle reste dans
/// l'attribut `datetime` de l'élément `<time>`, qui est fait pour ça.
///
/// Une entrée que l'on n'arrive pas à analyser est rendue telle quelle : mieux
/// vaut afficher une date brute qu'inventer une date lisible.
#[must_use]
pub fn horodatage_lisible(rfc3339: &str) -> String {
    /// Les mois en français. `chrono` ne les localise pas sans dépendance
    /// supplémentaire, et douze chaînes ne justifient pas une crate de plus.
    const MOIS: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];

    let Ok(instant) = chrono::DateTime::parse_from_rfc3339(rfc3339) else {
        return rfc3339.to_owned();
    };
    let local = instant.with_timezone(&chrono::Local);
    let mois = MOIS
        .get((chrono::Datelike::month(&local) as usize).saturating_sub(1))
        .copied()
        .unwrap_or("");
    format!(
        "{} {mois} {} à {:02}:{:02}",
        chrono::Datelike::day(&local),
        chrono::Datelike::year(&local),
        chrono::Timelike::hour(&local),
        chrono::Timelike::minute(&local),
    )
}

/// Construit le rapport complet.
///
/// `horodatage` est passé en paramètre plutôt que lu ici : une fonction qui
/// appelle l'horloge ne se teste pas deux fois de la même façon.
#[must_use]
pub fn construire(items: &[Item], machine: &str, horodatage: &str) -> String {
    let mut corps = String::new();

    for domaine in ORDRE {
        let du_domaine: Vec<&Item> = items.iter().filter(|i| i.domain == *domaine).collect();
        if du_domaine.is_empty() {
            continue;
        }

        // Le conteneur porte le défilement ET le focus : un tableau qui déborde
        // doit pouvoir être parcouru au clavier, ce qu'un `div` sans `tabindex`
        // interdit. `role="group"` et l'étiquette évitent qu'un lecteur d'écran
        // annonce un conteneur anonyme.
        corps.push_str(&format!(
            "<section aria-labelledby=\"d-{0:?}\">\n\
             <h2 id=\"d-{0:?}\">{1} <span class=\"compte\">{2} item(s)</span></h2>\n\
             <div class=\"cadre-table\" role=\"group\" aria-labelledby=\"d-{0:?}\" tabindex=\"0\">\n\
             <table>\n<caption class=\"sr\">Items observés du domaine {1}</caption>\n\
             <thead><tr><th scope=\"col\">Item</th><th scope=\"col\">Valeur constatée</th>\
             <th scope=\"col\">À quoi ça sert</th></tr></thead>\n<tbody>\n",
            domaine,
            titre_domaine(*domaine),
            du_domaine.len()
        ));

        for item in du_domaine {
            corps.push_str(&format!(
                "<tr><th scope=\"row\"><code>{}</code></th>\
                 <td class=\"{}\">{}</td><td class=\"but\">{}</td></tr>\n",
                echapper(&item.path),
                classe_valeur(&item.observed),
                echapper(&item.observed.to_string()),
                echapper(&item.purpose)
            ));
        }
        corps.push_str("</tbody>\n</table>\n</div>\n</section>\n");
    }

    let illisibles = items.iter().filter(|i| !i.observed.est_constat()).count();
    let avertissement = if illisibles == 0 {
        String::new()
    } else {
        format!(
            "<p class=\"note\">{illisibles} item(s) n'ont pas pu être lus : la clé \
             correspondante exige des privilèges élevés. Ils sont marqués « illisible » \
             et non « absent » — l'outil ne sait pas ce qu'ils valent, et ne le suppose \
             pas.</p>\n"
        )
    };

    format!(
        "<!doctype html>\n<html lang=\"fr\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Keystone — état de {machine}</title>\n\
         <style>\n{STYLE}</style>\n</head>\n<body>\n\
         <header class=\"bandeau\">\n<div class=\"bandeau-inner\">\n\
         <p class=\"marque\">Keystone</p>\n\
         <p class=\"sous\">{machine} · <time datetime=\"{horodatage}\">{lisible}</time></p>\n</div>\n</header>\n\
         <main>\n<p class=\"chapeau\">{total} items relevés, en lecture seule. \
         Aucune écriture système n'a eu lieu pendant ce scan.</p>\n\
         {avertissement}{corps}\
         <footer><p>Rapport autonome : il ne contacte aucun serveur pour s'afficher.</p>\n\
         <p>Avant de le transmettre : il porte le nom de cette machine et la liste \
         des logiciels installés avec leur version exacte. Aucun secret, aucun \
         chemin d'exclusion — mais de quoi savoir ce qui est à jour et ce qui \
         ne l'est pas.</p>\
         </footer>\n</main>\n</body>\n</html>\n",
        machine = echapper(machine),
        horodatage = echapper(horodatage),
        lisible = echapper(&horodatage_lisible(horodatage)),
        total = items.len(),
    )
}

/// Feuille de style, reprise de `design/tokens.css`.
///
/// Les valeurs ne sont pas choisies ici : elles sont **recopiées** des tokens,
/// qui sont vérifiés par calcul dans `design/palette-validation.md`. Substituer
/// une couleur à l'œil dans ce fichier casserait cette vérification sans que
/// personne le voie.
const STYLE: &str = r#":root {
  --surface-0: #0B0F14; --surface-1: #131A22; --surface-2: #1B242F;
  --hairline: rgba(255,255,255,.07); --hairline-strong: rgba(255,255,255,.12);
  --ink-1: #E6EDF3; --ink-2: #9FB0C0; --ink-3: #798C9D;
  --vital: #4FD1C5; --attention: #E8A33D;
  --sp-2: 8px; --sp-3: 12px; --sp-4: 16px; --sp-5: 24px; --sp-6: 32px; --sp-7: 48px;
  --r-card: 10px;
  --font-sans: 'Inter','Segoe UI',system-ui,sans-serif;
  --font-mono: 'JetBrains Mono',ui-monospace,monospace;
}
* { box-sizing: border-box; }
body {
  margin: 0; background: var(--surface-0); color: var(--ink-1);
  font: 14px/1.55 var(--font-sans);
  -webkit-font-smoothing: antialiased;
}
/* Le bandeau cadre la page depuis les bords ; le corps reste une colonne
   centrée, plus étroite. Les deux ne partagent jamais le même conteneur. */
.bandeau {
  width: 100%; border-bottom: 1px solid var(--hairline-strong);
  background: var(--surface-1);
}
.bandeau-inner {
  display: flex; align-items: baseline; justify-content: space-between;
  gap: var(--sp-4); padding: var(--sp-4) var(--sp-6);
}
.marque { margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -.01em; }
.sous { margin: 0; color: var(--ink-2); font-size: 13px; }
main { max-width: 1080px; margin: 0 auto; padding: var(--sp-7) var(--sp-5) var(--sp-7); }
.chapeau { color: var(--ink-2); margin: 0 0 var(--sp-5); max-width: 68ch; }
/* Pas de filet d'accent a gauche : c'est le tic de la boite d'alerte
   generique, il ne dit rien que le texte ne dise deja, et il depense une
   couleur d'etat pour decorer. A la place, la notation du dessin technique
   — hachure a 45 degres, pas de 7 px, trait de 1,15 px, les valeurs exactes
   du motif de la marque — qui signifie « zone non levee ». Monochrome, donc lisible a
   l'impression. En contraste force elle disparait — background-image y calcule
   a none — et le bloc est alors marque par un trait tirete, voir le bloc
   @media plus bas. Le bloc est RECESSE : ce
   qu'on n'a pas pu lire s'enfonce, il ne saute pas aux yeux. */
.note {
  margin: 0 0 var(--sp-6); padding: var(--sp-3) var(--sp-4) var(--sp-3) var(--sp-5);
  background: var(--surface-0);
  box-shadow: inset 0 1px 0 rgba(0,0,0,.28);
  background-image: repeating-linear-gradient(45deg,
    var(--hairline) 0 1.15px, transparent 1.15px 7px);
  background-repeat: no-repeat; background-size: 7px 100%;
  border-radius: var(--r-card); color: var(--ink-2);
  max-width: 68ch;
}
section { margin-bottom: var(--sp-7); }
h2 {
  font-size: 16px; font-weight: 600; margin: 0 0 var(--sp-3);
  padding-bottom: var(--sp-2); border-bottom: 1px solid var(--hairline);
}
.compte { color: var(--ink-3); font-weight: 400; font-size: 13px; }
/* Le tableau défile dans SON CONTENEUR, pas lui-même.
   `display:block` posé sur <table> retirait la sémantique de tableau de l'arbre
   d'accessibilité : le lecteur d'écran cessait de proposer la navigation par
   ligne et par colonne, et lisait chaque cellule comme du texte plat. Le
   commentaire d'origine — « le tableau défile dans son propre conteneur » —
   décrivait donc un effet visuel juste et une conséquence non dite. */
.cadre-table {
  overflow-x: auto; background: var(--surface-1); border-radius: var(--r-card);
}
.cadre-table:focus-visible { outline: 2px solid var(--vital); outline-offset: 2px; }
table { width: 100%; border-collapse: collapse; }
thead th {
  text-align: left; font-size: 11px; letter-spacing: .09em; text-transform: uppercase;
  color: var(--ink-3); font-weight: 600; padding: var(--sp-3) var(--sp-4);
  border-bottom: 1px solid var(--hairline-strong); white-space: nowrap;
}
tbody th, tbody td {
  padding: var(--sp-3) var(--sp-4); border-bottom: 1px solid var(--hairline);
  text-align: left; font-weight: 400; vertical-align: top;
}
tbody tr:last-child th, tbody tr:last-child td { border-bottom: 0; }
tbody tr:hover { background: var(--surface-2); }
code { font: 12.5px/1.6 var(--font-mono); color: var(--ink-2); }
.but { color: var(--ink-3); max-width: 46ch; }
.v-normale { color: var(--ink-1); }
.v-vital { color: var(--vital); }
.v-absent { color: var(--ink-3); }
.v-illisible { color: var(--attention); }
footer { margin-top: var(--sp-7); padding-top: var(--sp-4);
  border-top: 1px solid var(--hairline); color: var(--ink-3); font-size: 13px; }
.sr {
  position: absolute; width: 1px; height: 1px; overflow: hidden;
  clip: rect(0 0 0 0); white-space: nowrap;
}
/* Contraste imposé par le système : on rend la main aux couleurs de l'OS
   plutôt que d'imposer les nôtres. */
@media (forced-colors: active) {
  /* La hachure NE SURVIT PAS au contraste forcé, et le commentaire d'à côté
     affirmait le contraire — vérifié au texte normatif de CSS Color Adjust
     Level 1 : « background-image computes to none unless the original value
     contains a url() function », et « box-shadow […] compute to none ». Fond,
     ombre et hachure disparaissent donc tous les trois, et le bloc « non
     calculable » devenait indiscernable d'un bloc mesuré.

     Ce qui survit avec certitude, c'est une BORDURE : le spec en force la
     couleur, il ne la supprime pas. Un trait tireté appartient à la même
     famille de notation que la hachure — en dessin technique, il dit
     « provisoire, non levé ». Le sens est donc conservé sans dépendre d'une
     propriété dont on n'est pas sûr qu'elle soit rendue.

     `forced-color-adjust: none` aurait peut-être suffi à conserver la hachure.
     Peut-être : la lecture du spec est ambiguë sur ce point précis. On ne
     construit pas une garantie d'accessibilité sur un peut-être. */
  .note { border-left: 3px dashed CanvasText; }

  .v-vital, .v-illisible, .v-absent { color: CanvasText; }
  table, .note { border: 1px solid CanvasText; }
}
/* Le rapport est fait pour être transmis et imprimé. Encore faut-il qu'il
   s'imprime.
   Les classes de valeur portent une couleur explicite, qui l'emporte sur le
   `color: #000` du corps : sans les repeindre, `.v-normale` — la valeur par
   défaut de la plupart des items — sortait à **1,18:1 sur papier blanc**,
   c'est-à-dire invisible. Recalculé, pas estimé.
   Même chose pour les filets, en blanc translucide : sur fond blanc, le
   tableau perdait toutes ses séparations. */
@media print {
  body { background: #fff; color: #111; }
  .bandeau, .cadre-table, .note { background: transparent; }
  .v-normale, .v-vital, .v-illisible, .v-absent { color: #111; }
  .but, .compte, code, .sous, thead th, footer { color: #333; }
  table, tbody th, tbody td, thead th, h2, footer, .note {
    border-color: #ccc;
  }
  /* A l'impression, ni fond ni ombre ne survivent : la hachure est redessinee
     en noir, et c'est elle seule qui marque le bloc. */
  .note {
    background: #fff;
    box-shadow: none;
    background-image: repeating-linear-gradient(45deg,
      #444 0 1.15px, transparent 1.15px 7px);
    background-repeat: no-repeat; background-size: 7px 100%;
  }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use ks_core::Provenance;

    fn item(chemin: &str, domaine: Domain, valeur: ItemValue) -> Item {
        Item {
            path: chemin.to_owned(),
            domain: domaine,
            desired: None,
            observed: valeur,
            observed_at: chrono::Utc::now(),
            provenance: Provenance::Observed,
            purpose: "Une finalité.".to_owned(),
            risk: "Un risque.".to_owned(),
            reference: None,
        }
    }

    #[test]
    fn le_rapport_ne_contacte_aucun_serveur() {
        // Principe P5, « Local, point final ». Un rapport qui appelle un serveur
        // pour s'afficher raconte à ce serveur qu'on l'a ouvert, quand, et depuis
        // où. La règle vaut pour les polices autant que pour les scripts.
        let html = construire(
            &[item("security.x", Domain::Security, ItemValue::Bool(true))],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        for interdit in [
            "http://", "https://", "//fonts.", "cdn.", "<script", "@import", "src=",
        ] {
            assert!(
                !html.contains(interdit),
                "le rapport contient « {interdit} », donc une dépendance externe"
            );
        }
    }

    #[test]
    fn une_valeur_hostile_est_echappee() {
        // Les noms viennent du registre : un input non fiable. Une application
        // nommée « <script> » injecterait du code dans un fichier destiné à être
        // ouvert, et parfois transmis.
        let html = construire(
            &[item(
                "inventory.software[x].name",
                Domain::Inventory,
                ItemValue::Text("<script>alert('x')</script>".to_owned()),
            )],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn un_item_illisible_est_signale_sans_etre_confondu_avec_une_absence() {
        let html = construire(
            &[
                item(
                    "security.a",
                    Domain::Security,
                    ItemValue::illisible("accès refusé"),
                ),
                item("security.b", Domain::Security, ItemValue::Absent),
            ],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        assert!(html.contains("illisible"), "l'aveu doit être visible");
        assert!(
            html.contains("1 item(s) n'ont pas pu être lus"),
            "et compté à part de l'absence"
        );
    }

    #[test]
    fn la_couleur_ne_porte_jamais_seule() {
        // Aucun sens ne doit dépendre de la couleur : chaque état s'écrit aussi
        // en toutes lettres dans la cellule. Un lecteur daltonien, ou un rapport
        // imprimé en noir et blanc, lit la même information.
        let html = construire(
            &[item(
                "security.a",
                Domain::Security,
                ItemValue::illisible("accès refusé"),
            )],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        assert!(html.contains("forced-colors"), "contraste système respecté");
        // Le libellé, pas seulement la classe.
        let sans_classes = html.replace("v-illisible", "");
        assert!(sans_classes.contains("illisible — accès refusé"));
    }

    #[test]
    fn le_rapport_reste_lisible_une_fois_imprime() {
        // Le rapport est fait pour être transmis et imprimé, et il s'imprimait
        // en blanc sur blanc : les classes de valeur portent une couleur
        // explicite qui l'emporte sur celle du corps, et `.v-normale` — le cas
        // par défaut de la plupart des items — sortait à 1,18:1 sur papier.
        //
        // Un audit l'a mesuré ; ce test l'empêche de revenir.
        let impression = STYLE
            .split("@media print")
            .nth(1)
            .expect("une feuille d'impression doit exister");

        for classe in [".v-normale", ".v-vital", ".v-illisible", ".v-absent"] {
            assert!(
                impression.contains(classe),
                "« {classe} » n'est pas repeinte pour l'impression : elle sortirait \
                 dans sa couleur d'écran, illisible sur papier blanc"
            );
        }
        assert!(
            impression.contains("border-color"),
            "les filets sont en blanc translucide : sans repeinte, le tableau perd \
             toutes ses séparations sur fond blanc"
        );
    }

    #[test]
    fn le_tableau_garde_sa_semantique_de_tableau() {
        // `display: block` sur un `<table>` fait défiler le tableau, et retire
        // sa sémantique de l'arbre d'accessibilité : le lecteur d'écran cesse de
        // proposer la navigation par ligne et par colonne. Le défilement
        // appartient donc au conteneur, jamais au tableau.
        assert!(
            !STYLE.contains("table {\n  width: 100%; border-collapse: collapse; display: block"),
            "le défilement ne se pose pas sur le <table> lui-même"
        );
        assert!(
            STYLE.contains(".cadre-table"),
            "un conteneur doit porter le défilement"
        );

        let html = construire(
            &[item("security.a", Domain::Security, ItemValue::Bool(true))],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        assert!(html.contains("role=\"group\""), "le conteneur s'annonce");
        assert!(
            html.contains("class=\"cadre-table\" role=\"group\" aria-labelledby"),
            "et il porte une étiquette plutôt que d'être anonyme"
        );
        assert!(
            html.contains("tabindex=\"0\""),
            "un tableau qui déborde doit se parcourir au clavier"
        );
    }

    #[test]
    fn le_nom_de_machine_se_lit_dans_linventaire_et_ne_sinvente_pas() {
        // Deux clients lisent ce nom : `ks report` et la coque `ks-ui`. Ils
        // doivent titrer sur la même machine, sinon l'un des deux ment.
        let releve = item(
            "inventory.host.name",
            Domain::Inventory,
            ItemValue::Text("PC-DE-BUREAU".to_owned()),
        );
        assert_eq!(nom_machine(std::slice::from_ref(&releve)), "PC-DE-BUREAU");

        // À défaut de relevé, un repli neutre — jamais un nom fabriqué.
        assert_eq!(nom_machine(&[]), "poste");
    }

    #[test]
    fn un_domaine_vide_ne_produit_pas_de_section() {
        let html = construire(
            &[item("security.a", Domain::Security, ItemValue::Bool(true))],
            "poste",
            "2026-08-02T12:00:00Z",
        );
        assert!(html.contains("Posture de sécurité"));
        assert!(
            !html.contains("Sauvegarde"),
            "une section sans item n'a rien à dire"
        );
    }
    #[test]
    fn lhorodatage_saffiche_en_clair_et_reste_lisible_par_une_machine() {
        // Le rapport montrait « 2026-08-02T22:33:00.021721100+00:00 », pendant
        // que la coque affichait le même instant en « 00:33 » deux centimètres
        // plus haut. Deux nombres différents pour un seul moment, sur un outil
        // dont toute la thèse est qu'on peut le croire.
        let html = construire(&[], "PC", "2026-08-02T22:33:00.021721100+00:00");

        assert!(
            html.contains("<time datetime=\"2026-08-02T22:33:00.021721100+00:00\">"),
            "la valeur machine doit survivre dans l'attribut datetime"
        );
        assert!(
            !html.contains(">2026-08-02T22:33:00.021721100+00:00<"),
            "elle ne doit plus être le texte affiché"
        );
        assert!(
            html.contains("août 2026 à "),
            "le texte affiché est en clair : {html}"
        );
    }

    #[test]
    fn un_horodatage_illisible_saffiche_tel_quel_plutot_que_dinventer() {
        assert_eq!(horodatage_lisible("pas une date"), "pas une date");
    }
    /// Ce qui porte un sens à l'écran doit le porter aussi en contraste forcé.
    ///
    /// Le bloc « non lu » se distingue par une hachure et une récession. Or le
    /// spec CSS Color Adjust est explicite : en contraste forcé,
    /// `background-image` calcule à `none` et `box-shadow` aussi. Fond, ombre
    /// et hachure disparaissent donc **tous les trois**, et le bloc devenait
    /// indiscernable d'un paragraphe ordinaire — alors qu'un commentaire du
    /// fichier affirmait qu'il survivait.
    ///
    /// Ce test exige qu'une marque **qui survit** soit déclarée. Une bordure en
    /// survit : le spec en force la couleur, il ne la supprime pas.
    #[test]
    fn le_bloc_non_lu_reste_distinct_en_contraste_force() {
        // Le bloc s'arrête à SON accolade fermante, et pas à la fin de la
        // feuille. La première version employait `split_once`, donc lisait
        // tout ce qui suivait le marqueur — y compris la feuille d'impression,
        // qui parle elle aussi de `.note`. Le test passait alors même quand la
        // règle de contraste forcé était retirée : il ne contrôlait rien.
        let bloc = {
            let apres = STYLE
                .split_once("@media (forced-colors: active)")
                .map(|(_, apres)| apres)
                .expect("le rapport doit porter un bloc de contraste forcé");
            let debut = apres.find('{').expect("bloc mal formé") + 1;
            let mut profondeur = 1_i32;
            let mut fin = debut;
            for (i, c) in apres[debut..].char_indices() {
                match c {
                    '{' => profondeur += 1,
                    '}' => {
                        profondeur -= 1;
                        if profondeur == 0 {
                            fin = debut + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            assert!(fin > debut, "accolade fermante introuvable");
            &apres[debut..fin]
        };

        // Une bordure ORDINAIRE ne suffit pas : le bloc en recevait déjà une
        // par `table, .note { border: 1px solid CanvasText }`, identique à
        // celle d'un tableau. Il était donc bien visible, et indiscernable
        // d'autre chose — ce qui est le défaut, pas sa correction. On exige
        // une marque PROPRE au bloc, et le tireté est ce que le dessin
        // technique emploie pour « provisoire, non levé ».
        //
        // La première version de ce test cherchait « .note », « CanvasText »
        // et « border » n'importe où dans le bloc : les trois y étaient déjà,
        // donc il passait même la règle retirée. Il ne contrôlait rien.
        // Les commentaires sortent du champ AVANT toute analyse. Celui qui
        // explique cette règle NOMME `background-image` et `box-shadow` pour
        // dire pourquoi ils ne conviennent pas : sans ce retrait, le contrôle
        // échouait sur sa propre justification. C'est le troisième garde-fou
        // de ce dépôt à trébucher ainsi — le filtre de vocabulaire doit citer
        // les mots qu'il proscrit, la barrière winget doit nommer les drapeaux
        // qu'elle interdit. Un contrôle se lit toujours après avoir retiré ce
        // qui l'explique.
        let sans_commentaires = {
            let mut sortie = String::with_capacity(bloc.len());
            let mut reste = bloc;
            while let Some(debut) = reste.find("/*") {
                sortie.push_str(&reste[..debut]);
                match reste[debut..].find("*/") {
                    Some(fin) => reste = &reste[debut + fin + 2..],
                    None => {
                        reste = "";
                        break;
                    }
                }
            }
            sortie.push_str(reste);
            sortie
        };

        let regle_du_bloc = sans_commentaires
            .split(';')
            .find(|r| r.contains(".note") && r.contains("dashed"))
            .map(|r| r.trim().to_owned());

        assert!(
            regle_du_bloc.is_some(),
            "aucune marque propre au bloc « non lu » en contraste forcé : il y              porte la même bordure qu'un tableau, donc plus rien ne le              distingue. Bloc reçu : {bloc}"
        );
        let regle = regle_du_bloc.unwrap_or_default();
        assert!(
            regle.contains("CanvasText"),
            "la marque doit employer une couleur système, seule à survivre : {regle}"
        );
        assert!(
            !regle.contains("background-image") && !regle.contains("box-shadow"),
            "background-image et box-shadow calculent tous deux à `none` en              contraste forcé : ils ne peuvent pas porter la marque. {regle}"
        );
    }
}

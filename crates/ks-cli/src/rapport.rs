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

        corps.push_str(&format!(
            "<section aria-labelledby=\"d-{0:?}\">\n\
             <h2 id=\"d-{0:?}\">{1} <span class=\"compte\">{2} item(s)</span></h2>\n\
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
        corps.push_str("</tbody>\n</table>\n</section>\n");
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
         <p class=\"sous\">{machine} · {horodatage}</p>\n</div>\n</header>\n\
         <main>\n<p class=\"chapeau\">{total} items relevés, en lecture seule. \
         Aucune écriture système n'a eu lieu pendant ce scan.</p>\n\
         {avertissement}{corps}\
         <footer><p>Rapport autonome : il ne contacte aucun serveur pour s'afficher.</p>\
         </footer>\n</main>\n</body>\n</html>\n",
        machine = echapper(machine),
        horodatage = echapper(horodatage),
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
  --font-mono: 'JetBrains Mono',ui-monospace,'Cascadia Mono',monospace;
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
.note {
  margin: 0 0 var(--sp-6); padding: var(--sp-3) var(--sp-4);
  border-left: 3px solid var(--attention); background: var(--surface-1);
  border-radius: 0 var(--r-card) var(--r-card) 0; color: var(--ink-2);
  max-width: 68ch;
}
section { margin-bottom: var(--sp-7); }
h2 {
  font-size: 16px; font-weight: 600; margin: 0 0 var(--sp-3);
  padding-bottom: var(--sp-2); border-bottom: 1px solid var(--hairline);
}
.compte { color: var(--ink-3); font-weight: 400; font-size: 13px; }
/* Le tableau défile dans son propre conteneur : le corps de page ne défile
   jamais horizontalement, même sur un écran étroit. */
table {
  width: 100%; border-collapse: collapse; display: block; overflow-x: auto;
  background: var(--surface-1); border-radius: var(--r-card);
}
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
  .v-vital, .v-illisible, .v-absent { color: CanvasText; }
  table, .note { border: 1px solid CanvasText; }
}
@media print {
  body { background: #fff; color: #000; }
  .bandeau, table { background: transparent; }
  .but, .compte, code { color: #333; }
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
}

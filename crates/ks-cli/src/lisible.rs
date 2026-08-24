//! Rendre lisible ce que la machine a mesuré, sans toucher au relevé.
//!
//! Un item porte des **octets**, parce qu'un octet est ce que la machine a
//! mesuré et que la Phase 1 comparera des octets. Mais `56043241472` à l'écran
//! ne dit rien à personne, et le principe P6 refuse ce qu'on ne peut pas
//! comprendre. La conversion appartient donc à l'affichage, jamais au relevé.
//!
//! **La même règle vaut pour les jetons** (ADR-0015). Un item qui sort d'une
//! table de codes porte `active-sans-verrou-uefi`, parce que c'est ce qui se
//! compare d'un scan à l'autre et ce que l'utilisateur écrira dans
//! `workstation.yaml` ; l'écran, lui, affiche « activée, sans verrou UEFI ».
//! Le vocabulaire vit dans `ks_collectors::jetons`, le libellé vit ici, et
//! nulle part ailleurs.
//!
//! Ce module vit dans la bibliothèque, et non dans le binaire `ks`, pour la
//! même raison que [`crate::rapport`] : la coque `ks-ui` affiche les mêmes
//! valeurs que la CLI. Deux formateurs écrits séparément finissent par afficher
//! deux tailles différentes pour le même disque, et le jour où ça arrive, on ne
//! sait plus lequel croire.

use ks_core::{Item, ItemValue};

/// Le libellé français de chaque jeton du vocabulaire fermé.
///
/// **Cette table est le pendant exact de `ks_collectors::jetons`**, et le test
/// `chaque_jeton_porte_son_libelle_francais` exige qu'aucun jeton n'en soit
/// absent : un jeton sans libellé s'afficherait brut à l'écran, ce que le
/// principe P6 refuse.
///
/// Elle contient volontairement deux entrées qui se traduisent pareil —
/// `eteint` pour VBS, `eteinte` pour l'intégrité du code. Ce sont deux
/// vocabulaires distincts, portés par deux items distincts ; les fondre
/// donnerait un jeton unique dont le sens dépendrait de l'item qui le porte.
const LIBELLES: &[(&str, &str)] = &[
    // Type de démarrage d'un service Windows.
    ("demarrage-noyau", "au démarrage du noyau"),
    ("demarrage-systeme", "au démarrage du système"),
    ("automatique", "automatique"),
    ("manuel", "manuel"),
    ("desactive", "désactivé"),
    // Protection dont le verrou UEFI est optionnel — Credential Guard.
    ("inactive", "désactivée"),
    ("active-verrou-uefi", "activée, verrouillée par UEFI"),
    ("active-sans-verrou-uefi", "activée, sans verrou UEFI"),
    // État de la protection LSA, son verrou mis à part (ADR-0015, point 4).
    ("active", "activée"),
    // Sécurité basée sur la virtualisation, telle qu'elle tourne.
    ("eteint", "éteinte"),
    (
        "configure-non-demarre",
        "configurée, mais pas en cours d'exécution",
    ),
    ("en-execution", "en cours d'exécution"),
    ("arrete", "à l'arrêt"),
    // Application de la stratégie d'intégrité du code.
    ("eteinte", "éteinte"),
    ("audit", "audit — journalise sans bloquer"),
    ("imposee", "imposée"),
    // Ce que le matériel sait faire.
    ("disponible-sur-ce-materiel", "disponible sur ce matériel"),
    ("absente-de-ce-materiel", "absente de ce matériel"),
    // Complétude de l'attribution de l'inventaire logiciel.
    ("complete", "complète"),
    ("partielle", "partielle"),
    // Disponibilité d'un mécanisme d'instantané (ADR-0021). Le libellé nomme la
    // portée de la réponse : elle vaut pour CETTE machine, et pour elle seule.
    ("filet-disponible", "disponible sur cette machine"),
    ("filet-indisponible", "indisponible sur cette machine"),
];

/// Préfixe des jetons de code hors table.
///
/// Recopié plutôt qu'importé : la CLI lit un jeton **déjà écrit** — dans un
/// relevé du jour, ou demain dans un magasin d'observations vieux d'une
/// semaine. Le test `le_prefixe_des_codes_inconnus_est_celui_des_collecteurs`
/// interdit les deux écritures de diverger.
const PREFIXE_CODE_INCONNU: &str = "code-inconnu:";

/// Le libellé français d'un jeton, ou `None` si la chaîne n'en est pas un.
///
/// Renvoyer `None` plutôt qu'une valeur de repli est délibéré : une version de
/// firmware, un nom de fuseau ou un chemin d'exclusion n'ont rien à traduire, et
/// les faire passer par une table les exposerait à une traduction accidentelle.
#[must_use]
pub fn libelle_jeton(jeton: &str) -> Option<String> {
    if let Some(code) = jeton.strip_prefix(PREFIXE_CODE_INCONNU) {
        return Some(format!("code inconnu ({code})"));
    }
    LIBELLES
        .iter()
        .find(|(j, _)| *j == jeton)
        .map(|(_, libelle)| (*libelle).to_owned())
}

/// Rend une valeur lisible, jetons traduits, sans rien convertir d'autre.
///
/// C'est ce dont `ks explain` et le rapport HTML ont besoin : ils affichent la
/// valeur constatée, et rien que la valeur constatée.
#[must_use]
pub fn libelle(valeur: &ItemValue) -> String {
    match valeur {
        ItemValue::Text(t) => libelle_jeton(t).unwrap_or_else(|| t.clone()),
        ItemValue::Absent
        | ItemValue::Bool(_)
        | ItemValue::Int(_)
        | ItemValue::List(_)
        | ItemValue::Illisible { .. } => valeur.to_string(),
    }
}

/// Les composantes exactes d'une valeur composite, ou `None` si elle n'en a pas.
///
/// # Pourquoi cette fonction existe
///
/// Le principe P6 exige qu'« un indicateur composite soit toujours dépliable en
/// ses composantes exactes ». Une [`ItemValue::List`] s'affichait « 14
/// élément(s) » partout — dans `ks explain` comme dans le rapport — et **nulle
/// part** on ne pouvait voir lesquels. L'item était donc affiché sans être
/// explicable, ce que P6 interdit.
///
/// Le résumé n'est pas remplacé pour autant : une cellule de tableau porte le
/// décompte, et le dépliage vit à côté. Les deux répondent à deux questions.
#[must_use]
pub fn composantes(valeur: &ItemValue) -> Option<&[String]> {
    match valeur {
        ItemValue::List(entrees) => Some(entrees),
        ItemValue::Absent
        | ItemValue::Bool(_)
        | ItemValue::Int(_)
        | ItemValue::Text(_)
        | ItemValue::Illisible { .. } => None,
    }
}

/// Rend une valeur d'item lisible par un humain, sans toucher au modèle.
///
/// Seuls les chemins qui se terminent par `_bytes` sont convertis : la
/// conversion se déclenche sur une convention de nommage explicite, pas sur une
/// heuristique de magnitude qui transformerait un jour un compteur en taille.
#[must_use]
pub fn valeur(item: &Item) -> String {
    if let (true, ItemValue::Int(n)) = (item.path.ends_with("_bytes"), &item.observed) {
        if let Ok(o) = u64::try_from(*n) {
            return octets(o);
        }
    }
    libelle(&item.observed)
}

/// Formate une taille en unités binaires, avec la ponctuation française.
///
/// Une décimale suffit : la deuxième donnerait une précision que ni le calcul
/// ni le besoin ne justifient. L'espace avant l'unité est **insécable**, sans
/// quoi le terminal — ou la fenêtre — coupe « 52,2 » et « Gio » sur deux lignes.
#[must_use]
pub fn octets(octets: u64) -> String {
    const UNITES: [&str; 5] = ["o", "Kio", "Mio", "Gio", "Tio"];
    #[allow(clippy::cast_precision_loss)] // Une décimale affichée : la perte est sous l'arrondi.
    let mut valeur = octets as f64;
    let mut rang = 0;
    while valeur >= 1024.0 && rang < UNITES.len() - 1 {
        valeur /= 1024.0;
        rang += 1;
    }
    let arrondi = if rang == 0 {
        format!("{octets}")
    } else {
        format!("{valeur:.1}").replace('.', ",")
    };
    format!("{arrondi}\u{a0}{}", UNITES[rang])
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_collectors::jetons;

    #[test]
    fn une_taille_saffiche_en_unites_lisibles() {
        // La valeur relevée sur la machine de référence. « 56043241472 » ne dit
        // rien à personne ; « 52,2 Gio » situe immédiatement le disque WSL comme
        // le premier poste d'occupation du poste.
        assert_eq!(octets(56_043_241_472), "52,2\u{a0}Gio");
        assert_eq!(octets(100_663_296), "96,0\u{a0}Mio");
        // En deçà du kibioctet, l'arrondi n'apporte rien : on garde l'entier.
        assert_eq!(octets(0), "0\u{a0}o");
        assert_eq!(octets(512), "512\u{a0}o");
        assert_eq!(octets(1024), "1,0\u{a0}Kio");
    }

    #[test]
    fn la_virgule_est_francaise_et_lespace_insecable() {
        let rendu = octets(1_610_612_736);
        assert!(rendu.contains(','), "séparateur décimal français");
        assert!(!rendu.contains('.'), "jamais le point décimal anglais");
        assert!(
            rendu.contains('\u{a0}'),
            "espace insécable : sinon le terminal coupe le nombre de son unité"
        );
        assert!(!rendu.contains(' '), "aucune espace ordinaire");
    }

    /// **Le libellé de chaque jeton, figé mot pour mot.**
    ///
    /// C'est le critère d'acceptation de l'ADR-0015 : le vocabulaire déménage
    /// du collecteur vers l'affichage, et l'écran ne bouge pas. Les chaînes
    /// ci-dessous sont celles que `ks scan`, `ks explain`, le rapport HTML et
    /// le poste de pilotage affichaient avant le déménagement.
    #[test]
    fn le_libelle_francais_est_celui_daffiche_avant_le_demenagement() {
        for (jeton, attendu) in [
            ("demarrage-noyau", "au démarrage du noyau"),
            ("demarrage-systeme", "au démarrage du système"),
            ("automatique", "automatique"),
            ("manuel", "manuel"),
            ("desactive", "désactivé"),
            ("inactive", "désactivée"),
            ("active-verrou-uefi", "activée, verrouillée par UEFI"),
            ("active-sans-verrou-uefi", "activée, sans verrou UEFI"),
            ("active", "activée"),
            ("eteint", "éteinte"),
            (
                "configure-non-demarre",
                "configurée, mais pas en cours d'exécution",
            ),
            ("en-execution", "en cours d'exécution"),
            ("arrete", "à l'arrêt"),
            ("eteinte", "éteinte"),
            ("audit", "audit — journalise sans bloquer"),
            ("imposee", "imposée"),
            ("disponible-sur-ce-materiel", "disponible sur ce matériel"),
            ("absente-de-ce-materiel", "absente de ce matériel"),
            ("complete", "complète"),
            ("partielle", "partielle"),
        ] {
            assert_eq!(
                libelle_jeton(jeton).as_deref(),
                Some(attendu),
                "le libellé de « {jeton} » a changé : c'est un changement d'écran"
            );
        }

        // Un code hors table garde sa nuance jusqu'à l'écran, chiffre compris.
        assert_eq!(
            libelle_jeton("code-inconnu:42").as_deref(),
            Some("code inconnu (42)")
        );
    }

    /// Aucun jeton n'échappe à la table des libellés.
    ///
    /// La barrière est celle de l'ADR-0015 : ajouter un code casse d'abord la
    /// compilation de `ks_collectors::jetons` — le `match` exhaustif sans bras
    /// `_` de `variantes()` —, puis ce test, qui exige que le contributeur
    /// décide aussi de ce que l'utilisateur lira.
    #[test]
    fn chaque_jeton_porte_son_libelle_francais() {
        let vocabulaire = jetons::tous();
        assert!(
            !vocabulaire.is_empty(),
            "vocabulaire vide — ce test ne vérifierait rien"
        );

        for jeton in &vocabulaire {
            assert!(
                libelle_jeton(jeton).is_some(),
                "le jeton « {jeton} » n'a pas de libellé : il s'afficherait brut"
            );
        }

        // Et la table ne porte rien qui ne soit un jeton : une entrée orpheline
        // traduirait une valeur brute par accident.
        for (jeton, _) in LIBELLES {
            assert!(
                vocabulaire.iter().any(|v| v == jeton),
                "« {jeton} » n'appartient à aucune table de codes"
            );
        }
    }

    #[test]
    fn le_prefixe_des_codes_inconnus_est_celui_des_collecteurs() {
        // Deux écritures du même préfixe, aux deux bouts de la chaîne. Si
        // elles divergent, un code hors table s'affiche brut et personne ne
        // le remarque : la valeur reste lisible, seulement fausse de forme.
        assert_eq!(PREFIXE_CODE_INCONNU, jetons::PREFIXE_CODE_INCONNU);
    }

    #[test]
    fn une_valeur_qui_nest_pas_un_jeton_traverse_sans_etre_traduite() {
        // La version d'un firmware, un nom de fuseau, un chemin d'exclusion :
        // rien de tout cela ne sort d'une table de codes, et une traduction
        // accidentelle y serait pire qu'une absence de traduction.
        assert_eq!(libelle_jeton("P0CN20WW"), None);
        assert_eq!(libelle_jeton("Romance Standard Time"), None);
        assert_eq!(
            libelle(&ItemValue::Text("Romance Standard Time".into())),
            "Romance Standard Time"
        );
        // Les autres variantes gardent leur rendu d'origine.
        assert_eq!(libelle(&ItemValue::Absent), "absent");
        assert_eq!(libelle(&ItemValue::Bool(true)), "activé");
        assert_eq!(libelle(&ItemValue::Int(42)), "42");
        assert_eq!(
            libelle(&ItemValue::List(vec!["a".into(), "b".into()])),
            "2 élément(s)"
        );
        assert_eq!(
            libelle(&ItemValue::illisible("accès refusé sans élévation")),
            "illisible — accès refusé sans élévation"
        );
    }

    /// Une liste se déplie ; une valeur qui n'en est pas une ne se déplie pas.
    ///
    /// Le second contrôle importe autant que le premier : rendre `Some(&[])`
    /// pour un entier ferait afficher un dépliage vide sous chaque item scalaire.
    ///
    /// Le `match` de [`composantes`] est exhaustif **sans bras `_`** : ajouter
    /// une variante à `ItemValue` casse la compilation, donc la CI, avant qu'un
    /// test s'exécute — le contributeur doit venir décider si sa nouvelle forme
    /// se déplie.
    #[test]
    fn une_liste_se_deplie_en_ses_composantes_exactes() {
        let liste = ItemValue::List(vec!["b".into(), "a".into()]);
        assert_eq!(
            composantes(&liste),
            Some(["b".to_owned(), "a".to_owned()].as_slice()),
            "les composantes sont rendues telles quelles, sans tri ni troncature"
        );
        // Une liste vide se déplie en rien, et ce n'est pas une absence de
        // dépliage : « lue et vide » n'est pas « pas une liste ».
        assert_eq!(composantes(&ItemValue::List(vec![])), Some([].as_slice()));

        for scalaire in [
            ItemValue::Absent,
            ItemValue::Bool(true),
            ItemValue::Int(42),
            ItemValue::Text("jeton".into()),
            ItemValue::illisible("accès refusé sans élévation"),
        ] {
            assert_eq!(
                composantes(&scalaire),
                None,
                "« {scalaire} » n'est pas composite"
            );
        }
    }
}

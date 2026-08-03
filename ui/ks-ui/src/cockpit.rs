//! Le modèle que la coque envoie à l'écran — et la règle qui le gouverne.
//!
//! ## Aucun nombre affiché n'est inventé
//!
//! Chaque valeur que le cockpit montre vient d'un item réellement relevé, ou
//! l'écran dit qu'elle n'est pas collectée, avec la raison et ce qui la rendrait
//! disponible. Il n'existe ici ni valeur par défaut, ni zéro de repli, ni
//! pourcentage calculé sur un dénominateur absent.
//!
//! Trois mécanismes portent cette règle, et aucun ne repose sur la vigilance :
//!
//! 1. **Les valeurs vivent dans une seule table**, [`Cockpit::readings`],
//!    indexée par chemin d'item. Les écrans ne transportent que des **listes de
//!    chemins** : deux endroits de l'interface ne peuvent donc pas afficher deux
//!    valeurs différentes pour le même item, et un écran ne peut rien montrer
//!    qui ne soit pas dans la table.
//! 2. **[`NotComputable`] ne porte aucun champ numérique.** Un indicateur qu'on
//!    ne sait pas calculer ne peut pas transporter de nombre, même par accident.
//!    Le test `aucun_chiffre_ne_se_glisse_dans_un_indicateur_non_calculable` va
//!    plus loin et refuse le chiffre écrit en toutes lettres dans la prose.
//! 3. **[`DriftView`] est une énumération, pas une structure à décomptes.** Tant
//!    qu'aucun item ne porte d'état désiré, la variante publiée ne contient
//!    littéralement pas de champ « écarts » : la page ne peut pas afficher
//!    « 0 écart », parce que le zéro n'existe nulle part dans ce qu'elle reçoit.
//!    C'est très exactement ce que [`ks_core::item::Verdict::Incomparable`] a été
//!    créé pour empêcher, transposé à l'écran.

use std::collections::BTreeMap;

use ks_cli::lisible;
use ks_core::item::Verdict;
use ks_core::{Domain, Item, ItemValue};
use serde::Serialize;

/// Un relevé, tel que l'écran doit le rendre.
///
/// La valeur est déjà mise en forme ici, et pas dans la page : la CLI et la
/// coque affichent alors la même taille de disque, produite par le même code
/// ([`ks_cli::lisible`]).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// Chemin canonique de l'item — la clé qu'emploient les écrans.
    pub path: String,
    /// La valeur constatée, rendue lisible.
    pub value: String,
    /// À quoi sert cet item, en une phrase (principe P6).
    pub purpose: String,
    /// Ce qu'on risque en le changeant (principe P6).
    pub risk: String,
    /// La valeur brute, quand l'item porte un entier.
    ///
    /// Elle sert aux encodages visuels — la longueur d'une jauge d'occupation.
    /// Sans elle, la page devrait relire le nombre dans la chaîne affichée,
    /// c'est-à-dire redériver une mesure à partir de sa mise en forme.
    pub number: Option<i64>,
    /// Faux quand la lecture a échoué : ni une valeur, ni une absence.
    pub readable: bool,
    /// Pourquoi la lecture a échoué, quand elle a échoué.
    pub reason: Option<String>,
    /// Le contenu, quand l'item porte une liste. Sans lui, l'écran ne pourrait
    /// afficher que « 2 élément(s) », c'est-à-dire un décompte sans son objet.
    pub list: Option<Vec<String>>,
}

impl Reading {
    /// Construit le relevé d'un item.
    fn from(item: &Item) -> Self {
        let (readable, reason) = match &item.observed {
            ItemValue::Illisible { raison } => (false, Some(raison.clone())),
            ItemValue::Absent
            | ItemValue::Bool(_)
            | ItemValue::Int(_)
            | ItemValue::Text(_)
            | ItemValue::List(_) => (true, None),
        };
        let list = match &item.observed {
            ItemValue::List(v) => Some(v.clone()),
            ItemValue::Absent
            | ItemValue::Bool(_)
            | ItemValue::Int(_)
            | ItemValue::Text(_)
            | ItemValue::Illisible { .. } => None,
        };
        let number = match &item.observed {
            ItemValue::Int(n) => Some(*n),
            ItemValue::Absent
            | ItemValue::Bool(_)
            | ItemValue::Text(_)
            | ItemValue::List(_)
            | ItemValue::Illisible { .. } => None,
        };
        Self {
            path: item.path.clone(),
            value: valeur_affichee(item),
            purpose: item.purpose.clone(),
            risk: item.risk.clone(),
            number,
            readable,
            reason,
            list,
        }
    }
}

/// Rend la valeur d'un item telle que l'écran l'affiche.
///
/// La conversion des octets est celle de la CLI. Celle des durées lui est
/// propre : `286488` ne dit rien à personne, et la CLI n'a pas encore de raison
/// de changer sa sortie, qui est lue par des scripts.
fn valeur_affichee(item: &Item) -> String {
    if let (true, ItemValue::Int(n)) = (item.path.ends_with("_seconds"), &item.observed) {
        if let Ok(s) = u64::try_from(*n) {
            return duree(s);
        }
    }
    lisible::valeur(item)
}

/// Formate une durée en secondes, du plus grand ordre de grandeur utile.
///
/// Deux unités au plus : « 3 j 7 h » se lit d'un coup d'œil, « 3 j 7 h 34 min
/// 48 s » demande à être décodé, et la seconde près n'apporte rien sur un temps
/// de fonctionnement. L'espace avant l'unité est insécable.
fn duree(secondes: u64) -> String {
    const MINUTE: u64 = 60;
    const HEURE: u64 = 60 * MINUTE;
    const JOUR: u64 = 24 * HEURE;

    if secondes < MINUTE {
        format!("{secondes}\u{a0}s")
    } else if secondes < HEURE {
        format!("{}\u{a0}min", secondes / MINUTE)
    } else if secondes < JOUR {
        format!(
            "{}\u{a0}h {}\u{a0}min",
            secondes / HEURE,
            (secondes % HEURE) / MINUTE
        )
    } else {
        format!(
            "{}\u{a0}j {}\u{a0}h",
            secondes / JOUR,
            (secondes % JOUR) / HEURE
        )
    }
}

/// Un indicateur que Keystone ne sait pas calculer aujourd'hui.
///
/// **Ce type ne porte aucun champ numérique, et c'est sa raison d'être.** Un
/// `Option<f64>` se serait rempli d'un zéro le jour où quelqu'un aurait trouvé
/// commode d'avoir « une valeur par défaut » ; ici il n'y a pas de place pour
/// un nombre.
///
/// Les trois premiers champs suivent l'ordre imposé par la voix du produit
/// (brief §1.3) : ce qui s'est passé, ce que ça implique, ce qu'on peut faire.
/// Le quatrième est le détail technique, qui se copie et ne se lit pas — jamais
/// un code d'erreur nu en guise de phrase.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotComputable {
    /// Ce qui s'est passé.
    pub happened: String,
    /// Ce que ça implique.
    pub implies: String,
    /// Ce qu'on peut faire pour que l'indicateur devienne calculable.
    pub unlocked_by: String,
    /// Le détail technique : exigence concernée, cause exacte.
    pub detail: String,
}

impl NotComputable {
    /// Raccourci de construction, pour que les quatre champs restent alignés et
    /// lisibles à la déclaration.
    fn new(happened: &str, implies: &str, unlocked_by: &str, detail: &str) -> Self {
        Self {
            happened: happened.to_owned(),
            implies: implies.to_owned(),
            unlocked_by: unlocked_by.to_owned(),
            detail: detail.to_owned(),
        }
    }
}

/// Ce que vaut la confrontation entre état désiré et état constaté, à l'échelle
/// de la machine.
///
/// L'énumération est la barrière : tant qu'aucun item n'est contraint, la
/// variante publiée **ne contient pas de champ de décompte d'écarts**. Une
/// structure unique avec `deviation: 0` aurait laissé la page afficher
/// « 0 écart », c'est-à-dire annoncer qu'une comparaison a eu lieu et n'a rien
/// trouvé — sur une machine où rien n'a jamais été comparé.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DriftView {
    /// Aucun item ne porte d'état désiré.
    NotComputable {
        /// Pourquoi, et ce qui l'ouvrirait.
        #[serde(flatten)]
        why: NotComputable,
        /// Le nombre d'items lus sans contrainte déclarée. C'est une mesure
        /// réelle — celle du travail effectué — et non un décompte d'écarts.
        unconstrained: usize,
    },
    /// Au moins un item est contraint : les décomptes ont un sens.
    Computed {
        /// Items sans état désiré.
        unconstrained: usize,
        /// Items dont le constaté correspond au désiré.
        compliant: usize,
        /// Items en écart.
        deviation: usize,
        /// Items dont la lecture a échoué : ni conformes, ni en écart.
        incomparable: usize,
    },
}

/// Une famille d'items d'un même écran, telle qu'elle s'affiche en tableau.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    /// Deuxième segment du chemin — `platform` pour `security.platform.*`.
    pub key: String,
    /// Le libellé affiché.
    pub label: String,
    /// Les chemins des items, dans l'ordre où ils ont été relevés.
    pub paths: Vec<String>,
}

/// L'écran « Posture de sécurité ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityView {
    /// Nombre d'items du domaine.
    pub total: usize,
    /// Ceux dont la lecture a abouti.
    pub readable: usize,
    /// Ceux dont la lecture a échoué. Comptés, jamais rangés avec les autres.
    pub unreadable: usize,
    /// Et **nommés** : un décompte d'illisibles sans les chemins n'explique
    /// rien, ce que le principe P6 refuse.
    pub unreadable_paths: Vec<String>,
    /// Les familles, dans l'ordre d'affichage.
    pub families: Vec<Family>,
}

/// L'écran « Inventaire logiciel ».
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareView {
    /// Les agrégats : total, non attribuées, empaquetées, taux d'attribution…
    pub aggregate_paths: Vec<String>,
    /// Les applications qu'aucun gestionnaire ne suit, une par item.
    pub unattributed_paths: Vec<String>,
}

/// Une entrée nommée d'une famille indexée par crochets : un volume, une distro.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    /// Ce qui figure entre crochets dans le chemin — `C:\`, `debian`.
    pub name: String,
    /// Les chemins des items qui la décrivent.
    pub paths: Vec<String>,
}

/// Une ligne attendue à l'écran, qu'elle ait été relevée ou non.
///
/// La liste est **fixe**, et c'est le point : quand un chemin manque à l'appel,
/// l'écran affiche « non relevé » plutôt que de sauter la ligne. Une ligne
/// absente ne se remarque pas ; une ligne qui dit qu'elle manque, si.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Expected {
    /// Le chemin attendu.
    pub path: String,
    /// Le libellé de la ligne.
    pub label: String,
}

/// Le décompte d'items d'un domaine.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCount {
    /// Le libellé du domaine.
    pub label: String,
    /// Le nombre d'items relevés.
    pub count: usize,
}

/// L'état de la machine, tel que le cockpit doit l'afficher.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cockpit {
    /// Le rapport HTML autonome, tel que `ks report` l'écrirait.
    pub html: String,
    /// Le nom relevé sur cette machine, jamais un nom d'exemple.
    pub machine: String,
    /// Le nombre d'items réellement observés.
    pub item_count: usize,
    /// L'instant de la collecte, en RFC 3339.
    pub collected_at: String,
    /// **La seule source des valeurs affichées**, indexée par chemin d'item.
    pub readings: BTreeMap<String, Reading>,
    /// Répartition des items par domaine.
    pub domains: Vec<DomainCount>,
    /// Les lignes de l'écran « Machine », relevées ou non.
    pub machine_rows: Vec<Expected>,
    /// L'écran « Posture de sécurité ».
    pub security: SecurityView,
    /// L'écran « Inventaire logiciel ».
    pub software: SoftwareView,
    /// Les volumes relevés.
    pub volumes: Vec<Named>,
    /// Les distributions WSL relevées.
    pub distros: Vec<Named>,
    /// L'anneau : une posture composite ne se calcule pas sans référence.
    pub posture: NotComputable,
    /// La dérive, et le zéro qu'elle refuse de publier.
    pub drift: DriftView,
    /// Le domaine des mises à jour, qui n'est pas collecté à ce stade.
    pub updates: NotComputable,
    /// L'attribution de l'espace par consommateur, qui ne l'est pas non plus.
    pub space_attribution: NotComputable,
    /// Les machines virtuelles Hyper-V, hors du collecteur de virtualisation.
    pub virtual_machines: NotComputable,
}

/// Les lignes de l'écran « Machine », dans l'ordre d'affichage.
const MACHINE: &[(&str, &str)] = &[
    ("inventory.host.name", "Nom de la machine"),
    ("inventory.os.name", "Système"),
    ("inventory.os.kernel", "Version du noyau"),
    ("inventory.cpu.cores", "Cœurs logiques"),
    ("inventory.memory.total_bytes", "Mémoire installée"),
    ("inventory.uptime_seconds", "Temps de fonctionnement"),
];

/// Libellés des familles de la posture, dans l'ordre d'affichage.
///
/// Une famille relevée mais absente de cette table n'est **pas** perdue : elle
/// s'ajoute à la suite, étiquetée par sa clé. Un tableau de correspondance qui
/// filtre est un tableau qui fait disparaître.
const FAMILLES_SECURITE: &[(&str, &str)] = &[
    ("platform", "Plateforme"),
    ("defender", "Defender"),
    ("services", "Services"),
    ("firewall", "Pare-feu"),
    ("firmware", "Micrologiciel"),
    ("clock", "Horloge"),
];

/// Libellé d'un domaine. Repris de `ks_cli::rapport`, qui n'expose pas le sien.
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

/// Ordre d'affichage des domaines : la posture d'abord, c'est ce qu'on vient
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

/// Ce que porte un chemin entre crochets : `space.volume[C:\].used_percent`
/// donne `C:\`.
fn entre_crochets(path: &str) -> Option<&str> {
    let debut = path.find('[')?;
    let fin = path[debut..].find(']')? + debut;
    Some(&path[debut + 1..fin])
}

/// Le deuxième segment d'un chemin : `security.platform.secure_boot` donne
/// `platform`.
fn famille(path: &str) -> Option<&str> {
    path.split('.').nth(1)
}

/// Regroupe des chemins par entrée nommée, dans l'ordre de première apparition.
fn grouper_nommes<'a>(items: impl Iterator<Item = &'a Item>) -> Vec<Named> {
    let mut sortie: Vec<Named> = Vec::new();
    for item in items {
        let Some(nom) = entre_crochets(&item.path) else {
            continue;
        };
        match sortie.iter_mut().find(|n| n.name == nom) {
            Some(existant) => existant.paths.push(item.path.clone()),
            None => sortie.push(Named {
                name: nom.to_owned(),
                paths: vec![item.path.clone()],
            }),
        }
    }
    sortie
}

/// Construit l'écran de posture à partir des items du domaine sécurité.
fn construire_securite(items: &[Item]) -> SecurityView {
    let du_domaine: Vec<&Item> = items
        .iter()
        .filter(|i| i.domain == Domain::Security)
        .collect();

    let unreadable_paths: Vec<String> = du_domaine
        .iter()
        .filter(|i| !i.observed.est_constat())
        .map(|i| i.path.clone())
        .collect();

    // Les familles connues d'abord, dans l'ordre. Celles qu'on ne connaît pas
    // ensuite, étiquetées par leur clé — elles apparaissent plutôt que de
    // disparaître.
    let mut familles: Vec<Family> = Vec::new();
    for (clef, libelle) in FAMILLES_SECURITE {
        let paths: Vec<String> = du_domaine
            .iter()
            .filter(|i| famille(&i.path) == Some(*clef))
            .map(|i| i.path.clone())
            .collect();
        if !paths.is_empty() {
            familles.push(Family {
                key: (*clef).to_owned(),
                label: (*libelle).to_owned(),
                paths,
            });
        }
    }
    for item in &du_domaine {
        let Some(clef) = famille(&item.path) else {
            continue;
        };
        if FAMILLES_SECURITE.iter().any(|(k, _)| *k == clef) {
            continue;
        }
        match familles.iter_mut().find(|f| f.key == clef) {
            Some(existante) => existante.paths.push(item.path.clone()),
            None => familles.push(Family {
                key: clef.to_owned(),
                label: clef.to_owned(),
                paths: vec![item.path.clone()],
            }),
        }
    }

    SecurityView {
        total: du_domaine.len(),
        readable: du_domaine.len() - unreadable_paths.len(),
        unreadable: unreadable_paths.len(),
        unreadable_paths,
        families: familles,
    }
}

/// Compte les verdicts, et décide si la dérive a un sens.
///
/// Le `match` est **exhaustif, sans bras `_`** : ajouter une valeur à
/// [`Verdict`] casse la compilation ici, donc la CI, avant qu'un test
/// s'exécute. Le contributeur est forcé de décider ce que l'écran en fait.
fn construire_derive(items: &[Item]) -> DriftView {
    let (mut unconstrained, mut compliant, mut deviation, mut incomparable) = (0, 0, 0, 0);
    for item in items {
        match item.verdict() {
            Verdict::NonContraint => unconstrained += 1,
            Verdict::Conforme => compliant += 1,
            Verdict::Ecart => deviation += 1,
            Verdict::Incomparable { .. } => incomparable += 1,
        }
    }

    if compliant + deviation + incomparable == 0 {
        return DriftView::NotComputable {
            why: NotComputable::new(
                "Aucun item ne porte d'état désiré.",
                "Il n'y a donc pas zéro écart : il n'y a pas de comparaison. Publier un zéro \
                 laisserait croire qu'une confrontation a eu lieu et n'a rien trouvé.",
                "Adopter l'état lu comme référence ouvre cet écran : chaque item déclaré sera \
                 confronté à sa valeur constatée, et le résultat dira conforme, écart, ou \
                 incomparable quand la lecture a échoué.",
                "D2-02 · aucun état désiré chargé · Verdict::NonContraint sur tous les items",
            ),
            unconstrained,
        };
    }

    DriftView::Computed {
        unconstrained,
        compliant,
        deviation,
        incomparable,
    }
}

impl Cockpit {
    /// Assemble l'état affichable à partir des items relevés.
    ///
    /// Aucun appel système ici : la fonction ne reçoit que la donnée, ce qui la
    /// rend testable sur des inventaires fabriqués — y compris l'inventaire
    /// vide, qui est le cas où l'invention serait la plus tentante.
    #[must_use]
    pub fn build(items: &[Item], machine: &str, collected_at: &str, html: String) -> Self {
        let readings: BTreeMap<String, Reading> = items
            .iter()
            .map(|i| (i.path.clone(), Reading::from(i)))
            .collect();

        let domains: Vec<DomainCount> = ORDRE
            .iter()
            .filter_map(|d| {
                let count = items.iter().filter(|i| i.domain == *d).count();
                (count > 0).then(|| DomainCount {
                    label: titre_domaine(*d).to_owned(),
                    count,
                })
            })
            .collect();

        let software = SoftwareView {
            aggregate_paths: items
                .iter()
                .filter(|i| i.path.starts_with("inventory.software.") && !i.path.contains('['))
                .map(|i| i.path.clone())
                .collect(),
            unattributed_paths: items
                .iter()
                .filter(|i| i.path.starts_with("inventory.software["))
                .map(|i| i.path.clone())
                .collect(),
        };

        Self {
            html,
            machine: machine.to_owned(),
            item_count: items.len(),
            collected_at: collected_at.to_owned(),
            readings,
            domains,
            machine_rows: MACHINE
                .iter()
                .map(|(path, label)| Expected {
                    path: (*path).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
            security: construire_securite(items),
            software,
            volumes: grouper_nommes(items.iter().filter(|i| i.path.starts_with("space.volume["))),
            distros: grouper_nommes(
                items
                    .iter()
                    .filter(|i| i.path.starts_with("virtualization.wsl[")),
            ),
            posture: NotComputable::new(
                "Aucun état de référence n'est chargé sur ce poste.",
                "Une posture composite se calcule contre une référence. Sans référence, tout \
                 chiffre affiché ici serait une convention, pas une mesure — et un indicateur \
                 composite qu'on ne sait pas déplier en ses composantes exactes est une \
                 décoration.",
                "Adopter l'état lu comme référence produira le fichier d'état désiré. La posture \
                 deviendra alors calculable, et dépliable item par item.",
                "P6 · D2-02 · workstation.yaml non chargé",
            ),
            drift: construire_derive(items),
            updates: NotComputable::new(
                "Le domaine des mises à jour n'est pas collecté à ce stade.",
                "Le nombre de mises à jour en attente n'est donc pas connu, et un zéro serait une \
                 affirmation que rien n'appuie.",
                "Le collecteur de mises à jour interrogera winget, Windows Update et les \
                 gestionnaires des distributions. L'écran affichera alors le plan, ses vagues et \
                 son retour arrière.",
                "D3 · aucun collecteur enregistré pour ce domaine",
            ),
            space_attribution: NotComputable::new(
                "Le relevé porte le taux d'occupation de chaque volume, pas ce qui l'occupe.",
                "L'écran ne peut donc pas dire quelle part revient aux disques virtuels, aux \
                 caches ou aux données de travail, et il n'avance aucune quantité récupérable.",
                "L'attribution par consommateur, avec la quarantaine qui la rend réversible, \
                 arrive au domaine de l'espace.",
                "D4 · attribution par consommateur non collectée",
            ),
            virtual_machines: NotComputable::new(
                "Les machines virtuelles Hyper-V ne sont pas relevées.",
                "Cet écran ne montre que les distributions WSL : une machine virtuelle, même en \
                 cours d'exécution, n'y figure pas.",
                "Le collecteur de virtualisation couvrira Hyper-V au même titre que WSL.",
                "D9 · Hyper-V hors du collecteur virtualisation",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_core::{Nature, Provenance};

    fn item(chemin: &str, domaine: Domain, valeur: ItemValue) -> Item {
        Item {
            path: chemin.to_owned(),
            domain: domaine,
            // Le poste de pilotage ne distingue pas encore les natures : il
            // compte et répartit tout ce qui a été observé. Ce lot ne change
            // pas ce que l'écran dit.
            nature: Nature::Reglage,
            desired: None,
            observed: valeur,
            observed_at: chrono::Utc::now(),
            provenance: Provenance::Observed,
            purpose: "Une finalité.".to_owned(),
            risk: "Un risque.".to_owned(),
            reference: None,
        }
    }

    fn construire(items: &[Item]) -> Cockpit {
        Cockpit::build(
            items,
            "POSTE-DE-TEST",
            "2026-08-03T12:00:00Z",
            String::new(),
        )
    }

    #[test]
    fn un_inventaire_vide_ne_produit_aucun_chiffre_de_mesure() {
        // Le cas où l'invention serait la plus tentante : rien n'a été lu, et
        // il faut quand même remplir un écran. La réponse est de ne rien
        // remplir — pas un zéro, pas un pourcentage, pas une famille vide.
        let vide = construire(&[]);

        assert_eq!(vide.item_count, 0);
        assert!(vide.readings.is_empty(), "aucune valeur ne s'invente");
        assert!(vide.domains.is_empty(), "aucun domaine ne se déclare seul");
        assert!(vide.volumes.is_empty());
        assert!(vide.distros.is_empty());
        assert!(vide.security.families.is_empty());
        assert_eq!(vide.security.total, 0);
        assert!(vide.software.aggregate_paths.is_empty());
        assert!(vide.software.unattributed_paths.is_empty());

        // Les lignes attendues survivent, elles : l'écran doit pouvoir dire
        // « non relevé », ce qu'une liste vide ne permettrait pas.
        assert_eq!(vide.machine_rows.len(), MACHINE.len());
        for ligne in &vide.machine_rows {
            assert!(
                !vide.readings.contains_key(&ligne.path),
                "« {} » : la ligne est attendue, la valeur ne doit pas exister",
                ligne.path
            );
        }
    }

    #[test]
    fn la_derive_ne_publie_jamais_un_zero_ecart() {
        // Le défaut que `Verdict::Incomparable` a été créé pour empêcher,
        // transposé à l'écran : sans état désiré, il n'y a pas zéro écart, il
        // n'y a pas de comparaison. La barrière n'est pas une convention
        // d'affichage — le champ n'existe pas dans ce que la page reçoit.
        let sans_desir = construire(&[
            item("security.a", Domain::Security, ItemValue::Bool(true)),
            item("security.b", Domain::Security, ItemValue::Bool(false)),
        ]);
        let json = serde_json::to_string(&sans_desir.drift).expect("structure simple");

        assert!(
            !json.contains("deviation"),
            "un décompte d'écarts est publié alors qu'aucune comparaison n'a eu lieu : {json}"
        );
        assert!(!json.contains("compliant"), "{json}");
        assert!(json.contains("notComputable"), "{json}");
        assert!(
            json.contains("\"unconstrained\":2"),
            "le nombre d'items lus reste une mesure réelle : {json}"
        );

        // Falsification : dès qu'un item est contraint, la comparaison a un
        // sens, et les décomptes reviennent. Sans ce second cas, la barrière
        // pourrait tenir en ne publiant jamais rien.
        let mut contraint = item("security.a", Domain::Security, ItemValue::Bool(false));
        contraint.desired = Some(ItemValue::Bool(true));
        let avec_desir = construire(&[
            contraint,
            item("security.b", Domain::Security, ItemValue::Bool(false)),
        ]);
        let json = serde_json::to_string(&avec_desir.drift).expect("structure simple");
        assert!(json.contains("\"deviation\":1"), "{json}");
        assert!(json.contains("\"unconstrained\":1"), "{json}");
        assert!(!json.contains("notComputable"), "{json}");
    }

    #[test]
    fn aucun_chiffre_ne_se_glisse_dans_un_indicateur_non_calculable() {
        // Le type n'a pas de champ numérique ; restait la prose, où un « 0 »
        // ou un « 94 » se serait glissé sans que rien ne le voie. Le champ
        // `detail` est exclu à dessein : il porte les références d'exigences
        // (D2-02, D3), qui ne sont pas des mesures.
        let c = construire(&[]);
        let mut indicateurs = vec![
            ("posture", &c.posture),
            ("updates", &c.updates),
            ("space_attribution", &c.space_attribution),
            ("virtual_machines", &c.virtual_machines),
        ];
        let DriftView::NotComputable { why, .. } = &c.drift else {
            unreachable!("un inventaire vide ne se compare à rien")
        };
        indicateurs.push(("drift", why));

        for (nom, indicateur) in indicateurs {
            for (champ, texte) in [
                ("happened", &indicateur.happened),
                ("implies", &indicateur.implies),
                ("unlocked_by", &indicateur.unlocked_by),
            ] {
                assert!(
                    !texte.chars().any(|c| c.is_ascii_digit()),
                    "{nom}.{champ} porte un chiffre, donc une mesure qui n'existe pas : {texte}"
                );
                assert!(!texte.is_empty(), "{nom}.{champ} est vide");
            }
            assert!(
                !indicateur.detail.is_empty(),
                "{nom} sans détail technique à recopier"
            );
        }
    }

    #[test]
    fn la_voix_du_produit_tient_dans_les_messages_de_la_coque() {
        // Cinq règles, issues du brief §1.3, et un vocabulaire proscrit vérifié
        // par ailleurs sur la documentation. Ces messages sont les seules
        // phrases longues que la coque produise côté Rust : ils sont donc le
        // seul endroit où la voix peut déraper sans que personne le voie.
        let c = construire(&[]);
        let DriftView::NotComputable { why, .. } = &c.drift else {
            unreachable!("un inventaire vide ne se compare à rien")
        };

        for indicateur in [
            &c.posture,
            &c.updates,
            &c.space_attribution,
            &c.virtual_machines,
            why,
        ] {
            for texte in [
                &indicateur.happened,
                &indicateur.implies,
                &indicateur.unlocked_by,
                &indicateur.detail,
            ] {
                assert!(
                    !texte.contains('!'),
                    "ni superlatif d'urgence ni point d'exclamation : {texte}"
                );
                let bas = texte.to_lowercase();
                for proscrit in [
                    "nettoy",
                    "optimis",
                    "problème",
                    "menace détectée",
                    "vous avez oublié",
                    "critique",
                    "urgent",
                ] {
                    assert!(
                        !bas.contains(proscrit),
                        "« {proscrit} » est proscrit par le glossaire : {texte}"
                    );
                }
            }
        }

        // L'ordre imposé : ce qui s'est passé, ce que ça implique, ce qu'on
        // peut faire. Un champ vide casserait la séquence sans rien signaler.
        assert!(c.posture.happened.ends_with('.'));
        assert!(!c.posture.implies.is_empty());
        assert!(!c.posture.unlocked_by.is_empty());
    }

    #[test]
    fn un_item_illisible_est_compte_et_nomme_a_part() {
        // Trois items sont illisibles en permanence sur la machine de
        // référence. Les fondre dans le total afficherait vert sur ce qu'on
        // n'a pas regardé ; les compter sans les nommer n'expliquerait rien.
        let c = construire(&[
            item(
                "security.platform.secure_boot",
                Domain::Security,
                ItemValue::Bool(true),
            ),
            item(
                "security.defender.exclusions.paths",
                Domain::Security,
                ItemValue::illisible("accès refusé sans élévation"),
            ),
        ]);

        assert_eq!(c.security.total, 2);
        assert_eq!(c.security.readable, 1);
        assert_eq!(c.security.unreadable, 1);
        assert_eq!(
            c.security.unreadable_paths,
            vec!["security.defender.exclusions.paths"]
        );

        let releve = &c.readings["security.defender.exclusions.paths"];
        assert!(!releve.readable);
        assert_eq!(
            releve.reason.as_deref(),
            Some("accès refusé sans élévation"),
            "la raison remonte jusqu'à l'écran"
        );
    }

    #[test]
    fn une_famille_inconnue_apparait_plutot_que_de_disparaitre() {
        // Un tableau de correspondance qui filtre est un tableau qui fait
        // disparaître : le jour où un collecteur publie `security.tpm.*`,
        // l'écran doit le montrer, fût-ce sous sa clé brute.
        let c = construire(&[
            item(
                "security.platform.secure_boot",
                Domain::Security,
                ItemValue::Bool(true),
            ),
            item(
                "security.tpm.version",
                Domain::Security,
                ItemValue::Text("2.0".into()),
            ),
        ]);

        let clefs: Vec<&str> = c.security.families.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(clefs, vec!["platform", "tpm"]);
        assert_eq!(c.security.total, 2);
        assert_eq!(
            c.security
                .families
                .iter()
                .map(|f| f.paths.len())
                .sum::<usize>(),
            c.security.total,
            "aucun item de la posture ne doit tomber hors des familles"
        );
    }

    #[test]
    fn les_volumes_et_les_distros_se_nomment_depuis_leur_chemin() {
        let c = construire(&[
            item(
                "space.volume[C:\\].used_percent",
                Domain::Space,
                ItemValue::Int(59),
            ),
            item(
                "space.volume[D:\\].used_percent",
                Domain::Space,
                ItemValue::Int(29),
            ),
            item(
                "virtualization.wsl[debian].version",
                Domain::Virtualization,
                ItemValue::Int(2),
            ),
            item(
                "virtualization.wsl[debian].disk_bytes",
                Domain::Virtualization,
                ItemValue::Int(56_043_241_472),
            ),
        ]);

        assert_eq!(
            c.volumes
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            vec!["C:\\", "D:\\"]
        );
        assert_eq!(c.distros.len(), 1);
        assert_eq!(c.distros[0].name, "debian");
        assert_eq!(c.distros[0].paths.len(), 2);

        // Et la taille s'affiche comme dans la CLI, par le même code.
        assert_eq!(
            c.readings["virtualization.wsl[debian].disk_bytes"].value,
            "52,2\u{a0}Gio"
        );
    }

    #[test]
    fn une_duree_saffiche_en_deux_unites_au_plus() {
        // `286488` est le temps de fonctionnement relevé sur la machine de
        // référence. Affiché tel quel, il ne dit rien à personne.
        assert_eq!(duree(286_488), "3\u{a0}j 7\u{a0}h");
        assert_eq!(duree(3_600), "1\u{a0}h 0\u{a0}min");
        assert_eq!(duree(90), "1\u{a0}min");
        assert_eq!(duree(42), "42\u{a0}s");
        assert!(
            duree(286_488).contains('\u{a0}'),
            "espace insécable : le nombre ne se coupe pas de son unité"
        );
    }

    #[test]
    fn une_liste_arrive_avec_son_contenu_et_pas_seulement_son_decompte() {
        // « 2 élément(s) » est un décompte sans son objet. L'écran doit
        // pouvoir nommer les deux gestionnaires détectés.
        let c = construire(&[item(
            "inventory.software.managers",
            Domain::Inventory,
            ItemValue::List(vec!["winget".into(), "MSIX".into()]),
        )]);
        assert_eq!(
            c.readings["inventory.software.managers"].list,
            Some(vec!["winget".to_owned(), "MSIX".to_owned()])
        );
    }
}

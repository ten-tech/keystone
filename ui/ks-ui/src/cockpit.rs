//! Le modèle que la coque envoie à l'écran — et la règle qui le gouverne.
//!
//! ## Aucun nombre affiché n'est inventé
//!
//! Chaque valeur que le cockpit montre vient d'un item réellement relevé, ou
//! l'écran dit qu'elle n'est pas collectée, avec la raison et ce qui la rendrait
//! disponible. Il n'existe ici ni valeur par défaut, ni zéro de repli, ni
//! pourcentage calculé sur un dénominateur absent.
//!
//! Quatre mécanismes portent cette règle, et aucun ne repose sur la vigilance :
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
//!    qu'aucun fichier d'état désiré n'a été confronté, la variante publiée ne
//!    contient littéralement pas de champ « écarts » : la page ne peut pas
//!    afficher « 0 écart », parce que le zéro n'existe nulle part dans ce
//!    qu'elle reçoit. C'est très exactement ce que
//!    [`ks_core::item::Verdict::Incomparable`] a été créé pour empêcher,
//!    transposé à l'écran.
//! 4. **Les figures comptent des chemins, elles ne reçoivent pas de nombres.**
//!    [`NatureGroup`] ne porte pas de décompte, exactement comme [`Family`] :
//!    la longueur d'un segment est le nombre d'items de sa liste, et la ligne
//!    de tableau qui lui répond vient de la même liste — les deux ne peuvent
//!    donc pas se contredire. Le test
//!    `aucune_figure_ne_publie_un_nombre_ecrit_a_la_main` refuse toute autre
//!    source dans `app.js`, et
//!    `chaque_item_releve_tombe_dans_exactement_une_nature` vérifie que la
//!    somme des parts fait toujours le tout — une part-à-tout dont les parts
//!    ne font plus le compte est le mensonge qu'une barre empilée raconte le
//!    mieux.
//!
//! ## Un écart toléré reste un écart
//!
//! [`DriftView::Computed`] porte les écarts **et** les tolérances qui les
//! annotent, jamais les uns à la place des autres. Le verdict dit le fait
//! constaté, la tolérance dit la politique décidée : ranger un item toléré
//! ailleurs que dans les écarts le ferait sortir de l'écran, alors que c'est
//! précisément ce qu'on a choisi de garder sous les yeux jusqu'à une date.
//!
//! Les tolérances qui ne couvrent plus rien — échue, sans objet, chemin non
//! observé — voyagent avec les autres et se lisent à part : elles ne décrivent
//! pas la machine, elles décrivent le fichier, et ce sont trois manières dont un
//! état désiré pourrit en silence (D2-07).
//!
//! Sans ces champs, la coque et `ks diff` diraient deux choses du même fichier,
//! ce que le principe P4 interdit — la CLI est la surface de référence. C'est la
//! même raison qui fait compter les verdicts par
//! [`ks_cli::confrontation::verdict_publie`] plutôt que par [`Item::verdict`].
//!
//! ## Ce que la coque ne sait pas encore tracer
//!
//! [`Cockpit::observation_series`] déclare l'absence de la frise des
//! intervalles d'observation. La coque lit **un relevé**, pas une série : un
//! intervalle demande deux bornes, et une lecture n'en donne qu'une. La figure
//! ne se dessine donc pas, et dit pourquoi — ce qui n'est pas mesuré se
//! déclare, avec sa raison et l'action qui le rendrait disponible (ADR-0013).

use std::collections::BTreeMap;

use ks_cli::confrontation::{self, Confrontation, ErreurDeChargement, Qualification};
use ks_cli::etat_desire::ErreurDeLecture;
use ks_cli::lisible;
use ks_core::item::Verdict;
use ks_core::{Domain, Item, ItemValue, Nature};
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
    /// La valeur déclarée dans le fichier d'état désiré, rendue lisible.
    ///
    /// `None` quand rien ne contraint cet item — ce qui est le cas de tous tant
    /// qu'aucun fichier n'est chargé. Elle vit ici, dans la table des relevés,
    /// et non dans [`DriftView`] : c'est une valeur de l'item, et la table est
    /// **la seule source des valeurs affichées**. La mettre ailleurs ouvrirait
    /// un second endroit où le désiré d'un item peut s'écrire.
    ///
    /// La mise en forme est celle de la CLI ([`ks_cli::lisible::libelle`]), la
    /// même que `ks diff` emploie pour la colonne « déclaré ».
    pub desired: Option<String>,
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
            desired: item.desired.as_ref().map(lisible::libelle),
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

/// Ce qu'une tolérance déclarée vaut aujourd'hui, tel que l'écran le rend.
///
/// Transposition exacte de [`Qualification`], et le `match` qui la produit est
/// **exhaustif sans bras `_`** : ajouter une qualification au noyau casse la
/// compilation ici, donc oblige à décider ce que l'écran en dit.
///
/// Les trois valeurs autres que [`Self::InForce`] décrivent chacune une manière
/// dont un fichier d'état pourrit en silence, et c'est pourquoi elles portent un
/// nom plutôt qu'un drapeau : une échéance dépassée que personne ne relit, une
/// tolérance qui ne couvre plus rien, un chemin qui n'existe plus.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ToleranceStatus {
    /// L'item est en écart et la tolérance court encore.
    InForce {
        /// Jours restants, l'échéance comprise. Vaut zéro le jour même.
        #[serde(rename = "daysLeft")]
        days_left: i64,
    },
    /// L'échéance est passée : l'écart est redevenu actif, tout seul.
    Expired {
        /// Depuis combien de jours.
        #[serde(rename = "daysSince")]
        days_since: i64,
    },
    /// L'item n'est pas en écart : la tolérance ne couvre rien.
    Moot,
    /// Aucun item observé ne porte ce chemin.
    NotObserved,
}

impl ToleranceStatus {
    /// Ce que le noyau a qualifié, tel que l'écran doit le rendre.
    ///
    /// Le `match` est **exhaustif sans bras `_`**, et c'est la barrière : une
    /// variante ajoutée à [`Qualification`] casse la compilation de la coque
    /// avant qu'un test s'exécute.
    fn from(qualification: &Qualification) -> Self {
        match qualification {
            Qualification::EnVigueur { jours_restants } => Self::InForce {
                days_left: *jours_restants,
            },
            Qualification::Echue { depuis_jours } => Self::Expired {
                days_since: *depuis_jours,
            },
            Qualification::SansObjet => Self::Moot,
            Qualification::NonObservee => Self::NotObserved,
        }
    }
}

/// Une tolérance déclarée, avec ce qu'elle vaut, telle que l'écran la rend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToleranceView {
    /// Le chemin visé, tel qu'il est écrit dans le fichier.
    pub path: String,
    /// Pourquoi cet écart est toléré, tel qu'il est écrit dans le fichier.
    pub reason: String,
    /// Le dernier jour où elle vaut, inclus.
    pub expires: chrono::NaiveDate,
    /// Ce qu'elle vaut réellement.
    #[serde(flatten)]
    pub status: ToleranceStatus,
}

/// Ce que vaut la confrontation entre état désiré et état constaté, à l'échelle
/// de la machine.
///
/// L'énumération est la barrière : tant qu'aucun fichier d'état désiré n'a été
/// confronté, la variante publiée **ne contient pas de champ de décompte
/// d'écarts**. Une structure unique avec `deviation: 0` aurait laissé la page
/// afficher « 0 écart », c'est-à-dire annoncer qu'une comparaison a eu lieu et
/// n'a rien trouvé — sur une machine où rien n'a jamais été comparé.
///
/// # Les tolérances voyagent avec les décomptes, et jamais sans
///
/// Une liste d'écarts sans les tolérances qui les annotent ferait dire à la
/// coque autre chose qu'à `ks diff` du même fichier, ce que le principe P4
/// interdit : la CLI est la surface de référence. Les deux vivent donc dans la
/// même variante, et elles n'existent que là où une confrontation a eu lieu.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DriftView {
    /// Aucun fichier d'état désiré n'a été confronté au relevé.
    NotComputable {
        /// Pourquoi, et ce qui l'ouvrirait.
        #[serde(flatten)]
        why: NotComputable,
        /// Le nombre d'items lus sans contrainte déclarée. C'est une mesure
        /// réelle — celle du travail effectué — et non un décompte d'écarts.
        unconstrained: usize,
    },
    /// Un fichier a été confronté : les décomptes ont un sens.
    Computed {
        /// Items sans état désiré.
        unconstrained: usize,
        /// Items dont le constaté correspond au désiré.
        compliant: usize,
        /// **Les chemins** des items en écart, dans l'ordre du relevé.
        ///
        /// Des chemins et non un décompte, pour la raison écrite en tête de ce
        /// module : l'écran compte ce qu'il reçoit, et la ligne d'un écart vient
        /// de la même liste que le nombre affiché à côté. Un champ `deviation`
        /// séparé serait un second endroit où ce nombre s'écrit, donc un endroit
        /// où il peut différer.
        deviations: Vec<String>,
        /// Items dont la lecture a échoué : ni conformes, ni en écart.
        incomparable: usize,
        /// Les tolérances déclarées, chacune qualifiée, **dans l'ordre du
        /// fichier** — celui dans lequel l'utilisateur les relira.
        ///
        /// Elles ne déplacent aucun item : un écart toléré reste publié en
        /// écart. Le verdict dit le fait, la tolérance dit la politique.
        tolerances: Vec<ToleranceView>,
    },
}

/// Ce que la coque a pu faire du fichier d'état désiré.
///
/// **Ce type est une entrée de [`Cockpit::build`], pas une sortie** : il n'est
/// pas sérialisable, et il porte le [`Confrontation`] du noyau. L'écran, lui,
/// reçoit [`DesiredStateView`] et [`DriftView`], qui en dérivent tous les deux.
///
/// La résolution du chemin ne se fait pas ici mais dans `main.rs`, à côté du
/// commentaire qui la justifie : c'est une décision de plateforme, et ce module
/// ne touche pas au disque — ce qui le laisse éprouvable sur des chargements
/// fabriqués, y compris ceux qui échouent.
#[derive(Debug, Clone)]
pub enum DesiredStateLoad {
    /// Le fichier a été lu, puis confronté au relevé.
    Loaded {
        /// Le chemin **réellement** lu, jamais le premier cherché.
        path: String,
        /// Ce que la confrontation a produit.
        bilan: Confrontation,
    },
    /// Aucun fichier à aucun des emplacements cherchés.
    ///
    /// C'est l'état normal d'un poste qui n'a pas encore adopté son état, et non
    /// une panne : il se distingue donc d'[`Self::Unusable`], qui est un échec.
    Missing {
        /// Les emplacements cherchés, dans l'ordre où ils l'ont été.
        ///
        /// Tous, et pas seulement le premier : un état qu'on ne peut pas
        /// expliquer n'est pas affichable (principe P6), et « fichier
        /// introuvable » sans dire *où* l'on a regardé n'explique rien.
        searched: Vec<String>,
    },
    /// Le fichier existe et n'a pas pu être exploité.
    Unusable {
        /// Le chemin réellement lu.
        path: String,
        /// Ce qui s'est passé, ce que ça implique, ce qu'on peut faire.
        message: String,
        /// Le détail technique, qui se copie et ne se lit pas.
        detail: String,
    },
}

impl DesiredStateLoad {
    /// Construit le refus à partir de l'erreur du noyau.
    ///
    /// Le message destiné à l'utilisateur et le détail technique restent **deux
    /// champs distincts** : c'est ce qui permet d'afficher l'un et de recopier
    /// l'autre dans un rapport, plutôt qu'un code d'erreur nu en guise de phrase.
    #[must_use]
    pub fn refuse(path: String, e: &ErreurDeChargement) -> Self {
        Self::Unusable {
            path,
            message: e.to_string(),
            detail: detail_technique(e),
        }
    }
}

/// Le détail technique d'un échec de chargement, celui que la phrase ne dit pas.
///
/// Les deux `match` sont **exhaustifs sans bras `_`** : une variante ajoutée à
/// [`ErreurDeChargement`] ou à [`ErreurDeLecture`] casse la compilation de la
/// coque, donc oblige à décider ce qu'elle en donne à recopier.
///
/// Le cas qui justifie cette fonction est [`ErreurDeLecture::Document`] : sa
/// phrase annonce que « le détail technique nomme la ligne en cause », et ce
/// détail vit dans un champ que l'affichage de l'erreur n'interpole pas. Le
/// recopier par `to_string()` seul le perdrait, en promettant le contraire.
fn detail_technique(e: &ErreurDeChargement) -> String {
    match e {
        ErreurDeChargement::Absent { chemin } => format!("fichier absent · {chemin}"),
        ErreurDeChargement::Fichier { chemin, detail } => format!("{chemin} · {detail}"),
        ErreurDeChargement::Lecture(lecture) => match lecture {
            ErreurDeLecture::Document { detail } => detail.clone(),
            ErreurDeLecture::Format { trouve, attendu } => {
                format!("apiVersion lu « {trouve} », attendu « {attendu} »")
            }
            ErreurDeLecture::AcceptationEnDouble { item } => {
                format!("acceptedDrift · deux entrées pour « {item} »")
            }
        },
        ErreurDeChargement::Typage { erreurs } => erreurs
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// D'où vient l'état désiré, tel que l'écran doit le dire.
///
/// **Le chemin réellement lu s'affiche toujours**, et quand il n'y en a pas, les
/// emplacements cherchés s'affichent tous : un écran qui annonce « aucune
/// comparaison » sans dire quel fichier il attendait, ni où, est un écran qu'on
/// ne peut pas expliquer (principe P6).
///
/// La prose, elle, n'est pas ici : elle vit dans le [`NotComputable`] de
/// [`DriftView::NotComputable`], qui est déjà l'endroit où l'écran va chercher
/// « ce qui s'est passé, ce que ça implique, ce qu'on peut faire ». L'écrire
/// deux fois en ferait deux textes qui divergent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DesiredStateView {
    /// Le fichier a été lu et confronté.
    Loaded {
        /// Le chemin réellement lu.
        path: String,
        /// Déclarations effectivement posées sur un item observé.
        declared: usize,
        /// Les chemins déclarés qu'aucun item observé ne porte.
        ///
        /// **Deux causes, indiscernables** : une faute de frappe, ou un item
        /// légitimement disparu. Les deux se nomment, sans en choisir une.
        #[serde(rename = "notObserved")]
        not_observed: Vec<String>,
    },
    /// Aucun fichier à aucun des emplacements cherchés.
    Missing {
        /// Les emplacements cherchés, dans l'ordre.
        searched: Vec<String>,
    },
    /// Le fichier existe et n'a pas pu être exploité.
    Unusable {
        /// Le chemin réellement lu.
        path: String,
    },
}

/// Une nature d'items, telle que la figure de répartition la dessine.
///
/// **Elle ne porte aucun décompte, et c'est délibéré.** Un champ `count` serait
/// un second endroit où le même nombre s'écrit, donc un endroit où il peut
/// différer de la table des relevés. L'écran compte les chemins qu'il reçoit,
/// exactement comme il le fait déjà pour les familles de la posture : la
/// longueur d'un segment et la ligne du tableau qui lui répond viennent alors
/// littéralement de la même liste.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NatureGroup {
    /// Le jeton de la nature — `reglage`, `objectif`, `mesure`, `constat`.
    ///
    /// C'est lui qui choisit la couleur du segment : la couleur suit l'entité,
    /// jamais son rang, donc un filtre qui change le nombre de segments ne
    /// repeint pas les survivants.
    pub key: String,
    /// Le libellé affiché.
    pub label: String,
    /// L'item a-t-il vocation à figurer dans le fichier d'état désiré ?
    pub declarable: bool,
    /// Ce que cette nature veut dire, en une phrase (principe P6).
    pub meaning: String,
    /// Les chemins des items, dans l'ordre où ils ont été relevés.
    pub paths: Vec<String>,
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

/// Ce qu'un disque virtuel pèse sur un volume : une part attribuée, et nommée.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribution {
    /// Ce qui occupe — le nom de la distribution WSL.
    pub name: String,
    /// Le chemin de l'item qui porte la taille. Sans lui, la part serait un
    /// nombre sans origine, ce que le principe P6 refuse.
    pub path: String,
    /// La taille, mise en forme par le même code que la CLI.
    pub value: String,
    /// La taille, en octets. C'est elle qui s'additionne ; la chaîne ci-dessus
    /// ne sert qu'à l'affichage.
    pub bytes: u64,
}

/// Ce que devient l'occupation d'un volume une fois la part nommée retirée.
///
/// # La barrière, et pourquoi elle est un type plutôt qu'un contrôle
///
/// **Le reste n'existe que dans la première variante.** Une somme attribuée
/// supérieure à l'occupation relevée ne produit donc ni un reste négatif, ni un
/// reste écrêté à zéro : le premier serait absurde, le second se lirait « tout
/// est attribué », soit l'inverse exact de ce qui a été mesuré. Elle produit
/// l'aveu que les deux mesures ne se rapprochent pas.
///
/// Le cas est atteignable, et pas théorique : un `ext4.vhdx` stocké épars ou
/// compressé par NTFS pèse moins sur le volume que sa taille apparente, et
/// `available_space` répond sous quota là où `total_space` ne l'est pas.
///
/// [`Unattributed::rapprocher`] en est le **seul** constructeur, et il passe par
/// `u64` : un reste négatif n'y est littéralement pas représentable.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Unattributed {
    /// Ce qui reste sans nom, une fois la part attribuée retirée.
    Measured {
        /// Le reste, mis en forme.
        value: String,
        /// Le reste, en octets.
        bytes: u64,
    },
    /// La somme des parts nommées dépasse l'occupation relevée.
    Unreconciled {
        /// De combien elle la dépasse, mis en forme. Le chiffre passe avant le
        /// constat, comme partout ailleurs.
        excess: String,
    },
}

impl Unattributed {
    /// Confronte la part nommée à l'occupation relevée.
    fn rapprocher(used_bytes: u64, attributed_bytes: u64) -> Self {
        match used_bytes.checked_sub(attributed_bytes) {
            Some(reste) => Self::Measured {
                value: lisible::octets(reste),
                bytes: reste,
            },
            // `checked_sub` sur `u64` échoue **exactement** quand la part
            // attribuée dépasse l'occupation. C'est la soustraction elle-même
            // qui refuse, pas une comparaison qu'on aurait pensé à écrire.
            None => Self::Unreconciled {
                excess: lisible::octets(attributed_bytes.saturating_sub(used_bytes)),
            },
        }
    }
}

/// L'occupation d'un volume, calculée à partir des deux items d'octets.
///
/// Le taux **ne se relève pas** : il se calcule ici, comme un jeton se traduit
/// en phrase française dans [`ks_cli::lisible`] plutôt que d'être stocké
/// (ADR-0015, ADR-0022). Un item de plus l'aurait rendu dérivable de deux
/// façons, donc divergent un jour.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Occupancy {
    /// Le chemin des octets occupés.
    pub used_path: String,
    /// Le chemin de la taille du volume.
    pub total_path: String,
    /// Les octets occupés, mis en forme — le chiffre qui passe avant le ratio.
    pub used: String,
    /// La taille du volume, mise en forme.
    pub total: String,
    /// Les octets occupés. Ils bornent la part attribuable.
    pub used_bytes: u64,
    /// La part occupée, en pour cent, arrondie à l'entier inférieur.
    pub percent: u64,
    /// Ce qui reste sans nom, ou l'aveu que les deux mesures ne se rapprochent
    /// pas.
    pub unattributed: Unattributed,
}

/// Un volume relevé, tel que l'écran « Espace » doit le rendre.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeView {
    /// Ce qui figure entre crochets dans le chemin — `C:\`.
    pub name: String,
    /// Les chemins des items relevés sur ce volume.
    pub paths: Vec<String>,
    /// Ce que Keystone sait nommer sur ce volume, disque par disque.
    ///
    /// Vide n'est pas un aveu d'ignorance : c'est le constat qu'aucun disque
    /// virtuel relevé ne vit ici. Ce que Keystone ne mesure pas du tout se dit
    /// ailleurs, dans [`Cockpit::space_attribution`].
    pub attributions: Vec<Attribution>,
    /// La somme des parts nommées, mise en forme.
    pub attributed: String,
    /// La somme des parts nommées, en octets.
    pub attributed_bytes: u64,
    /// L'occupation, quand la taille **et** les octets occupés ont été relevés.
    ///
    /// `None` dès qu'il en manque un : un taux sur un dénominateur absent est
    /// très exactement ce que ce module refuse de publier.
    pub occupancy: Option<Occupancy>,
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
    /// Répartition des items par nature — la figure qui explique ce qui se
    /// déclare, et ce qui ne se déclare jamais.
    pub natures: Vec<NatureGroup>,
    /// Les lignes de l'écran « Machine », relevées ou non.
    pub machine_rows: Vec<Expected>,
    /// L'écran « Posture de sécurité ».
    pub security: SecurityView,
    /// L'écran « Inventaire logiciel ».
    pub software: SoftwareView,
    /// Les volumes relevés, avec la part de leur occupation que Keystone nomme.
    pub volumes: Vec<VolumeView>,
    /// Les disques virtuels qu'aucun volume relevé ne réclame.
    ///
    /// Ils sont mesurés, et c'est bien pour cela qu'ils voyagent : une taille
    /// connue qu'on tait est le défaut symétrique d'une attribution inventée.
    pub unplaced_disks: Vec<Attribution>,
    /// Les distributions WSL relevées.
    pub distros: Vec<Named>,
    /// L'anneau : une posture composite ne se calcule pas sans référence.
    pub posture: NotComputable,
    /// D'où vient l'état désiré — le chemin lu, ou ceux qui ont été cherchés.
    pub desired_state: DesiredStateView,
    /// La dérive, et le zéro qu'elle refuse de publier.
    pub drift: DriftView,
    /// Le domaine des mises à jour, qui n'est pas collecté à ce stade.
    pub updates: NotComputable,
    /// L'attribution de l'espace par consommateur, qui ne l'est pas non plus.
    pub space_attribution: NotComputable,
    /// Les machines virtuelles Hyper-V, hors du collecteur de virtualisation.
    pub virtual_machines: NotComputable,
    /// La frise des intervalles d'observation, que la coque ne sait pas tracer.
    pub observation_series: NotComputable,
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

/// Ce qu'une nature dit à l'écran : son jeton, son libellé, son sens.
///
/// Le `match` est **exhaustif, sans bras `_`** : ajouter une variante à
/// [`Nature`] casse la compilation ici, donc la CI, avant qu'un test s'exécute.
/// Le contributeur est forcé de venir écrire ce que la figure en dit — et
/// surtout si elle se déclare, ce qui est tout le propos de cette figure.
///
/// La phrase n'est pas décorative : un item dont on n'explique ni la finalité
/// ni le sort n'est pas affichable (principe P6). C'est elle qui répond à
/// « pourquoi ce segment ne finit-il pas dans `workstation.yaml` ? ».
const fn dit_la_nature(nature: Nature) -> (&'static str, &'static str, &'static str) {
    match nature {
        Nature::Reglage => (
            "reglage",
            "Réglages",
            "Un réglage a un état désirable, et un verbe l'écrira le jour où la convergence \
             existera. Il se déclare.",
        ),
        Nature::Objectif => (
            "objectif",
            "Objectifs",
            "Un objectif se déclare et se suit, mais aucun verbe ne l'écrit : on peut le \
             vouloir, le mesurer et le signaler sans savoir agir dessus.",
        ),
        Nature::Mesure => (
            "mesure",
            "Mesures",
            "Une mesure évolue d'elle-même. Faute d'un vocabulaire de contrainte, la déclarer \
             reviendrait à l'exiger égale à sa valeur du jour, donc en écart à la lecture \
             suivante.",
        ),
        Nature::Constat => (
            "constat",
            "Constats",
            "Un constat est un fait sur la machine : on le relève, on le subit, on ne le \
             déclare jamais.",
        ),
    }
}

/// Ordre d'affichage des natures : ce qui se déclare d'abord.
///
/// Cette table peut, elle, oublier une variante sans casser la compilation —
/// c'est la limite du procédé, et elle est couverte par le test
/// `chaque_item_releve_tombe_dans_exactement_une_nature`, qui compare la somme
/// des groupes au nombre d'items relevés. Un segment manquant à la figure y
/// apparaît comme un total qui ne fait plus le compte.
const ORDRE_NATURES: &[Nature] = &[
    Nature::Reglage,
    Nature::Objectif,
    Nature::Mesure,
    Nature::Constat,
];

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

/// Ce que porte un chemin entre crochets : `space.volume[C:\].used_bytes`
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

/// La valeur entière d'un item, quand il existe et qu'il en porte une.
///
/// Le `match` est **exhaustif sans bras `_`** : ajouter une variante à
/// [`ItemValue`] casse la compilation ici, donc oblige à décider si elle porte
/// un nombre. Un `_` la rangerait en silence parmi les absences.
fn entier(items: &[Item], chemin: &str) -> Option<u64> {
    let item = items.iter().find(|i| i.path == chemin)?;
    match &item.observed {
        ItemValue::Int(n) => u64::try_from(*n).ok(),
        ItemValue::Absent
        | ItemValue::Bool(_)
        | ItemValue::Text(_)
        | ItemValue::List(_)
        | ItemValue::Illisible { .. } => None,
    }
}

/// Le texte d'un item, quand il existe et qu'il en porte un.
fn texte<'a>(items: &'a [Item], chemin: &str) -> Option<&'a str> {
    let item = items.iter().find(|i| i.path == chemin)?;
    match &item.observed {
        ItemValue::Text(t) => Some(t.as_str()),
        ItemValue::Absent
        | ItemValue::Bool(_)
        | ItemValue::Int(_)
        | ItemValue::List(_)
        | ItemValue::Illisible { .. } => None,
    }
}

/// Les disques virtuels relevés, chacun avec la lettre du volume qui le porte.
///
/// La lettre est normalisée par [`ks_collectors::lettre_de_volume`], le même
/// code que celui qui la dérive du registre : `C:\` du point de montage et `C:`
/// de la clé WSL désignent alors le même volume, et deux normalisations écrites
/// séparément ne peuvent pas cesser de rapprocher les mêmes choses.
///
/// Un disque dont le volume n'est pas relevé, ou pas dérivable, sort d'ici avec
/// `None`. Il n'est **jamais** rattaché « par défaut » au volume système : ce
/// presque-toujours-vrai compterait des gibioctets sur un volume qui ne les
/// porte pas, sans que rien ne le signale.
fn disques_virtuels(items: &[Item]) -> Vec<(Option<String>, Attribution)> {
    items
        .iter()
        .filter(|i| i.path.starts_with("virtualization.wsl[") && i.path.ends_with(".disk_bytes"))
        .filter_map(|i| {
            let nom = entre_crochets(&i.path)?;
            let octets = entier(items, &i.path)?;
            let volume = texte(items, &format!("virtualization.wsl[{nom}].volume"))
                .and_then(ks_collectors::lettre_de_volume);
            Some((
                volume,
                Attribution {
                    name: nom.to_owned(),
                    path: i.path.clone(),
                    value: lisible::octets(octets),
                    bytes: octets,
                },
            ))
        })
        .collect()
}

/// Construit l'écran « Espace » : chaque volume, son occupation, et la part que
/// Keystone sait en nommer.
///
/// Renvoie aussi les disques virtuels qu'**aucun** volume ne réclame — volume
/// non dérivable, ou lettre qui ne correspond à aucun volume relevé. Ils sont
/// mesurés : les taire les ferait disparaître de l'écran alors que leur taille
/// est connue, ce qui est le défaut symétrique de celui qu'on répare ici.
fn construire_volumes(items: &[Item]) -> (Vec<VolumeView>, Vec<Attribution>) {
    let disques = disques_virtuels(items);
    let mut reclames: Vec<String> = Vec::new();

    let volumes: Vec<VolumeView> =
        grouper_nommes(items.iter().filter(|i| i.path.starts_with("space.volume[")))
            .into_iter()
            .map(|nomme| {
                let lettre = ks_collectors::lettre_de_volume(&nomme.name);
                // **Les deux lettres doivent exister ET être égales.** Deux `None` ne
                // se valent pas : un volume qu'on n'a pas su nommer et un disque qu'on
                // n'a pas su placer ne se rapprochent de rien, surtout pas l'un de
                // l'autre.
                let attributions: Vec<Attribution> = disques
                    .iter()
                    .filter(|(volume, _)| match (volume, &lettre) {
                        (Some(v), Some(l)) => v == l,
                        (None, _) | (_, None) => false,
                    })
                    .map(|(_, part)| part.clone())
                    .collect();
                reclames.extend(attributions.iter().map(|a| a.path.clone()));

                // Une somme qui déborderait `u64` n'est pas une somme : elle se dit
                // absente plutôt que repliée. Aucun poste ne l'atteindra, et c'est
                // justement pourquoi personne ne verrait le repli.
                let attributed_bytes = attributions
                    .iter()
                    .try_fold(0_u64, |somme, a| somme.checked_add(a.bytes))
                    .unwrap_or(0);

                let total_path = format!("space.volume[{}].total_bytes", nomme.name);
                let used_path = format!("space.volume[{}].used_bytes", nomme.name);
                let occupancy = occupation(items, &total_path, &used_path, attributed_bytes);

                VolumeView {
                    name: nomme.name,
                    paths: nomme.paths,
                    attributed: lisible::octets(attributed_bytes),
                    attributed_bytes,
                    attributions,
                    occupancy,
                }
            })
            .collect();

    let orphelins = disques
        .into_iter()
        .filter(|(_, part)| !reclames.contains(&part.path))
        .map(|(_, part)| part)
        .collect();

    (volumes, orphelins)
}

/// L'occupation d'un volume, quand ses deux items d'octets ont été relevés.
///
/// `None` dès qu'il en manque un, ou que la taille vaut zéro : un taux sur un
/// dénominateur absent ou nul est ce que le collecteur refuse déjà d'émettre, et
/// ce que l'écran refuse d'inventer.
fn occupation(
    items: &[Item],
    total_path: &str,
    used_path: &str,
    attributed_bytes: u64,
) -> Option<Occupancy> {
    let total_bytes = entier(items, total_path)?;
    let used_bytes = entier(items, used_path)?;
    let percent = used_bytes.checked_mul(100)?.checked_div(total_bytes)?;
    Some(Occupancy {
        used_path: used_path.to_owned(),
        total_path: total_path.to_owned(),
        used: lisible::octets(used_bytes),
        total: lisible::octets(total_bytes),
        used_bytes,
        percent,
        unattributed: Unattributed::rapprocher(used_bytes, attributed_bytes),
    })
}

/// Regroupe les items par nature, dans l'ordre d'affichage.
///
/// Une nature qu'aucun item ne porte **ne produit pas de groupe** : la figure
/// n'a alors pas de segment vide à dessiner, et le tableau pas de ligne à zéro.
/// C'est la règle déjà retenue pour les domaines — un domaine sans item ne
/// figure pas, parce que rien n'y a été collecté.
fn construire_natures(items: &[Item]) -> Vec<NatureGroup> {
    ORDRE_NATURES
        .iter()
        .filter_map(|nature| {
            let paths: Vec<String> = items
                .iter()
                .filter(|i| i.nature == *nature)
                .map(|i| i.path.clone())
                .collect();
            if paths.is_empty() {
                return None;
            }
            let (key, label, meaning) = dit_la_nature(*nature);
            Some(NatureGroup {
                key: key.to_owned(),
                label: label.to_owned(),
                declarable: nature.est_declarable(),
                meaning: meaning.to_owned(),
                paths,
            })
        })
        .collect()
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
/// # Ce qui fait qu'une comparaison a eu lieu
///
/// Ce n'est pas « au moins un item porte un désir » : c'est **qu'un fichier a
/// été confronté**. Les deux se ressemblent et ne disent pas la même chose. Un
/// fichier chargé qui ne déclarerait rien d'observé a bel et bien été confronté,
/// et `ks diff` en publie les décomptes ; à l'inverse, aucun item ne peut porter
/// de désir sans qu'un fichier ait été lu. Faire dépendre l'écran du chargement
/// plutôt que de son résultat aligne exactement les deux surfaces (principe P4).
///
/// # Le verdict est celui que `ks diff` publie
///
/// [`confrontation::verdict_publie`] et non [`Item::verdict`] : un item
/// déclarable dont la lecture a échoué est publié **incomparable**, même si rien
/// ne le déclare. La coque comptait auparavant les verdicts bruts, et rangeait
/// donc les trois exclusions Defender parmi les non contraints là où la CLI les
/// nomme. Deux surfaces, deux chiffres, sur la même machine.
///
/// Le `match` est **exhaustif, sans bras `_`** : ajouter une valeur à
/// [`Verdict`] casse la compilation ici, donc la CI, avant qu'un test
/// s'exécute. Le contributeur est forcé de décider ce que l'écran en fait.
fn construire_derive(items: &[Item], charge: &DesiredStateLoad) -> DriftView {
    let bilan = match charge {
        DesiredStateLoad::Loaded { bilan, .. } => bilan,
        DesiredStateLoad::Missing { .. } => {
            return DriftView::NotComputable {
                why: NotComputable::new(
                    "Aucun fichier d'état désiré n'a été trouvé sur ce poste.",
                    "Il n'y a donc pas zéro écart : il n'y a pas de comparaison. Publier un zéro \
                     laisserait croire qu'une confrontation a eu lieu et n'a rien trouvé.",
                    "Adopter l'état lu comme référence écrit ce fichier, sans qu'aucun formulaire \
                     soit à remplir. Déposé à l'un des emplacements cherchés, il est relu au \
                     relevé suivant : chaque item déclaré sera confronté à sa valeur constatée, \
                     et le résultat dira conforme, écart, ou incomparable quand la lecture a \
                     échoué.",
                    "D2-02 · `ks import` écrit workstation.yaml · aucun des emplacements cherchés \
                     ne le porte",
                ),
                // Aucun fichier n'a été lu : aucun item n'est contraint, et le
                // nombre d'items relevés reste, lui, une mesure réelle.
                unconstrained: items.len(),
            };
        }
        // Le refus vient du noyau, et sa phrase avec : c'est `ks diff` qui
        // l'écrit, et la réécrire ici ferait deux textes pour un même refus.
        // Seules les deux phrases suivantes appartiennent à l'écran, qui doit
        // dire ce que ça implique et ce qu'on peut faire — le noyau, lui, rend
        // la main à un terminal.
        DesiredStateLoad::Unusable {
            message, detail, ..
        } => {
            return DriftView::NotComputable {
                why: NotComputable::new(
                    message,
                    "Rien n'a été comparé, et rien ne l'a été à moitié : un chargement partiel \
                     laisserait lire « aucun écart » sur un fichier dont une part n'aurait pas \
                     été prise en compte.",
                    "Le détail technique ci-dessous nomme la cause, et la ligne quand le fichier \
                     en désigne une. Le fichier corrigé, la confrontation reprend au relevé \
                     suivant.",
                    detail,
                ),
                unconstrained: items.len(),
            };
        }
    };

    let (mut unconstrained, mut compliant, mut incomparable) = (0, 0, 0);
    let mut deviations: Vec<String> = Vec::new();
    for item in items {
        match confrontation::verdict_publie(item) {
            Verdict::NonContraint => unconstrained += 1,
            Verdict::Conforme => compliant += 1,
            Verdict::Ecart => deviations.push(item.path.clone()),
            Verdict::Incomparable { .. } => incomparable += 1,
        }
    }

    DriftView::Computed {
        unconstrained,
        compliant,
        deviations,
        incomparable,
        tolerances: bilan
            .tolerances
            .iter()
            .map(|t| ToleranceView {
                path: t.item.clone(),
                reason: t.raison.clone(),
                expires: t.echeance,
                status: ToleranceStatus::from(&t.qualification),
            })
            .collect(),
    }
}

/// Ce que l'anneau de posture dit, selon qu'une référence est chargée ou non.
///
/// **Le texte dépend du chargement, et il le faut** : « aucun état de référence
/// n'est chargé sur ce poste » était vrai tant que la coque n'en lisait aucun.
/// Le jour où elle en lit un, la même phrase s'affiche à côté du chemin du
/// fichier lu, et se contredit à l'écran. Une référence chargée ne rend pas pour
/// autant la posture calculable : il y manque un barème, pas une lecture.
fn construire_posture(charge: &DesiredStateLoad) -> NotComputable {
    match charge {
        DesiredStateLoad::Loaded { .. } => NotComputable::new(
            "Keystone ne calcule aucun indicateur composite de posture.",
            "Un tel indicateur pèse des items les uns contre les autres, et ce barème n'existe \
             pas. Affiché quand même, il donnerait une note que personne ne saurait déplier en \
             ses composantes exactes — c'est-à-dire une décoration.",
            "La confrontation, elle, est calculée : l'écran Dérive la publie item par item, avec \
             les tolérances qui annotent les écarts.",
            "P6 · D2-02 · référence chargée, aucun barème de pondération défini",
        ),
        DesiredStateLoad::Missing { .. } | DesiredStateLoad::Unusable { .. } => NotComputable::new(
            "Aucun état de référence n'est chargé sur ce poste.",
            "Une posture composite se calcule contre une référence. Sans référence, tout chiffre \
             affiché ici serait une convention, pas une mesure — et un indicateur composite qu'on \
             ne sait pas déplier en ses composantes exactes est une décoration.",
            "Adopter l'état lu comme référence produira le fichier d'état désiré. La confrontation \
             deviendra alors possible, et dépliable item par item.",
            "P6 · D2-02 · workstation.yaml non chargé",
        ),
    }
}

/// D'où vient l'état désiré, tel que l'écran doit le dire.
///
/// Le `match` est **exhaustif sans bras `_`** : une issue de chargement ajoutée
/// casse la compilation ici, donc oblige à décider ce que l'écran en montre.
fn construire_source(charge: &DesiredStateLoad) -> DesiredStateView {
    match charge {
        DesiredStateLoad::Loaded { path, bilan } => DesiredStateView::Loaded {
            path: path.clone(),
            declared: bilan.declarees,
            not_observed: bilan.non_observes.clone(),
        },
        DesiredStateLoad::Missing { searched } => DesiredStateView::Missing {
            searched: searched.clone(),
        },
        DesiredStateLoad::Unusable { path, .. } => {
            DesiredStateView::Unusable { path: path.clone() }
        }
    }
}

impl Cockpit {
    /// Assemble l'état affichable à partir des items relevés.
    ///
    /// Aucun appel système ici : la fonction ne reçoit que la donnée, ce qui la
    /// rend testable sur des inventaires fabriqués — y compris l'inventaire
    /// vide, qui est le cas où l'invention serait la plus tentante. Le
    /// chargement du fichier d'état désiré arrive de la même façon, déjà fait :
    /// ses trois issues s'éprouvent donc sans toucher au disque.
    #[must_use]
    pub fn build(
        items: &[Item],
        machine: &str,
        collected_at: &str,
        html: String,
        charge: &DesiredStateLoad,
    ) -> Self {
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

        let (volumes, unplaced_disks) = construire_volumes(items);

        Self {
            natures: construire_natures(items),
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
            volumes,
            unplaced_disks,
            distros: grouper_nommes(
                items
                    .iter()
                    .filter(|i| i.path.starts_with("virtualization.wsl[")),
            ),
            posture: construire_posture(charge),
            desired_state: construire_source(charge),
            drift: construire_derive(items, charge),
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
                "Le relevé porte l'occupation de chaque volume en octets, et n'en attribue que \
                 les disques virtuels WSL — pas le reste de ce qui l'occupe.",
                "La part non attribuée reste donc sans nom : ni les caches de chaînes d'outils, \
                 ni les instantanés, ni les données de travail ne sont mesurés, et aucune \
                 quantité récupérable n'est avancée. Ce qui est attribué se lit par volume, avec \
                 le détail par distribution, et ce qui ne l'est pas est nommé comme tel plutôt \
                 que fondu dans le total.",
                "L'attribution par consommateur, avec la quarantaine qui la rend réversible, \
                 arrive au domaine de l'espace. Elle nommera ce que la part non attribuée \
                 recouvre aujourd'hui.",
                "D4 · seuls les disques WSL sont rapprochés d'un volume",
            ),
            virtual_machines: NotComputable::new(
                "Les machines virtuelles Hyper-V ne sont pas relevées.",
                "Cet écran ne montre que les distributions WSL : une machine virtuelle, même en \
                 cours d'exécution, n'y figure pas.",
                "Le collecteur de virtualisation couvrira Hyper-V au même titre que WSL.",
                "D9 · Hyper-V hors du collecteur virtualisation",
            ),
            observation_series: NotComputable::new(
                "La coque lit un relevé, pas une série d'observations.",
                "Le magasin encode des intervalles : une valeur a tenu de telle date à telle \
                 date. Un intervalle demande donc deux bornes, et cette lecture n'en donne \
                 qu'une. Tracer une frise sur une seule borne donnerait à une absence \
                 l'allure d'une mesure, et c'est exactement ce que la frise sert à éviter.",
                "Consigner chaque lecture dans le magasin local ouvre la série ; un sondage \
                 régulier lui donne sa seconde borne. La frise se lira alors chemin par \
                 chemin, et chaque bande dira entre quelles dates la valeur a tenu.",
                "D2-04 · ks scan --record alimente le magasin · table observation non lue par \
                 la coque",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ks_cli::confrontation::Tolerance;
    use ks_core::{Nature, Provenance};

    use super::*;

    /// Le frontend, tel qu'il est embarqué dans le binaire. Les figures vivent
    /// dans ces trois fichiers, et les barrières qui les gouvernent aussi.
    const INDEX: &str = include_str!("../web/index.html");
    const SCRIPT: &str = include_str!("../web/app.js");
    const STYLE: &str = include_str!("../web/app.css");

    fn item(chemin: &str, domaine: Domain, valeur: ItemValue) -> Item {
        nature_item(chemin, domaine, valeur, Nature::Reglage)
    }

    /// Le même, quand c'est la nature qui est le sujet du test.
    fn nature_item(chemin: &str, domaine: Domain, valeur: ItemValue, nature: Nature) -> Item {
        Item {
            path: chemin.to_owned(),
            domain: domaine,
            nature,
            desired: None,
            observed: valeur,
            observed_at: chrono::Utc::now(),
            provenance: Provenance::Observed,
            purpose: "Une finalité.".to_owned(),
            risk: "Un risque.".to_owned(),
            reference: None,
        }
    }

    /// Le cockpit d'une machine dont aucun état désiré n'a été trouvé.
    fn construire(items: &[Item]) -> Cockpit {
        avec_chargement(
            items,
            &DesiredStateLoad::Missing {
                searched: vec![
                    "C:\\Utilisateurs\\essai\\AppData\\Local\\Keystone\\workstation.yaml"
                        .to_owned(),
                    "workstation.yaml".to_owned(),
                ],
            },
        )
    }

    fn avec_chargement(items: &[Item], charge: &DesiredStateLoad) -> Cockpit {
        Cockpit::build(
            items,
            "POSTE-DE-TEST",
            "2026-08-03T12:00:00Z",
            String::new(),
            charge,
        )
    }

    /// Une date civile littérale. Aucune horloge dans un chemin asserté.
    fn jour(litteral: &str) -> chrono::NaiveDate {
        litteral.parse().expect("date littérale valide")
    }

    /// Une tolérance déjà qualifiée, telle que le noyau la rendrait.
    fn tolerance(item: &str, echeance: &str, qualification: Qualification) -> Tolerance {
        Tolerance {
            item: item.to_owned(),
            raison: "pilote du scanner du labo".to_owned(),
            echeance: jour(echeance),
            qualification,
        }
    }

    /// Un chargement abouti, avec les tolérances qu'il a qualifiées.
    fn confronte(tolerances: Vec<Tolerance>) -> DesiredStateLoad {
        DesiredStateLoad::Loaded {
            path: "C:\\essai\\workstation.yaml".to_owned(),
            bilan: Confrontation {
                declarees: 1,
                tolerances,
                non_observes: Vec::new(),
            },
        }
    }

    /// Un item contraint dont le constaté diffère du déclaré : un écart.
    fn en_ecart(chemin: &str) -> Item {
        let mut item = item(chemin, Domain::Security, ItemValue::Bool(false));
        item.desired = Some(ItemValue::Bool(true));
        item
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
        assert!(
            vide.natures.is_empty(),
            "aucune nature ne produit un segment vide : une figure sans items ne se dessine pas"
        );
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
        // transposé à l'écran : sans fichier d'état désiré, il n'y a pas zéro
        // écart, il n'y a pas de comparaison. La barrière n'est pas une
        // convention d'affichage — le champ n'existe pas dans ce que la page
        // reçoit.
        let sans_fichier = construire(&[
            item("security.a", Domain::Security, ItemValue::Bool(true)),
            item("security.b", Domain::Security, ItemValue::Bool(false)),
        ]);
        let json = serde_json::to_string(&sans_fichier.drift).expect("structure simple");

        assert!(
            !json.contains("deviation"),
            "un décompte d'écarts est publié alors qu'aucune comparaison n'a eu lieu : {json}"
        );
        assert!(!json.contains("compliant"), "{json}");
        assert!(!json.contains("tolerance"), "{json}");
        assert!(json.contains("notComputable"), "{json}");
        assert!(
            json.contains("\"unconstrained\":2"),
            "le nombre d'items lus reste une mesure réelle : {json}"
        );

        // Et un fichier TROUVÉ MAIS REFUSÉ ne bascule pas davantage : un
        // chargement qui échoue n'est pas une comparaison qui ne trouve rien.
        let refuse = avec_chargement(
            &[en_ecart("security.a")],
            &DesiredStateLoad::Unusable {
                path: "C:\\essai\\workstation.yaml".to_owned(),
                message: "Le fichier n'a pas pu être relu en entier.".to_owned(),
                detail: "mapping values are not allowed in this context at line 12".to_owned(),
            },
        );
        let json = serde_json::to_string(&refuse.drift).expect("structure simple");
        assert!(
            !json.contains("deviation") && json.contains("notComputable"),
            "un fichier refusé publie des décomptes : {json}"
        );

        // Falsification : dès qu'un fichier est confronté, la comparaison a un
        // sens, et les décomptes reviennent. Sans ce second cas, la barrière
        // pourrait tenir en ne publiant jamais rien.
        let compare = avec_chargement(
            &[
                en_ecart("security.a"),
                item("security.b", Domain::Security, ItemValue::Bool(false)),
            ],
            &confronte(Vec::new()),
        );
        let json = serde_json::to_string(&compare.drift).expect("structure simple");
        assert!(json.contains("\"deviations\":[\"security.a\"]"), "{json}");
        assert!(json.contains("\"unconstrained\":1"), "{json}");
        assert!(!json.contains("notComputable"), "{json}");
    }

    #[test]
    fn un_ecart_tolere_reste_publie_en_ecart() {
        // LA RÈGLE DE CONCEPTION QUE CET ÉCRAN NE PEUT PAS ENFREINDRE. Le
        // verdict dit le fait, la tolérance dit la politique. Ranger l'item
        // toléré ailleurs que dans les écarts le ferait sortir de l'écran,
        // alors que c'est précisément ce qu'on a choisi de garder sous les yeux
        // jusqu'à une date — et à l'échéance il faudrait l'y faire réapparaître,
        // alors qu'ici il n'aura jamais cessé d'être là.
        //
        // Le contrôle est un ÉCART DE DÉCOMPTE : les mêmes items, confrontés
        // avec et sans la tolérance, doivent produire exactement la même
        // dérive, tolérances mises à part. C'est la seule façon de prouver
        // qu'elle n'a rien déplacé, plutôt que déplacé deux fois.
        let items = [
            en_ecart("security.services.windefend.startup"),
            item("security.b", Domain::Security, ItemValue::Bool(false)),
        ];
        let avec = avec_chargement(
            &items,
            &confronte(vec![tolerance(
                "security.services.windefend.startup",
                "2026-10-15",
                Qualification::EnVigueur { jours_restants: 59 },
            )]),
        );
        let sans = avec_chargement(&items, &confronte(Vec::new()));

        let (
            DriftView::Computed {
                deviations: avec_d,
                compliant: avec_c,
                incomparable: avec_i,
                unconstrained: avec_n,
                tolerances,
            },
            DriftView::Computed {
                deviations: sans_d,
                compliant: sans_c,
                incomparable: sans_i,
                unconstrained: sans_n,
                ..
            },
        ) = (&avec.drift, &sans.drift)
        else {
            panic!("un fichier confronté doit produire des décomptes");
        };

        assert_eq!(
            (avec_d, avec_c, avec_i, avec_n),
            (sans_d, sans_c, sans_i, sans_n),
            "la tolérance a déplacé un item d'une catégorie à une autre"
        );
        assert_eq!(
            avec_d.iter().map(String::as_str).collect::<Vec<_>>(),
            ["security.services.windefend.startup"],
            "l'écart toléré a disparu de la liste des écarts"
        );
        assert_eq!(tolerances.len(), 1);
        assert_eq!(tolerances[0].path, "security.services.windefend.startup");

        // Et la tolérance voyage avec son échéance ET ses jours restants : une
        // liste sans eux se lirait comme « toléré pour toujours ».
        let json = serde_json::to_string(&avec.drift).expect("structure simple");
        assert!(json.contains("\"status\":\"inForce\""), "{json}");
        assert!(json.contains("\"daysLeft\":59"), "{json}");
        assert!(json.contains("\"expires\":\"2026-10-15\""), "{json}");
    }

    #[test]
    fn les_trois_qualifications_qui_ne_couvrent_rien_arrivent_a_lecran() {
        // Une échéance dépassée, une tolérance devenue sans objet et un chemin
        // disparu sont trois manières dont un état désiré pourrit en silence
        // (D2-07), et le silence est justement la cause première. Les taire à
        // l'écran les rendrait invisibles là où `ks diff` les nomme.
        let ecran = avec_chargement(
            &[en_ecart("security.a")],
            &confronte(vec![
                tolerance(
                    "security.a",
                    "2026-08-16",
                    Qualification::Echue { depuis_jours: 1 },
                ),
                tolerance("security.b", "2026-10-15", Qualification::SansObjet),
                tolerance("security.fantome", "2026-10-15", Qualification::NonObservee),
            ]),
        );
        let json = serde_json::to_string(&ecran.drift).expect("structure simple");

        for (attendu, absent) in [
            ("\"status\":\"expired\"", "une échéance dépassée"),
            ("\"daysSince\":1", "le nombre de jours écoulés"),
            ("\"status\":\"moot\"", "une tolérance sans objet"),
            ("\"status\":\"notObserved\"", "un chemin disparu"),
        ] {
            assert!(json.contains(attendu), "{absent} ne remonte pas : {json}");
        }

        // La raison écrite dans le fichier voyage avec : sans elle, l'écran
        // dirait qu'une ligne est à revoir sans dire ce qu'elle voulait dire.
        let DriftView::Computed { tolerances, .. } = &ecran.drift else {
            unreachable!("un fichier confronté produit des décomptes")
        };
        assert!(tolerances.iter().all(|t| !t.reason.is_empty()));
        // Et l'ordre est celui du fichier : c'est celui dans lequel on les
        // relira, et le réordonner obligerait à chercher.
        assert_eq!(
            tolerances
                .iter()
                .map(|t| t.path.as_str())
                .collect::<Vec<_>>(),
            ["security.a", "security.b", "security.fantome"]
        );
    }

    #[test]
    fn la_coque_compte_les_verdicts_comme_ks_diff_les_publie() {
        // LA BARRIÈRE DE PARITÉ (P4). `Item::verdict()` range un item
        // déclarable dont la lecture a échoué parmi les NON CONTRAINTS ; `ks
        // diff` le publie INCOMPARABLE, parce que `ks import` vient de dire
        // qu'il n'a pas su le lire. La coque comptait le premier, la CLI publie
        // le second : deux chiffres pour la même machine.
        //
        // Éprouvée par falsification : revenir à `item.verdict()` dans
        // `construire_derive` fait échouer ce test, l'illisible retombant parmi
        // les non contraints.
        let illisible = item(
            "security.defender.exclusions.paths",
            Domain::Security,
            ItemValue::illisible("accès refusé sans élévation"),
        );
        assert_eq!(
            illisible.verdict(),
            Verdict::NonContraint,
            "le verdict brut de cet item doit rester non contraint, \
             sans quoi le test ne prouve plus rien"
        );

        let ecran = avec_chargement(&[illisible, en_ecart("security.a")], &confronte(Vec::new()));
        let DriftView::Computed {
            unconstrained,
            incomparable,
            deviations,
            ..
        } = &ecran.drift
        else {
            unreachable!("un fichier confronté produit des décomptes")
        };
        assert_eq!(
            (*unconstrained, *incomparable, deviations.len()),
            (0, 1, 1),
            "un aveu de lecture est rangé parmi les non contraints, \
             là où `ks diff` le nomme incomparable"
        );
    }

    #[test]
    fn lecran_dit_toujours_quel_fichier_il_a_lu_ou_cherche() {
        // Principe P6 : un état qu'on ne peut pas expliquer n'est pas
        // affichable. « Aucune comparaison » sans dire quel fichier était
        // attendu, ni où on l'a cherché, n'explique rien — et laisse
        // l'utilisateur sans le moindre geste à faire.
        let cherches = vec![
            "C:\\Utilisateurs\\essai\\AppData\\Local\\Keystone\\workstation.yaml".to_owned(),
            "workstation.yaml".to_owned(),
        ];
        let DesiredStateView::Missing { searched } = &construire(&[]).desired_state else {
            panic!("un chargement manquant doit publier les chemins cherchés")
        };
        assert_eq!(
            searched, &cherches,
            "les deux emplacements se disent, dans l'ordre"
        );

        // Trouvé : c'est le chemin RÉELLEMENT lu qui s'affiche, jamais le
        // premier de la liste.
        let charge = confronte(Vec::new());
        let lu = avec_chargement(&[en_ecart("security.a")], &charge);
        let DesiredStateView::Loaded {
            path,
            declared,
            not_observed,
        } = &lu.desired_state
        else {
            panic!("un chargement abouti doit publier son chemin")
        };
        assert_eq!(path, "C:\\essai\\workstation.yaml");
        assert_eq!(*declared, 1);
        assert!(not_observed.is_empty());

        // Refusé : le chemin s'affiche aussi. Un fichier qu'on n'a pas su lire
        // sans dire lequel est la pire des trois issues à afficher.
        let refuse = avec_chargement(
            &[],
            &DesiredStateLoad::Unusable {
                path: "C:\\essai\\workstation.yaml".to_owned(),
                message: "Le fichier n'a pas pu être relu en entier.".to_owned(),
                detail: "did not find expected key at line 9".to_owned(),
            },
        );
        assert!(matches!(
            &refuse.desired_state,
            DesiredStateView::Unusable { path } if path == "C:\\essai\\workstation.yaml"
        ));
    }

    #[test]
    fn un_echec_de_chargement_garde_son_detail_technique_separe_de_sa_phrase() {
        // La phrase de `ErreurDeLecture::Document` PROMET que « le détail
        // technique nomme la ligne en cause » — et ce détail vit dans un champ
        // que l'affichage de l'erreur n'interpole pas. Le recopier par
        // `to_string()` seul le perdrait en promettant le contraire, ce qui est
        // pire que de ne rien promettre.
        //
        // Éprouvée par falsification : remplacer `detail_technique(e)` par
        // `e.to_string()` dans `DesiredStateLoad::refuse` fait échouer ce test.
        let e = ErreurDeChargement::Lecture(ErreurDeLecture::Document {
            detail: "mapping values are not allowed in this context at line 12 column 5".to_owned(),
        });
        let DesiredStateLoad::Unusable {
            message, detail, ..
        } = DesiredStateLoad::refuse("C:\\essai\\workstation.yaml".to_owned(), &e)
        else {
            unreachable!("`refuse` ne construit que des refus")
        };
        assert!(
            message.contains("Le détail technique nomme la ligne en cause"),
            "la phrase a perdu sa promesse : {message}"
        );
        assert!(
            detail.contains("line 12"),
            "la promesse n'est pas tenue : le détail ne nomme aucune ligne — {detail}"
        );
        assert!(
            !message.contains("line 12"),
            "le détail technique est recollé dans la phrase : {message}"
        );

        // ET LES DEUX ARRIVENT À L'ÉCRAN, toujours séparés. Un détail technique
        // calculé puis jamais publié est un détail perdu : l'utilisateur lirait
        // « la ligne en cause est nommée » sans jamais voir de ligne.
        let ecran = avec_chargement(
            &[],
            &DesiredStateLoad::refuse("C:\\essai\\workstation.yaml".to_owned(), &e),
        );
        let DriftView::NotComputable { why, .. } = &ecran.drift else {
            unreachable!("un fichier refusé ne se compare à rien")
        };
        assert_eq!(
            why.happened, message,
            "la phrase du noyau n'arrive pas à l'écran"
        );
        assert_eq!(
            why.detail, detail,
            "le détail technique n'arrive pas à l'écran"
        );
        assert_ne!(
            why.happened, why.detail,
            "la phrase et le détail sont redevenus une seule chaîne"
        );
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
            ("observation_series", &c.observation_series),
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
            &c.observation_series,
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
                "space.volume[C:\\].used_bytes",
                Domain::Space,
                ItemValue::Int(59),
            ),
            item(
                "space.volume[D:\\].used_bytes",
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
    fn chaque_item_releve_tombe_dans_exactement_une_nature() {
        // LA BARRIÈRE DU TOTAL. `dit_la_nature` casse la compilation quand une
        // variante s'ajoute à `Nature` ; `ORDRE_NATURES`, lui, peut l'oublier
        // en silence, et la figure perdrait alors un segment sans que rien ne
        // le signale — une part-à-tout dont les parts ne font plus le tout est
        // exactement le mensonge qu'une barre empilée sait le mieux raconter.
        //
        // Le contrôle est donc arithmétique : la somme des groupes vaut le
        // nombre d'items relevés, et aucun chemin n'est compté deux fois.
        let c = construire(&[
            nature_item(
                "a",
                Domain::Security,
                ItemValue::Bool(true),
                Nature::Reglage,
            ),
            nature_item(
                "b",
                Domain::Security,
                ItemValue::Bool(true),
                Nature::Objectif,
            ),
            nature_item("c", Domain::Space, ItemValue::Int(59), Nature::Mesure),
            nature_item("d", Domain::Inventory, ItemValue::Int(12), Nature::Constat),
            nature_item("e", Domain::Inventory, ItemValue::Int(12), Nature::Constat),
        ]);

        let total: usize = c.natures.iter().map(|n| n.paths.len()).sum();
        assert_eq!(
            total, c.item_count,
            "la somme des natures ne fait plus le compte des items : \
             la figure de répartition perdrait une part sans le dire"
        );

        let mut vus: Vec<&str> = c
            .natures
            .iter()
            .flat_map(|n| n.paths.iter().map(String::as_str))
            .collect();
        vus.sort_unstable();
        let distincts = vus.len();
        vus.dedup();
        assert_eq!(distincts, vus.len(), "un item est compté dans deux natures");

        // Et chaque chemin d'une part doit exister dans la table des relevés :
        // la longueur d'un segment est alors, littéralement, un décompte
        // d'items affichables.
        for chemin in &vus {
            assert!(
                c.readings.contains_key(*chemin),
                "« {chemin} » est compté dans une figure sans être un relevé"
            );
        }

        assert_eq!(
            c.natures
                .iter()
                .map(|n| (n.key.as_str(), n.paths.len(), n.declarable))
                .collect::<Vec<_>>(),
            vec![
                ("reglage", 1, true),
                ("objectif", 1, true),
                ("mesure", 1, false),
                ("constat", 2, false),
            ],
            "l'ordre d'affichage met les déclarables en tête, et le drapeau \
             `declarable` est ce qui porte le propos de la figure"
        );
    }

    #[test]
    fn aucune_figure_ne_publie_un_nombre_ecrit_a_la_main() {
        // LA BARRIÈRE CENTRALE DES FIGURES, et celle que l'ADR-0013 annonçait
        // manquante : « le jour où un écran calculera une valeur au lieu de la
        // lire, aucun test existant ne le verra ».
        //
        // Une part de figure ne reçoit jamais un nombre. Elle compte une liste
        // de chemins, ou elle lit un décompte que le noyau a établi par un
        // `match` exhaustif. Il n'existe pas de troisième source, et la
        // longueur d'un segment est donc toujours un décompte d'items réels.
        //
        // Le contrôle porte sur les DÉCLARATIONS et saute les commentaires :
        // sa première version refusait aussi la ligne d'app.js qui explique ce
        // qu'elle interdit, ce qui aurait forcé à taire la raison pour
        // satisfaire la barrière. Le défaut est déjà arrivé une fois ici, sur
        // l'anneau de focus.
        //
        // Éprouvée par falsification : remplacer `chemins.length` par un
        // nombre fait échouer ce test, en montrant la ligne fautive.
        const SOURCES: [&str; 2] = ["chemins.length", "etat.drift["];
        let mut publications = 0;
        for ligne in SCRIPT.lines() {
            let code = ligne.trim_start();
            if code.starts_with('*') || code.starts_with("/*") || code.starts_with("//") {
                continue;
            }
            let Some(pos) = code.find("valeur:") else {
                continue;
            };
            publications += 1;
            let expression = code[pos + "valeur:".len()..].trim_start();
            assert!(
                SOURCES.iter().any(|s| expression.starts_with(s)),
                "une figure publie « {expression} » : une part ne porte que le \
                 nombre de ses chemins, ou un décompte du noyau — jamais un \
                 nombre écrit ici"
            );
        }
        assert!(
            publications >= 2,
            "seulement {publications} valeur(s) de figure examinée(s) : \
             la barrière ne barre plus rien"
        );

        // Et le tableau équivalent n'est pas facultatif : toute figure est
        // suivie du tableau qui redit ses valeurs, avant la figure suivante ou
        // la fin de l'écran. Une valeur lisible au survol l'est aussi sans.
        let mut figures = 0;
        for (rang, apres) in INDEX.split("<figure class=\"fig\"").enumerate().skip(1) {
            figures += 1;
            let fin = apres
                .find("</section>")
                .unwrap_or(apres.len())
                .min(apres.find("<figure").unwrap_or(apres.len()));
            assert!(
                apres[..fin].contains("cadre-table"),
                "la figure n° {rang} n'a pas d'équivalent tableau sur son écran : \
                 l'infobulle deviendrait le seul accès à ses valeurs"
            );
        }
        assert!(
            figures >= 3,
            "seulement {figures} figure(s) examinée(s) : la barrière ne barre plus rien"
        );
    }

    #[test]
    fn chaque_etat_de_tolerance_porte_une_icone_et_un_libelle() {
        // LA BARRIÈRE QUI TIENT LE CONTRASTE FORCÉ SANS UNE RÈGLE DE PLUS.
        //
        // `forced-colors: active` calcule `background`, `background-image` et
        // `box-shadow` à `none` — c'est le texte normatif de CSS Color Adjust
        // Level 1 —, mais il FORCE la couleur d'un trait sans le supprimer. Une
        // pastille sans fond garde donc la forme de son icône, et son libellé
        // dit l'état en toutes lettres. Les trois teintes se confondent alors en
        // une seule sans qu'aucun sens se perde, parce qu'aucun n'y reposait.
        //
        // Le contrôle part des QUATRE QUALIFICATIONS DU NOYAU, sérialisées ici
        // même : renommer une variante, ou en ajouter une, fait échouer ce test
        // en nommant le jeton que la page ne sait pas rendre. Une liste écrite à
        // la main ne détecterait jamais ce qu'on a oublié d'y mettre.
        //
        // Éprouvée par falsification : retirer l'icône d'un des quatre états, ou
        // rendre l'un d'eux sans phrase, fait échouer ce test.
        for statut in [
            ToleranceStatus::InForce { days_left: 0 },
            ToleranceStatus::Expired { days_since: 0 },
            ToleranceStatus::Moot,
            ToleranceStatus::NotObserved,
        ] {
            let publie = serde_json::to_value(&statut).expect("structure simple");
            let jeton = publie["status"].as_str().expect("le statut est étiqueté");

            let apres = SCRIPT
                .split_once(&format!("  {jeton}: {{"))
                .unwrap_or_else(|| panic!("app.js ne sait pas rendre la qualification « {jeton} »"))
                .1;
            let bloc = &apres[..apres.find("\n  },").unwrap_or(apres.len())];

            let icone = valeur_dattribut_js(bloc, "icone").unwrap_or_else(|| {
                panic!("« {jeton} » n'a pas d'icône : la couleur porterait seule son sens")
            });
            assert!(
                INDEX.contains(&format!("id=\"{icone}\"")),
                "« {jeton} » désigne l'icône « {icone} », absente du sprite : \
                 la pastille se rendrait vide"
            );
            assert!(
                bloc.contains("=>"),
                "« {jeton} » n'a aucun libellé : une icône seule n'est pas un état"
            );
            let classe = valeur_dattribut_js(bloc, "classe")
                .unwrap_or_else(|| panic!("« {jeton} » n'a pas de classe"));

            // ET LA CLASSE NE DÉPEND DE RIEN QUE LE CONTRASTE FORCÉ EFFACE.
            for nom in classe.split_whitespace() {
                let regle = STYLE
                    .split_once(&format!(".{nom} {{"))
                    .unwrap_or_else(|| panic!("« .{nom} » n'existe pas dans la feuille"))
                    .1;
                let corps = &regle[..regle.find('}').unwrap_or(regle.len())];
                for efface in ["background", "box-shadow"] {
                    assert!(
                        !corps.contains(efface),
                        "« .{nom} » repose sur « {efface} », que le contraste forcé \
                         calcule à `none` : « {corps} »"
                    );
                }
            }
        }

        // L'icône a une taille propre : sans elle, un `<svg>` sans attributs se
        // rend à sa taille par défaut, et emporte la ligne du tableau.
        let regle = STYLE
            .split_once(".tol .ic {")
            .expect("l'icône des pastilles n'a plus de règle")
            .1;
        assert!(
            regle[..regle.find('}').unwrap_or(regle.len())].contains("width:"),
            "l'icône des pastilles n'a plus de taille"
        );

        // Et le vert reste interdit ici : une tolérance en vigueur n'est pas un
        // état sain, c'est un écart qu'on garde sous les yeux jusqu'à une date.
        let vigueur = STYLE
            .split_once(".tol-vigueur {")
            .expect("l'état « en vigueur » n'a plus de règle")
            .1;
        let corps = &vigueur[..vigueur.find('}').unwrap_or(vigueur.len())];
        for vert in ["--vital", "--s5", "--s1"] {
            assert!(
                !corps.contains(vert),
                "une tolérance en vigueur prend « {vert} » : \
                 tolérer n'est pas rendre sain — « {corps} »"
            );
        }
    }

    /// La valeur d'une propriété littérale d'app.js : `icone: "ks-tolere"`.
    fn valeur_dattribut_js<'a>(bloc: &'a str, propriete: &str) -> Option<&'a str> {
        let motif = format!("{propriete}: \"");
        let pos = bloc.find(&motif)? + motif.len();
        let fin = bloc[pos..].find('"')?;
        Some(&bloc[pos..pos + fin])
    }

    #[test]
    fn chaque_selecteur_du_contraste_force_existe_dans_linterface() {
        // Une règle de contraste forcé qui vise une classe absente du balisage
        // est de la décoration : elle rassure en revue et ne protège rien. Le
        // défaut a déjà été trouvé dans ce dépôt, et il ne se voit pas — le
        // bloc a l'air complet, et personne ne rend l'interface en contraste
        // imposé pour vérifier.
        //
        // Les classes des figures sont posées par app.js et non par
        // index.html : le contrôle regarde donc les deux fichiers, sans quoi
        // il refuserait précisément ce qu'il doit protéger.
        //
        // Éprouvée par falsification : ajouter une règle sur une classe qui
        // n'existe nulle part fait échouer ce test, en la nommant.
        // **Le motif designe la REGLE, pas le nom.** Chercher la chaine nue
        // « forced-colors: active » trouve aussi le premier COMMENTAIRE qui la
        // cite, et deplace alors l'analyse vers un bloc qui n'en est pas un :
        // un test sans rapport echoue, et la cause est introuvable. C'est
        // arrive pendant le lot des tolerances.
        let debut = STYLE
            .find("@media (forced-colors: active) {")
            .expect("le bloc de contraste forcé a disparu");
        let mut profondeur = 0_i32;
        let mut fin = debut;
        for (decalage, caractere) in STYLE[debut..].char_indices() {
            match caractere {
                '{' => profondeur += 1,
                '}' => {
                    profondeur -= 1;
                    if profondeur == 0 {
                        fin = debut + decalage;
                        break;
                    }
                }
                _ => {}
            }
        }
        let bloc = &STYLE[debut..fin];

        let mut classes: Vec<&str> = Vec::new();
        let mut reste = bloc;
        while let Some(pos) = reste.find('.') {
            let apres = &reste[pos + 1..];
            let longueur = apres
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(apres.len());
            let classe = &apres[..longueur];
            // Un nombre décimal — `1.15px` — n'est pas un sélecteur.
            if !classe.is_empty()
                && !classe.starts_with(|c: char| c.is_ascii_digit())
                && !classes.contains(&classe)
            {
                classes.push(classe);
            }
            reste = &apres[longueur..];
        }

        assert!(
            classes.len() >= 12,
            "seulement {} classe(s) examinée(s) dans le bloc de contraste forcé : \
             la barrière ne barre plus rien",
            classes.len()
        );
        for classe in classes {
            assert!(
                jeton_present(INDEX, classe) || jeton_present(SCRIPT, classe),
                "le bloc de contraste forcé vise « .{classe} », qui n'existe ni \
                 dans le balisage ni dans le script : la règle ne protège rien"
            );
        }
    }

    /// Le nom de classe figure-t-il en entier dans la source ?
    ///
    /// En entier, et pas en fragment : sans cette borne, `.fig-s1` serait
    /// « trouvé » dans `fig-s15` s'il en existait un, et la barrière laisserait
    /// passer précisément le nom qu'elle croit vérifier.
    fn jeton_present(source: &str, classe: &str) -> bool {
        let bordure = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        let mut reste = source;
        let mut consomme = 0;
        while let Some(pos) = reste.find(classe) {
            let debut = consomme + pos;
            let fin = debut + classe.len();
            let avant = source[..debut].chars().next_back();
            let apres = source[fin..].chars().next();
            if !avant.is_some_and(bordure) && !apres.is_some_and(bordure) {
                return true;
            }
            reste = &reste[pos + classe.len()..];
            consomme = fin;
        }
        false
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

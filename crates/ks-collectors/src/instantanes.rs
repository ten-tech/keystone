//! Disponibilité des mécanismes d'instantané, relevée pendant le scan
//! (D3-07, ADR-0021 décision n° 5).
//!
//! L'ADR est citée sans lien, comme partout ailleurs dans ce crate : un chemin
//! relatif vers `docs/` se rendrait en lien mort dans la documentation générée,
//! qui ne vit pas à côté du dépôt.
//!
//! ## Ce que ce module répond, et pourquoi maintenant
//!
//! P3 ne dit pas que la réversibilité est souhaitable : il en fait un **critère
//! d'admission**. Or le filet dont une convergence dépendra n'existe pas
//! forcément sur la machine. Sur le poste de référence, la protection système
//! est désactivée, donc aucun point de restauration ne peut être pris.
//!
//! Découvrir cela au moment d'appliquer serait doublement fautif : trop tard, et
//! au mauvais endroit. La disponibilité de chaque mécanisme est donc un **fait
//! de la machine comme un autre**, relevé au scan et publié en
//! [`Nature::Constat`].
//!
//! ## Ce que « écart affiché par `ks diff` » vaut aujourd'hui, mesuré
//!
//! L'ADR-0021 écrit que « l'absence de filet devient un écart affiché par
//! `ks diff` ». Ce n'est pas ce que ces items produisent, et il vaut mieux
//! l'écrire ici que le laisser découvrir : un `Constat` n'a pas d'état désiré,
//! donc `ks import` ne le verse pas dans `workstation.yaml`, donc `ks diff` le
//! range parmi les **non contraints**. Mesuré le 2026-08-24 sur le poste de
//! référence, `ks diff --domain updates` rend « écarts 0 · conformes 0 ·
//! incomparables 0 · non contraints 4 ».
//!
//! La nature n'est pas le défaut. Rendre la disponibilité déclarable ferait
//! entrer dans le fichier d'état désiré une ligne qu'aucun verbe ne peut
//! satisfaire, puisque **Keystone n'active jamais la protection système de
//! lui-même** : ce serait une déclaration qui rassure et n'agit pas, soit
//! exactement l'état que l'ADR-0020 nomme « le pire des trois possibles ».
//!
//! Ce qui manque est ailleurs, et ce lot ne le décide pas : l'endroit où
//! l'absence de filet s'oppose à une action est le **bandeau du plan**
//! (ADR-0021, décision n° 3 — « un plan qui ne dispose que du rang 2 pour une
//! action donnée doit le dire dans le bandeau »), et ce bandeau n'existe qu'en
//! phase 2. D'ici là, le fait est relevé, publié et explicable ; il n'est pas
//! encore opposé à un plan.
//!
//! ## Ce module n'active rien, et ne le proposera pas
//!
//! **Keystone n'active jamais la protection système de lui-même** (ADR-0021,
//! décision n° 5). Réserver de l'espace disque sur le volume système est une
//! décision de l'utilisateur, dont le coût est réel et durable. Le filet
//! manquant se dit ; il ne se répare pas d'initiative. Ce module est un
//! collecteur : il lit le registre, il n'écrit nulle part, et sa signature
//! `&self` sans canal d'effet en est la barrière.
//!
//! ## La capacité, jamais le nom de l'édition
//!
//! Trois lectures du même fait donnent trois chaînes différentes sur le poste de
//! référence : `Win32_OperatingSystem.Caption` rend « Microsoft Windows 11
//! Famille », `ProductName` en registre rend « Windows 10 Home » — valeur figée
//! par l'éditeur, fausse sur les deux termes —, et `sysinfo`, que
//! `inventory.os.name` emploie déjà, rend « Windows 11 Home ».
//!
//! Un moteur qui déciderait de ses mécanismes en comparant des chaînes de ce
//! genre se tromperait le jour où l'une d'elles change. **Un mécanisme se
//! déclare disponible parce qu'on a vérifié qu'il répond**, jamais parce qu'un
//! nom ressemble. Le test `aucun_mecanisme_ne_se_decide_sur_le_nom_de_ledition`
//! en fait une barrière textuelle.
//!
//! ## « Illisible » n'est pas « indisponible »
//!
//! Les deux sont différents, et les confondre annoncerait un filet absent là où
//! l'on n'a pas su regarder. La CLI s'exécute sans élévation (SEC-01) : tout ce
//! qui exige un jeton privilégié pour être mesuré se dit **illisible**, avec sa
//! raison, et n'alimente aucun décompte — c'est la doctrine que
//! [`crate::posture`] applique déjà à ses trois états de lecture.
//!
//! La réciproque compte autant : le jeton `filet-indisponible` n'est publié que
//! lorsqu'une **mesure** a effectivement conclu à l'absence du mécanisme.
//!
//! ## Ce que ce module ne dit pas
//!
//! * **la couverture d'un instantané.** Elle se relève sur l'artefact, après
//!   coup, dans le contexte qui l'a produit (ADR-0021, décision n° 1) : le
//!   broker s'exécute en SYSTEM là où `ks` s'exécute sans élévation, donc un
//!   même export ne couvre pas les mêmes clés selon qui le lance ;
//! * **le degré de preuve.** Existence, relecture, restauration éprouvée : ces
//!   trois degrés portent sur la classe de mécanisme et s'établissent dans le
//!   labo, pas au scan ;
//! * **le coût d'un retour arrière.** Deux mesures manquent toujours, la durée
//!   d'un `wsl --export` réel et le coût d'un point de restauration ; elles ne
//!   s'estiment pas.
//!
//! ## Les deux natures d'instantané que ce module ne recense pas
//!
//! **Le point de contrôle Hyper-V n'est pas un mécanisme.** L'ADR-0021 le retire
//! du modèle : ni le poste de référence, en édition Famille, ni le labo ne
//! savent le produire, et NF-07 répute inexistant un chemin de retour arrière
//! non testé. L'alternative « le garder en le documentant comme non disponible
//! ici » y est explicitement écartée — une variante qui apparaît dans le JSON,
//! dans les libellés et dans la documentation laisse croire à une couverture,
//! et personne ne relit la note. Publier un item « Hyper-V : indisponible »
//! serait exactement cette alternative sous un autre nom.
//!
//! `ks_core::SnapshotKind` porte **encore** une variante `HyperVCheckpoint`, que
//! l'ADR-0021 décide de retirer et qu'un autre lot retirera : ce module ne
//! s'appuie donc pas sur cette énumération. [`Mecanisme`] est déclaré ici, et
//! c'est aussi le bon découpage — `SnapshotKind` décrit un instantané *pris*,
//! avec sa cible en paramètre, quand la disponibilité porte sur la **classe** de
//! mécanisme, sans cible.
//!
//! **La quarantaine n'est pas un filet.** `SnapshotKind::Quarantine` désigne
//! l'endroit où part ce qui aurait été supprimé (D4-04) : c'est une destination,
//! pas un chemin de retour arrière pour une convergence. L'exclure est une
//! décision, elle est écrite ici pour ne pas passer pour un oubli.

use ks_core::{Domain, Item, ItemValue, Nature, Provenance};

use crate::jetons::{DisponibiliteFilet, TableDeCodes as _};
use crate::posture::Lecture;
use crate::virtualisation::Distribution;

/// Un mécanisme d'instantané, au sens de l'ADR-0021 § 3 : une **classe**, sans
/// cible.
///
/// Les deux rangs ne se confondent pas. Le rang 1 est l'annulation que Keystone
/// fabrique lui-même, ciblée sur ce que l'action touche ; le rang 2 est le filet
/// de plateforme, dont la portée dépasse de très loin ce que le plan touche, et
/// qui n'est jamais le retour arrière nominal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mecanisme {
    /// Rang 1 — export de la branche de registre que l'item occupe.
    ExportDeBrancheDeRegistre,
    /// Rang 1 — copie du fichier exact que l'action modifie.
    CopieDeFichier,
    /// Rang 2 — point de restauration système Windows.
    PointDeRestaurationSysteme,
    /// Rang 2 — export d'une distribution WSL (`wsl --export`).
    ExportDeDistributionWsl,
}

impl Mecanisme {
    /// Le recensement complet.
    ///
    /// La liste est écrite à la main, mais **pas laissée à la vigilance** : le
    /// test `le_recensement_ne_saute_aucun_mecanisme` confronte ce tableau aux
    /// variantes réellement déclarées dans ce fichier, lues par `include_str!`.
    /// Une variante ajoutée sans rejoindre le tableau fait échouer la suite.
    pub const TOUS: [Self; 4] = [
        Self::ExportDeBrancheDeRegistre,
        Self::CopieDeFichier,
        Self::PointDeRestaurationSysteme,
        Self::ExportDeDistributionWsl,
    ];

    /// La clef du mécanisme dans le chemin de l'item.
    ///
    /// **C'est un format**, au même titre qu'un jeton (ADR-0015) : il part dans
    /// le relevé, puis dans le magasin d'observations, où une série se compare
    /// d'un scan à l'autre. Il ne se renomme jamais.
    #[must_use]
    pub const fn clef(self) -> &'static str {
        // `match` exhaustif sans bras `_` : ajouter un mécanisme casse ici la
        // compilation, donc la CI, avant qu'un test s'exécute.
        match self {
            Self::ExportDeBrancheDeRegistre => "registry_export",
            Self::CopieDeFichier => "file_copy",
            Self::PointDeRestaurationSysteme => "system_restore_point",
            Self::ExportDeDistributionWsl => "wsl_export",
        }
    }

    /// Le rang du filet : 1 pour l'annulation ciblée, 2 pour le filet de
    /// plateforme.
    #[must_use]
    pub const fn rang(self) -> u8 {
        match self {
            Self::ExportDeBrancheDeRegistre | Self::CopieDeFichier => 1,
            Self::PointDeRestaurationSysteme | Self::ExportDeDistributionWsl => 2,
        }
    }

    /// Le chemin de l'item publié.
    #[must_use]
    pub fn chemin(self) -> String {
        format!("updates.snapshot[{}].available", self.clef())
    }

    /// La finalité de l'item — ce que la valeur veut dire, et ce qui l'a mesurée.
    ///
    /// Elle nomme la mesure, et pas seulement l'intention : le principe P6
    /// interdit d'afficher un item que l'utilisateur ne peut pas comprendre, et
    /// une disponibilité sans son critère n'est pas compréhensible.
    const fn finalite(self) -> &'static str {
        match self {
            Self::ExportDeBrancheDeRegistre => {
                "Disponibilité de l'export d'une branche de registre, le filet ciblé \
                 que Keystone fabrique lui-même avant de changer un item. La mesure \
                 ouvre une branche témoin, lui demande sa première sous-clé, puis \
                 descend dedans : c'est la séquence exacte qu'un export parcourt, et \
                 celle qui sépare une capture réelle d'un fichier bien formé et vide."
            }
            Self::CopieDeFichier => {
                "Disponibilité de la copie du fichier qu'une action modifierait, \
                 second filet ciblé du rang 1. Sa mesure supposerait d'écrire dans le \
                 magasin d'artefacts détenu par le service privilégié, qui n'existe \
                 pas avant la phase 2 : la valeur dit donc « illisible », et ce que \
                 personne n'a mesuré ne s'affiche pas comme mesuré."
            }
            Self::PointDeRestaurationSysteme => {
                "Disponibilité du point de restauration système, filet de plateforme \
                 de dernier recours. La mesure lit « RPSessionInterval » sous la clé \
                 « SystemRestore » : à zéro, la protection système est désactivée, et \
                 aucun point ne peut être pris tant qu'elle ne l'est pas — activer la \
                 protection est une écriture système que Keystone ne fait jamais."
            }
            Self::ExportDeDistributionWsl => {
                "Disponibilité de l'export d'une distribution WSL, second filet de \
                 plateforme. La mesure lit les distributions déclarées au registre et \
                 vérifie qu'au moins un disque virtuel se laisse mesurer sur le \
                 disque : sans distribution, il n'y a rien à exporter."
            }
        }
    }

    /// Le risque que porte cet item — ici, celui de son absence.
    const fn risque(self) -> &'static str {
        match self {
            Self::ExportDeBrancheDeRegistre | Self::CopieDeFichier => {
                "Sans ce filet ciblé, une action qui écrit n'a pas d'annulation à sa \
                 portée, et le retour arrière retombe sur un filet de plateforme dont \
                 la portée dépasse de loin ce que l'action touchait."
            }
            Self::PointDeRestaurationSysteme | Self::ExportDeDistributionWsl => {
                "Un filet de plateforme absent restreint le retour arrière au seul \
                 rang 1. Un filet de plateforme présent n'est pas pour autant le \
                 chemin nominal : revenir par lui emporte aussi les écritures que le \
                 système et l'utilisateur ont faites pendant l'opération."
            }
        }
    }
}

/// Ce qu'une mesure de disponibilité a donné. **Trois issues, jamais deux.**
///
/// La deuxième est celle qui manquerait le plus : un mécanisme dont la mesure
/// est refusée n'est pas un mécanisme absent, et le publier comme indisponible
/// annoncerait un filet manquant là où l'on n'a pas su regarder.
///
/// Les trois issues sont exactement les trois formes que `ks-core` sait donner à
/// une valeur d'item : une valeur, un aveu, une absence. Le vocabulaire de la
/// première vit dans [`crate::jetons`], et nulle part ailleurs (ADR-0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disponibilite {
    /// La mesure a conclu — voir [`DisponibiliteFilet`].
    Mesuree(DisponibiliteFilet),
    /// La disponibilité n'a pas été mesurée, et la raison part avec la valeur.
    ///
    /// Deux causes s'y rangent, distinguées par leur raison et non par le type :
    /// la mesure est refusée sans élévation (SEC-01), ou elle exigerait une
    /// écriture qu'un collecteur ne fait jamais. Les deux disent « on n'a pas
    /// regardé », ce qui est ce que l'utilisateur doit lire.
    NonMesuree(&'static str),
    /// Le mécanisme n'a pas d'objet sur cette plateforme.
    SansObjet,
}

impl Disponibilite {
    /// Raccourci pour la conclusion positive.
    #[must_use]
    pub const fn disponible() -> Self {
        Self::Mesuree(DisponibiliteFilet::Disponible)
    }

    /// Raccourci pour la conclusion négative — **une mesure, pas un défaut de
    /// mesure**.
    #[must_use]
    pub const fn indisponible() -> Self {
        Self::Mesuree(DisponibiliteFilet::Indisponible)
    }

    /// La valeur telle qu'elle part dans l'item.
    ///
    /// [`Self::NonMesuree`] devient un [`ItemValue::Illisible`], la forme que
    /// `ks-core` réserve à ce qui n'est « ni une valeur, ni une absence » : elle
    /// porte sa raison et ne compte jamais dans un décompte.
    #[must_use]
    pub fn valeur(self) -> ItemValue {
        match self {
            Self::Mesuree(conclusion) => ItemValue::Text(conclusion.jeton()),
            Self::NonMesuree(raison) => ItemValue::illisible(raison),
            Self::SansObjet => ItemValue::Absent,
        }
    }
}

/// Ce qu'un sondage de branche de registre a donné.
///
/// Les cinq états viennent d'une mesure consignée par l'ADR-0021, et ils sont
/// la raison d'être de ce type : `reg export HKLM\SAM` renvoie **code 0**,
/// affiche « L'opération a réussi » et produit **138 octets** qui ne contiennent
/// rien, parce que la racine s'ouvre, annonce une sous-clé, et refuse la
/// descente vers elle. La branche voisine, `HKLM\SECURITY`, échoue franchement
/// dès l'ouverture.
///
/// Un sondage qui s'arrêterait à l'ouverture confondrait donc les deux premiers
/// états, c'est-à-dire un filet réel et un filet qui se déclare pris.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SondageDeBranche {
    /// La branche s'ouvre, annonce une sous-clé, et la descente vers elle réussit.
    DescenteReussie,
    /// La branche s'ouvre et annonce une sous-clé, mais la descente est refusée.
    DescenteRefusee,
    /// L'ouverture de la branche elle-même est refusée.
    OuvertureRefusee,
    /// La branche s'ouvre et n'annonce aucune sous-clé.
    SansSousClef,
    /// Aucune branche témoin ici.
    ///
    /// C'est le cas hors Windows, où il n'y a pas de registre du tout, et c'est
    /// aussi celui d'une machine dont la branche témoin n'existerait pas : dans
    /// les deux cas il n'y a rien à exporter, et rien n'a été refusé.
    #[default]
    SansRegistre,
}

/// Les mesures brutes dont les items se déduisent.
///
/// Séparer le relevé de sa traduction est ce qui rend les quatre mécanismes
/// éprouvables **sans machine Windows** : [`items_depuis`] est pure, et chaque
/// état de chaque mesure s'y injecte à la main.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Releve {
    /// `RPSessionInterval` sous `SystemRestore`, dans la ruche de la machine.
    pub intervalle_de_session: Lecture<u32>,
    /// Les distributions WSL déclarées au registre.
    pub distributions: Lecture<Vec<Distribution>>,
    /// Ce que le sondage de la branche témoin a donné.
    pub branche_temoin: SondageDeBranche,
}

/// Collecteur de disponibilité du filet (D3-07, ADR-0021 décision n° 5).
pub struct InstantanesCollector;

impl InstantanesCollector {
    /// Identifiant stable, pour le journal et les budgets d'empreinte.
    #[must_use]
    pub const fn nom() -> &'static str {
        "instantanes"
    }

    /// Les trois mesures, telles que cette machine les rend.
    #[must_use]
    pub fn releve() -> Releve {
        #[cfg(windows)]
        {
            Releve {
                intervalle_de_session: windows_impl::intervalle_de_session(),
                distributions: crate::virtualisation::VirtualisationCollector::distributions(),
                branche_temoin: windows_impl::sonder_la_branche_temoin(),
            }
        }
        #[cfg(not(windows))]
        {
            // Aucun de ces mécanismes n'a d'objet ici, et le défaut de `Releve`
            // le dit sans rien inventer : deux lectures absentes et un sondage
            // « sans registre ». Surtout pas des zéros, qui prétendraient qu'on
            // a compté.
            Releve::default()
        }
    }

    /// Un item par mécanisme.
    #[must_use]
    pub fn items() -> Vec<Item> {
        items_depuis(&Self::releve())
    }
}

/// Ce qu'on dit quand la mesure exigerait une écriture.
const SANS_MAGASIN: &str = "la copie de fichier dépose son artefact dans un magasin détenu par le \
                            service privilégié, absent avant la phase 2 : mesurer sa disponibilité \
                            exigerait d'écrire, ce qu'un collecteur ne fait jamais";

/// Ce qu'on dit quand la protection système est active mais invérifiable.
const RESTAURATION_NON_VERIFIABLE: &str =
    "la protection système n'est pas désactivée, mais l'inventaire des points de \
     retour est refusé sans élévation : rien ne permet de dire qu'un point serait \
     réellement pris";

/// Ce qu'on dit quand la clef de la restauration système est refusée.
const RESTAURATION_REFUSEE: &str = "accès refusé à la clé de la protection système";

/// Ce qu'on dit quand la branche témoin s'ouvre sans se laisser descendre.
const BRANCHE_CREUSE: &str = "la branche témoin s'ouvre et annonce une sous-clé, mais la descente \
                              vers elle est refusée : un export y réussirait sans rien capturer";

/// Ce qu'on dit quand la branche témoin est refusée dès l'ouverture.
const BRANCHE_REFUSEE: &str = "accès refusé à la branche témoin du registre";

/// Ce qu'on dit quand la branche témoin ne porte rien.
const BRANCHE_VIDE: &str = "la branche témoin n'annonce aucune sous-clé : rien ne permet de dire \
                            qu'un export capturerait quelque chose";

/// Ce qu'on dit quand le registre des distributions est refusé.
const WSL_REFUSE: &str = "accès refusé au registre des distributions WSL";

/// Ce qu'on dit quand des distributions existent sans disque mesurable.
const WSL_SANS_DISQUE: &str = "des distributions sont déclarées, mais aucun de leurs disques \
                               virtuels ne s'est laissé mesurer : rien ne permet de dire qu'un \
                               export produirait quelque chose";

/// La disponibilité d'un mécanisme, d'après le relevé.
///
/// # La barrière
///
/// Le `match` est **exhaustif et sans bras `_`**, et sans bras de liaison non
/// plus : ajouter un mécanisme casse ici la compilation, donc la CI, avant qu'un
/// test s'exécute. Le contributeur est amené à l'endroit exact où se décident
/// les conditions d'admission d'un filet, et il doit dire par quelle mesure le
/// sien se déclare disponible. C'est le même dispositif que la barrière SEC-02,
/// et il a la même limite assumée : il force la décision, il ne la juge pas.
#[must_use]
pub fn disponibilite(mecanisme: Mecanisme, releve: &Releve) -> Disponibilite {
    match mecanisme {
        Mecanisme::ExportDeBrancheDeRegistre => depuis_le_sondage(releve.branche_temoin),
        // Aucune mesure en lecture seule ne décide de celui-ci, et c'est une
        // décision, pas un trou : voir `SANS_MAGASIN`.
        Mecanisme::CopieDeFichier => Disponibilite::NonMesuree(SANS_MAGASIN),
        Mecanisme::PointDeRestaurationSysteme => depuis_lintervalle(&releve.intervalle_de_session),
        Mecanisme::ExportDeDistributionWsl => depuis_les_distributions(&releve.distributions),
    }
}

/// La disponibilité de l'export de branche, d'après le sondage.
fn depuis_le_sondage(sondage: SondageDeBranche) -> Disponibilite {
    match sondage {
        SondageDeBranche::DescenteReussie => Disponibilite::disponible(),
        SondageDeBranche::DescenteRefusee => Disponibilite::NonMesuree(BRANCHE_CREUSE),
        SondageDeBranche::OuvertureRefusee => Disponibilite::NonMesuree(BRANCHE_REFUSEE),
        SondageDeBranche::SansSousClef => Disponibilite::NonMesuree(BRANCHE_VIDE),
        SondageDeBranche::SansRegistre => Disponibilite::SansObjet,
    }
}

/// La disponibilité du point de restauration, d'après `RPSessionInterval`.
///
/// Une seule des quatre lectures conclut, et c'est la négative : à zéro, Windows
/// annonce que la protection système ne prend plus de point. Une valeur non
/// nulle ne prouve **pas** l'inverse — la création exige une élévation, et
/// `SRSetRestorePoint` renvoie `TRUE` en ayant sauté la création lorsqu'un point
/// existe depuis moins de 24 heures. Cette moitié-là se dit donc « illisible »,
/// jamais « disponible ».
fn depuis_lintervalle(lecture: &Lecture<u32>) -> Disponibilite {
    match lecture {
        Lecture::Trouvee(0) => Disponibilite::indisponible(),
        Lecture::Trouvee(_) => Disponibilite::NonMesuree(RESTAURATION_NON_VERIFIABLE),
        // La clé ou la valeur n'existe pas : le sous-système de restauration
        // n'est pas là. Hors Windows c'est le cas normal.
        Lecture::Absente => Disponibilite::SansObjet,
        Lecture::Refusee => Disponibilite::NonMesuree(RESTAURATION_REFUSEE),
    }
}

/// La disponibilité de l'export WSL, d'après les distributions déclarées.
///
/// Le critère est **un disque virtuel qui s'est laissé mesurer**, et non la
/// seule présence d'une entrée au registre : c'est la différence entre vérifier
/// qu'un mécanisme répond et se fier à un nom. La limite est assumée — une
/// distribution restée en version 1 n'a pas de disque virtuel, donc un poste qui
/// n'en porterait que de celles-là se lirait « illisible » plutôt que
/// « disponible ». Le sens de l'erreur est délibéré : on ne déclare jamais un
/// filet qu'on n'a pas vu.
fn depuis_les_distributions(lecture: &Lecture<Vec<Distribution>>) -> Disponibilite {
    match lecture {
        Lecture::Trouvee(distributions) if distributions.is_empty() => {
            Disponibilite::indisponible()
        }
        Lecture::Trouvee(distributions) => {
            if distributions.iter().any(|d| d.disque_octets.is_some()) {
                Disponibilite::disponible()
            } else {
                Disponibilite::NonMesuree(WSL_SANS_DISQUE)
            }
        }
        Lecture::Absente => Disponibilite::SansObjet,
        Lecture::Refusee => Disponibilite::NonMesuree(WSL_REFUSE),
    }
}

/// Traduit un relevé en items. **Pure** : c'est ce qui la rend éprouvable.
#[must_use]
pub fn items_depuis(releve: &Releve) -> Vec<Item> {
    let maintenant = chrono::Utc::now();
    Mecanisme::TOUS
        .into_iter()
        .map(|mecanisme| Item {
            path: mecanisme.chemin(),
            // D3 — l'instantané est ce qu'une mise à jour orchestrée exige avant
            // d'écrire (D3-07), pas une sauvegarde de données de travail. Le
            // glossaire sépare les deux, et `Domain::Backup` est à D6.
            domain: Domain::Updates,
            // La disponibilité d'un mécanisme se **subit** : elle décrit ce que
            // cette machine sait faire. Elle ne se déclare pas dans
            // `workstation.yaml`, et aucun verbe ne l'écrira — activer la
            // protection système reste une décision de l'utilisateur.
            nature: Nature::Constat,
            desired: None,
            observed: disponibilite(mecanisme, releve).valeur(),
            observed_at: maintenant,
            // `Observed` : ce module lit, il n'est l'auteur d'aucune de ces
            // valeurs. `Keystone` rendrait `Unknown` inatteignable, donc le
            // signal de sécurité du modèle inopérant.
            provenance: Provenance::Observed,
            purpose: mecanisme.finalite().to_owned(),
            risk: mecanisme.risque().to_owned(),
            reference: None,
        })
        .collect()
}

#[cfg(windows)]
mod windows_impl {
    use super::SondageDeBranche;
    use crate::posture::{classer, Lecture, ACCES_REFUSE};
    use windows_registry::LOCAL_MACHINE;

    /// Où Windows écrit la configuration de la protection système.
    const SYSTEM_RESTORE: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore";

    /// La valeur que Windows met à zéro quand la protection système ne prend
    /// plus de point.
    const RP_SESSION_INTERVAL: &str = "RPSessionInterval";

    /// La branche témoin du sondage d'export.
    ///
    /// C'est une branche que le rang 1 viserait réellement : le type de
    /// démarrage d'un service y vit, et c'est l'un des rares réglages que la
    /// convergence écrira. L'ADR-0021 l'a mesurée — `reg export` y rend 15,57 Mo
    /// en 1 038 ms —, ce qui évite de choisir une branche au hasard.
    ///
    /// **Le sondage n'exporte rien** et ne lit aucune valeur : il ouvre, demande
    /// le premier nom de sous-clé, et ouvre celle-là. Trois appels, quel que
    /// soit le nombre de services.
    const BRANCHE_TEMOIN: &str = r"SYSTEM\CurrentControlSet\Services";

    /// Lit `RPSessionInterval`, en séparant le refus de l'absence.
    pub(super) fn intervalle_de_session() -> Lecture<u32> {
        match LOCAL_MACHINE.open(SYSTEM_RESTORE) {
            Ok(clef) => classer(clef.get_u32(RP_SESSION_INTERVAL)),
            Err(e) if e.code().0 == ACCES_REFUSE => Lecture::Refusee,
            Err(_) => Lecture::Absente,
        }
    }

    /// Rejoue, en lecture, les trois pas qu'un export de branche parcourt.
    ///
    /// L'ordre importe : c'est la **descente** qui discrimine, et non
    /// l'ouverture. Sans elle, `HKLM\SAM` passerait pour une branche exportable.
    pub(super) fn sonder_la_branche_temoin() -> SondageDeBranche {
        let racine = match LOCAL_MACHINE.open(BRANCHE_TEMOIN) {
            Ok(k) => k,
            Err(e) if e.code().0 == ACCES_REFUSE => return SondageDeBranche::OuvertureRefusee,
            Err(_) => return SondageDeBranche::SansRegistre,
        };
        let mut sous_clefs = match racine.keys() {
            Ok(s) => s,
            Err(e) if e.code().0 == ACCES_REFUSE => return SondageDeBranche::OuvertureRefusee,
            Err(_) => return SondageDeBranche::SansSousClef,
        };
        let Some(premiere) = sous_clefs.next() else {
            return SondageDeBranche::SansSousClef;
        };
        match racine.open(&premiere) {
            Ok(_) => SondageDeBranche::DescenteReussie,
            Err(e) if e.code().0 == ACCES_REFUSE => SondageDeBranche::DescenteRefusee,
            // Une sous-clé disparue entre l'énumération et l'ouverture est une
            // course bénigne, pas un refus : on ne conclut rien.
            Err(_) => SondageDeBranche::SansSousClef,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un relevé où tout répond — la forme la plus favorable qui soit.
    fn releve_favorable() -> Releve {
        Releve {
            intervalle_de_session: Lecture::Trouvee(86_400),
            distributions: Lecture::Trouvee(vec![distribution(Some(56_043_241_472))]),
            branche_temoin: SondageDeBranche::DescenteReussie,
        }
    }

    /// Une distribution WSL, dont seul le disque compte ici.
    fn distribution(octets: Option<u64>) -> Distribution {
        Distribution {
            nom: "Debian".to_owned(),
            version: Some(2),
            disque_octets: octets,
            volume: Some("C:".to_owned()),
            interop: Some(true),
            montage_lecteurs: Some(true),
        }
    }

    /// La valeur qu'une conclusion de mesure produit, écrite une seule fois.
    ///
    /// Les tests comparent des `ItemValue`, pas des jetons : c'est ce que l'item
    /// porte réellement, donc ce qui part dans le magasin d'observations.
    fn valeur_de(conclusion: DisponibiliteFilet) -> ItemValue {
        Disponibilite::Mesuree(conclusion).valeur()
    }

    /// La valeur publiée pour un mécanisme, à partir d'un relevé.
    fn valeur(mecanisme: Mecanisme, releve: &Releve) -> ItemValue {
        items_depuis(releve)
            .into_iter()
            .find(|i| i.path == mecanisme.chemin())
            .unwrap_or_else(|| panic!("« {} » n'est pas publié", mecanisme.clef()))
            .observed
    }

    #[test]
    fn chaque_mecanisme_publie_sa_disponibilite_et_son_explication() {
        let items = items_depuis(&releve_favorable());
        assert_eq!(items.len(), Mecanisme::TOUS.len(), "un item par mécanisme");

        for item in &items {
            assert_eq!(item.domain, Domain::Updates);
            assert_eq!(item.provenance, Provenance::Observed);
            assert!(item.desired.is_none(), "un collecteur ne décide de rien");
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert!(!item.risk.is_empty(), "« {} » sans risque", item.path);
        }

        // Deux chemins identiques masqueraient un mécanisme derrière l'autre.
        let mut chemins: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        chemins.sort_unstable();
        let avant = chemins.len();
        chemins.dedup();
        assert_eq!(avant, chemins.len(), "deux mécanismes partagent un chemin");
    }

    /// Le recensement des mécanismes nomme-t-il toutes les variantes ?
    ///
    /// `Mecanisme::TOUS` est un tableau écrit à la main : le `match` exhaustif
    /// de [`Mecanisme::clef`] casse la compilation quand une variante s'ajoute,
    /// mais rien n'obligerait le contributeur à la verser aussi dans le
    /// recensement — et un mécanisme hors recensement ne publierait aucun item,
    /// donc n'apparaîtrait nulle part, en silence.
    ///
    /// La barrière est textuelle et autonome, sur le modèle de
    /// `la_liste_des_collecteurs_est_a_jour` : `include_str!` confronte le
    /// tableau aux variantes déclarées dans l'énumération, sans macro ni
    /// dépendance. Éprouvée par falsification — une variante ajoutée sans
    /// rejoindre `TOUS` fait échouer ce test en la nommant.
    #[test]
    fn le_recensement_ne_saute_aucun_mecanisme() {
        const SOURCE: &str = include_str!("instantanes.rs");

        // Les variantes de l'énumération : les lignes du bloc `pub enum
        // Mecanisme { … }` qui sont un identifiant suivi d'une virgule.
        let corps = SOURCE
            .split_once("pub enum Mecanisme {")
            .expect("l'énumération des mécanismes doit exister")
            .1
            .split_once("\n}")
            .expect("l'énumération doit se refermer")
            .0;
        let declarees: std::collections::BTreeSet<&str> = corps
            .lines()
            .map(str::trim)
            .filter_map(|l| l.strip_suffix(','))
            .filter(|nom| {
                !nom.is_empty()
                    && nom.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && nom.starts_with(char::is_uppercase)
            })
            .collect();

        let recensees: std::collections::BTreeSet<&str> = Mecanisme::TOUS
            .iter()
            .map(|m| {
                // `match` exhaustif sans bras `_` : le nom Rust de la variante,
                // écrit une fois de plus, à l'endroit où le recensement se
                // vérifie.
                match m {
                    Mecanisme::ExportDeBrancheDeRegistre => "ExportDeBrancheDeRegistre",
                    Mecanisme::CopieDeFichier => "CopieDeFichier",
                    Mecanisme::PointDeRestaurationSysteme => "PointDeRestaurationSysteme",
                    Mecanisme::ExportDeDistributionWsl => "ExportDeDistributionWsl",
                }
            })
            .collect();

        assert!(
            !declarees.is_empty(),
            "aucune variante lue dans la source — la barrière ne barre plus rien"
        );
        assert_eq!(
            declarees, recensees,
            "le recensement et l'énumération divergent :
  déclarées {declarees:?}
  recensées {recensees:?}"
        );
    }

    /// **LA BARRIÈRE DE LA MESURE DE CAPACITÉ** (ADR-0021, « le piège de mesure »).
    ///
    /// Trois lectures du même fait donnent trois chaînes différentes, et l'une
    /// d'elles — `ProductName` — est fausse sur les deux termes. Un mécanisme
    /// qui se déciderait sur un nom d'édition se tromperait le jour où l'une de
    /// ces chaînes change, et rien ne le signalerait.
    ///
    /// Le contrôle est textuel et porte sur ce fichier, donc sur les décisions
    /// d'admission elles-mêmes. Les motifs sont assemblés par `concat!` pour que
    /// le test ne contienne pas lui-même ce qu'il interdit.
    ///
    /// **Sa limite est assumée**, comme celle de la barrière SEC-02 : une
    /// comparaison écrite autrement — une constante nommée ailleurs, une lecture
    /// passée en paramètre — passerait. Un test grossier ne remplace ni la revue
    /// ni l'ADR ; il rend le geste évident impossible à commettre par
    /// distraction. Éprouvée par falsification.
    #[test]
    fn aucun_mecanisme_ne_se_decide_sur_le_nom_de_ledition() {
        const SOURCE: &str = include_str!("instantanes.rs");

        let interdits = [
            concat!("Product", "Name"),
            concat!("Edition", "ID"),
            concat!("long_os", "_version"),
            concat!("Win32_Operating", "System"),
            concat!("\"Enter", "prise\""),
            concat!("\"Ho", "me\""),
            concat!("\"Fam", "ille\""),
            concat!("\"P", "ro\""),
        ];

        // La documentation de tête cite ces valeurs pour dire de ne pas s'en
        // servir : on ne contrôle donc que le code, jamais les commentaires.
        let code: String = SOURCE
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        // Un contrôle qui ne contrôle rien passerait tout aussi vert : si la
        // mesure de capacité disparaissait de ce fichier, aucun motif interdit
        // n'y serait plus, et la barrière deviendrait une étape verte de plus.
        assert!(
            code.contains("RP_SESSION_INTERVAL") && code.contains("BRANCHE_TEMOIN"),
            "les deux sondes de capacité ont disparu — la barrière ne garde plus rien"
        );
        for motif in interdits {
            assert!(
                !code.contains(motif),
                "un mécanisme se décide sur « {motif} » : la disponibilité se \
                 mesure en interrogeant la capacité, jamais en lisant un nom \
                 d'édition (ADR-0021)"
            );
        }
    }

    /// **LA BARRIÈRE « ILLISIBLE N'EST PAS INDISPONIBLE »**.
    ///
    /// Un refus de lecture publié en `false` annoncerait un filet absent là où
    /// l'on n'a pas su regarder — et l'utilisateur renoncerait à appliquer, ou
    /// pire, croirait avoir compris. Chaque mesure refusée est donc passée en
    /// revue, et l'on exige d'elle une valeur illisible portant sa raison.
    ///
    /// Éprouvée par falsification : rendre `Indisponible` sur un refus fait
    /// échouer ce test en nommant le mécanisme.
    #[test]
    fn une_mesure_refusee_ne_se_publie_jamais_en_filet_absent() {
        let refus = [
            (
                Mecanisme::PointDeRestaurationSysteme,
                Releve {
                    intervalle_de_session: Lecture::Refusee,
                    ..releve_favorable()
                },
            ),
            (
                Mecanisme::ExportDeDistributionWsl,
                Releve {
                    distributions: Lecture::Refusee,
                    ..releve_favorable()
                },
            ),
            (
                Mecanisme::ExportDeBrancheDeRegistre,
                Releve {
                    branche_temoin: SondageDeBranche::OuvertureRefusee,
                    ..releve_favorable()
                },
            ),
            (
                Mecanisme::ExportDeBrancheDeRegistre,
                Releve {
                    branche_temoin: SondageDeBranche::DescenteRefusee,
                    ..releve_favorable()
                },
            ),
        ];

        for (mecanisme, releve) in &refus {
            let observee = valeur(*mecanisme, releve);
            assert_ne!(
                observee,
                valeur_de(DisponibiliteFilet::Indisponible),
                "« {} » : un refus de lecture se dit illisible, jamais « pas de filet »",
                mecanisme.clef()
            );
            assert_ne!(
                observee,
                ItemValue::Absent,
                "« {} » : un refus n'est pas une absence",
                mecanisme.clef()
            );
            assert!(
                !observee.est_constat(),
                "« {} » : un aveu ne doit alimenter aucun décompte",
                mecanisme.clef()
            );
            assert!(
                observee.to_string().contains("refus"),
                "« {} » : la raison doit remonter à l'utilisateur — {observee}",
                mecanisme.clef()
            );
        }
    }

    #[test]
    fn une_protection_systeme_desactivee_se_dit_indisponible_et_se_mesure() {
        // La valeur réellement relevée sur le poste de référence : zéro. C'est
        // le seul des quatre états qui conclut à l'absence de filet, et il
        // conclut parce qu'une mesure l'a dit, pas parce qu'un nom ressemblait.
        let releve = Releve {
            intervalle_de_session: Lecture::Trouvee(0),
            ..releve_favorable()
        };
        let observee = valeur(Mecanisme::PointDeRestaurationSysteme, &releve);
        assert_eq!(observee, valeur_de(DisponibiliteFilet::Indisponible));
        assert!(
            observee.est_constat(),
            "une mesure qui conclut est un constat, et elle compte"
        );

        // Et l'inverse ne se déduit pas : protection active ne veut pas dire
        // point pris. La création exige une élévation, et l'interface système
        // renvoie « vrai » en ayant sauté la création sous 24 heures.
        let actif = Releve {
            intervalle_de_session: Lecture::Trouvee(86_400),
            ..releve_favorable()
        };
        let observee = valeur(Mecanisme::PointDeRestaurationSysteme, &actif);
        assert_ne!(
            observee,
            valeur_de(DisponibiliteFilet::Disponible),
            "une protection active ne prouve pas qu'un point serait pris"
        );
        assert!(!observee.est_constat());
    }

    #[test]
    fn une_branche_qui_souvre_sans_se_laisser_descendre_ne_rend_pas_lexport_disponible() {
        // Le cas `HKLM\SAM` : code 0, « L'opération a réussi », 138 octets
        // vides. C'est le mode de défaillance que l'ADR-0021 place au premier
        // rang — un filet qui se déclare pris.
        for sondage in [
            SondageDeBranche::DescenteRefusee,
            SondageDeBranche::OuvertureRefusee,
            SondageDeBranche::SansSousClef,
        ] {
            let releve = Releve {
                branche_temoin: sondage,
                ..releve_favorable()
            };
            assert_ne!(
                valeur(Mecanisme::ExportDeBrancheDeRegistre, &releve),
                valeur_de(DisponibiliteFilet::Disponible),
                "« {sondage:?} » suffirait à déclarer un filet qu'on n'a pas vu"
            );
        }

        // Et la descente réussie, elle, conclut.
        assert_eq!(
            valeur(Mecanisme::ExportDeBrancheDeRegistre, &releve_favorable()),
            valeur_de(DisponibiliteFilet::Disponible)
        );
    }

    #[test]
    fn un_export_wsl_se_declare_sur_un_disque_mesure_et_pas_sur_une_entree_de_registre() {
        // Trois distributions déclarées dont aucun disque ne se laisse mesurer :
        // le registre dit qu'elles existent, et rien ne dit qu'un export
        // produirait quoi que ce soit.
        let sans_disque = Releve {
            distributions: Lecture::Trouvee(vec![distribution(None), distribution(None)]),
            ..releve_favorable()
        };
        let observee = valeur(Mecanisme::ExportDeDistributionWsl, &sans_disque);
        assert_ne!(observee, valeur_de(DisponibiliteFilet::Disponible));
        assert!(!observee.est_constat());

        // Aucune distribution : là, on a compté, et le filet n'a pas d'objet.
        // C'est une mesure, donc un constat, et surtout pas un aveu.
        let aucune = Releve {
            distributions: Lecture::Trouvee(Vec::new()),
            ..releve_favorable()
        };
        let observee = valeur(Mecanisme::ExportDeDistributionWsl, &aucune);
        assert_eq!(observee, valeur_de(DisponibiliteFilet::Indisponible));
        assert!(observee.est_constat());

        // WSL jamais installé : ni un zéro, ni un aveu. Une absence.
        let jamais = Releve {
            distributions: Lecture::Absente,
            ..releve_favorable()
        };
        assert_eq!(
            valeur(Mecanisme::ExportDeDistributionWsl, &jamais),
            ItemValue::Absent
        );
    }

    #[test]
    fn la_copie_de_fichier_ne_se_declare_pas_disponible_sans_avoir_ete_mesuree() {
        // Ce que personne n'a mesuré ne s'affiche pas comme mesuré. Quel que
        // soit le reste du relevé, cet item reste un aveu tant que le magasin
        // d'artefacts n'existe pas — et le vérifier exigerait d'écrire.
        for releve in [releve_favorable(), Releve::default()] {
            let observee = valeur(Mecanisme::CopieDeFichier, &releve);
            assert!(
                !observee.est_constat(),
                "une disponibilité non mesurée ne doit alimenter aucun décompte"
            );
            assert_ne!(observee, valeur_de(DisponibiliteFilet::Disponible));
            assert_ne!(observee, valeur_de(DisponibiliteFilet::Indisponible));
            assert!(
                observee.to_string().contains("écrire"),
                "la raison doit dire pourquoi on n'a pas mesuré — {observee}"
            );
        }
    }

    #[test]
    fn hors_de_toute_plateforme_le_releve_ne_publie_que_des_absences() {
        // Le défaut du relevé est ce que l'agent Linux produira. Aucun zéro,
        // aucun « false » : un mécanisme sans objet ici n'est pas un mécanisme
        // qu'on a mesuré absent.
        let items = items_depuis(&Releve::default());
        assert_eq!(items.len(), Mecanisme::TOUS.len());

        for item in &items {
            assert_ne!(
                item.observed,
                valeur_de(DisponibiliteFilet::Indisponible),
                "« {} » prétend avoir mesuré une absence de filet",
                item.path
            );
            assert_ne!(
                item.observed,
                valeur_de(DisponibiliteFilet::Disponible),
                "« {} » annonce un filet qu'aucune mesure n'a vu",
                item.path
            );
        }
    }

    #[test]
    fn aucun_item_de_filet_na_vocation_a_etre_declare() {
        // La disponibilité d'un mécanisme se subit : elle décrit ce que la
        // machine sait faire. La déclarer verserait dans `workstation.yaml` une
        // ligne que personne ne peut satisfaire — et Keystone n'active jamais la
        // protection système de lui-même (ADR-0021, décision n° 5).
        for item in items_depuis(&releve_favorable()) {
            // `match` exhaustif sans bras `_` : ajouter une variante à `Nature`
            // casse ici la compilation, donc la CI, avant qu'un test s'exécute.
            let declarable = match item.nature {
                Nature::Constat | Nature::Mesure => false,
                Nature::Reglage | Nature::Objectif => true,
            };
            assert!(
                !declarable,
                "« {} » : {:?} — la disponibilité d'un filet se constate",
                item.path, item.nature
            );
            assert_eq!(item.nature, Nature::Constat);
            assert!(!item.nature.est_declarable());
            assert!(
                !item.nature.est_convergeable(),
                "« {} » : aucun verbe n'activera la protection système",
                item.path
            );
        }
    }

    #[test]
    fn les_deux_rangs_de_filet_sont_nommes_et_distincts() {
        // Le rang n'est pas décoratif : le rang 2 n'est jamais le retour arrière
        // nominal, et un plan qui n'en dispose que de lui doit le dire. Deux
        // mécanismes par rang, et le compte tient.
        assert_eq!(
            Mecanisme::TOUS.iter().filter(|m| m.rang() == 1).count(),
            2,
            "le rang 1 est l'annulation que Keystone fabrique lui-même"
        );
        assert_eq!(
            Mecanisme::TOUS.iter().filter(|m| m.rang() == 2).count(),
            2,
            "le rang 2 est le filet de plateforme"
        );
        for mecanisme in Mecanisme::TOUS {
            assert!(
                (1..=2).contains(&mecanisme.rang()),
                "« {} » porte un rang hors du modèle",
                mecanisme.clef()
            );
        }
    }
}

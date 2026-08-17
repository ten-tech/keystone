//! # ks-collectors — la Phase 0, et rien de plus
//!
//! ## La règle de ce crate
//!
//! **Aucun collecteur n'écrit. Jamais. Nulle part.**
//!
//! Ce n'est pas une convention de style, c'est ce qui rend la Phase 0 exécutable
//! sans risque sur une machine de production — y compris la tienne. Un collecteur
//! qui écrirait quoi que ce soit invaliderait la promesse du projet et n'aurait
//! plus rien à faire ici.
//!
//! En pratique :
//!
//! * pas d'ouverture de handle en écriture, pas de `RegSetValue`, pas de `Set-*` ;
//! * un collecteur ne peut pas non plus *déclencher* un effet de bord (redémarrer un
//!   service pour lire son état, par exemple) ;
//! * un collecteur qui ne sait pas lire une valeur renvoie une absence, pas une
//!   supposition. Un inventaire qui invente est pire qu'un inventaire incomplet.
//!
//! ## Ce qui est implémenté aujourd'hui
//!
//! Cette section se périmait en silence. Elle a été doublée **trois** fois par
//! des modules ajoutés sans la corriger, et une quatrième fois par deux
//! affirmations devenues fausses sous elle. La consigne de relecture qui
//! figurait ici n'a jamais rien retenu : une consigne n'est pas une barrière.
//!
//! Un test la tient désormais — `la_liste_des_collecteurs_est_a_jour` — en
//! comparant cette liste aux `impl Collector for` réellement présents dans ce
//! fichier. Ajouter un collecteur sans le citer ici fait échouer la suite.
//!
//! [`HardwareCollector`] — portable, via `sysinfo`. C'est le socle qui tourne
//! partout, et qui sert de référence pour l'écriture des suivants.
//!
//! [`SoftwareCollector`] — réconciliation d'inventaire logiciel (D1-01).
//! L'attribution interroge les bases de suivi de winget en lecture seule
//! ([`winget`]), et déclare elle-même son incomplétude quand une base est
//! illisible plutôt que de publier un pourcentage flatteur.
//!
//! [`PostureCollector`] — Secure Boot, VBS/HVCI, Credential Guard, protection
//! LSA, exclusions et règles ASR de Defender, pare-feu, firmware, microcode,
//! horloge : la **configuration**, lue au registre. Et l'**état effectif** de
//! VBS, de l'intégrité mémoire, de Credential Guard et de la protection en temps
//! réel, lu par WMI ([`etat_effectif`], ADR-0005) — le registre ne le porte pas.
//!
//! [`VirtualisationCollector`] — distributions WSL par le registre : version,
//! taille réelle du disque virtuel, intégration et montage des lecteurs.
//!
//! Restent à écrire : TPM, BitLocker par volume, tâches planifiées — tous trois
//! refusés sans élévation, donc reportés au broker (Phase 2) —, la protection
//! DMA effective et l'usure NVMe/SMART.
//!
//! La cohérence de l'horloge figurait ici comme restant à écrire, quinze lignes
//! sous les items `security.clock.*` qui la produisent depuis plusieurs commits.
//!
//! ## Ce qu'un relevé dit de son origine, et ce qu'il ne dit pas
//!
//! Un collecteur produit un **relevé**, pas un changement. Sa provenance répond
//! donc à une seule question : *d'où vient cette lecture ?* Deux réponses sont
//! atteignables, et deux seulement :
//!
//! * `Provenance::Observed` — le cas courant, sans prétention sur l'auteur ;
//! * `Provenance::Managed` — la valeur a été lue sous une **ruche de politique**,
//!   qui n'a pas d'autre auteur possible ([`politique`], ADR-0018).
//!
//! Les deux interdictions comptent autant que la règle. `Keystone` serait faux —
//! l'outil n'est l'auteur d'aucune valeur qu'il a lue — et `Unknown` serait pire
//! encore : c'est le signal de sécurité de D2-05, et il naît d'une **comparaison
//! entre deux relevés**, jamais d'un seul. Le test
//! `un_releve_nest_ni_lauteur_de_la_valeur_ni_un_signal` en fait une barrière.
//!
//! L'auteur d'un *changement*, lui, vit dans [`attribution`] : liste blanche,
//! jamais inférence, et `Unknown` partout ailleurs (ADR-0011).
//!
//! ## Ce qu'un collecteur écrit dans un `ItemValue::Text`
//!
//! **Un jeton, jamais une phrase** (ADR-0015). Une valeur qui sort d'une table
//! de codes appartient au vocabulaire fermé de [`jetons`] ; la phrase française
//! vit dans `ks_cli::lisible`, avec la conversion d'octets et pour la même
//! raison. Le contrôle est mécanique — voir
//! `aucun_item_declarable_ne_porte_une_phrase_francaise`.

pub mod attribution;
pub mod etat_effectif;
pub mod jetons;
pub mod politique;
pub mod posture;
pub mod software;
pub mod virtualisation;
pub mod winget;

use chrono::Utc;
use ks_core::{Domain, Item, ItemValue, Nature, Provenance};
pub use posture::PostureCollector;
pub use software::{Application, Gestionnaire, Inventaire, SoftwareCollector};
pub use virtualisation::{Distribution, VirtualisationCollector};

impl Collector for VirtualisationCollector {
    fn id(&self) -> &'static str {
        "virtualisation"
    }

    fn domain(&self) -> Domain {
        Domain::Virtualization
    }

    fn collect(&self) -> Vec<Item> {
        Self::items()
    }
}

impl Collector for PostureCollector {
    fn id(&self) -> &'static str {
        "posture"
    }

    fn domain(&self) -> Domain {
        Domain::Security
    }

    fn collect(&self) -> Vec<Item> {
        Self::items()
    }
}

impl Collector for SoftwareCollector {
    fn id(&self) -> &'static str {
        "software"
    }

    fn domain(&self) -> Domain {
        Domain::Inventory
    }

    fn collect(&self) -> Vec<Item> {
        Self::items()
    }
}

/// Ce que tout collecteur sait faire.
pub trait Collector {
    /// Identifiant stable, utilisé dans le journal et les budgets d'empreinte.
    fn id(&self) -> &'static str;

    /// Domaine couvert.
    fn domain(&self) -> Domain;

    /// Collecte. Ne peut pas échouer globalement : un collecteur qui n'arrive pas à
    /// lire une valeur omet l'item plutôt que de faire échouer tout le scan.
    fn collect(&self) -> Vec<Item>;
}

/// Ce qu'un collecteur a produit lors d'un passage.
///
/// `Inventory` jetait cette information : il ne restait qu'une liste d'items,
/// sans trace de qui les avait produits ni de combien. Un collecteur en panne
/// devenait alors indiscernable d'un collecteur qui n'a rien trouvé — et le
/// magasin d'observations en aurait conclu que les items avaient **disparu**,
/// c'est-à-dire un changement, sur une machine où rien n'avait bougé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    /// L'identifiant stable du collecteur.
    pub id: &'static str,
    /// Combien d'items il a produits à ce passage.
    pub items: usize,
}

/// Un inventaire complet, à un instant donné.
#[derive(Debug, Default)]
pub struct Inventory {
    /// Les items observés.
    pub items: Vec<Item>,
    /// Ce que chaque collecteur a produit — voir [`Passage`].
    collecteurs: Vec<Passage>,
}

impl Inventory {
    /// Exécute tous les collecteurs disponibles sur cette plateforme.
    #[must_use]
    pub fn collect_all() -> Self {
        let collectors: Vec<Box<dyn Collector>> = vec![
            Box::new(HardwareCollector),
            Box::new(SoftwareCollector),
            Box::new(PostureCollector),
            Box::new(VirtualisationCollector),
        ];

        let mut items = Vec::new();
        let mut collecteurs = Vec::new();
        for c in &collectors {
            let produits = c.collect();
            collecteurs.push(Passage {
                id: c.id(),
                items: produits.len(),
            });
            items.extend(produits);
        }
        Self { items, collecteurs }
    }

    /// Le collecteur qui a produit ce chemin, s'il est connu.
    ///
    /// Sert à répondre à « cet item a-t-il disparu, ou son collecteur
    /// a-t-il échoué ? » — deux faits que rien ne distinguait jusqu'ici.
    #[must_use]
    pub fn passages(&self) -> &[Passage] {
        &self.collecteurs
    }

    /// Nombre d'items observés.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// L'inventaire est-il vide ?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Items d'un domaine donné.
    #[must_use]
    pub fn by_domain(&self, domain: Domain) -> Vec<&Item> {
        self.items.iter().filter(|i| i.domain == domain).collect()
    }
}

/// Construit un item observé, avec sa provenance, sa vocation et son explication.
///
/// Les champs `purpose` et `risk` ne sont pas décoratifs : l'exigence D1-10 interdit
/// d'afficher un item sans provenance, et le principe P6 (explicabilité) interdit
/// d'afficher un item que l'utilisateur ne peut pas comprendre. Le constructeur les
/// rend donc obligatoires — on ne peut pas les oublier.
///
/// `nature` obéit à la même règle, et pour la même raison (ADR-0009). Il n'y a
/// **ni valeur par défaut, ni `Option`** : une valeur par défaut serait une
/// décision qu'on n'a pas prise, et un collecteur ajouté sans elle verrait ses
/// items silencieusement rangés parmi ceux qui ne se déclarent jamais. Le
/// contributeur est arrêté par le compilateur, pas par une consigne de
/// relecture.
fn observed(
    path: &str,
    domain: Domain,
    nature: Nature,
    value: ItemValue,
    purpose: &str,
    risk: &str,
) -> Item {
    Item {
        path: path.to_owned(),
        domain,
        nature,
        desired: None,
        observed: value,
        observed_at: Utc::now(),
        // `Observed`, et surtout pas `Keystone` : un collecteur *lit*, il ne produit
        // pas la valeur. Marquer un relevé « Keystone » revenait à prétendre que
        // l'outil est l'auteur de la version de l'OS ou du taux de remplissage d'un
        // disque — et rendait `Unknown` inatteignable, donc le signal de sécurité
        // du modèle inopérant.
        provenance: Provenance::Observed,
        purpose: purpose.to_owned(),
        risk: risk.to_owned(),
        reference: None,
    }
}

/// Collecteur matériel portable — CPU, mémoire, stockage, temps de fonctionnement.
///
/// Volontairement portable : il tourne sur l'hôte Windows, dans une distro WSL2 et
/// dans une VM Linux avec le même code, ce qui donne une vue consolidée multi-OS
/// sans dupliquer la logique.
pub struct HardwareCollector;

impl Collector for HardwareCollector {
    fn id(&self) -> &'static str {
        "hardware"
    }

    fn domain(&self) -> Domain {
        Domain::Inventory
    }

    fn collect(&self) -> Vec<Item> {
        use sysinfo::{Disks, System};

        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        // Rien de ce collecteur ne se déclare. Le système, le noyau, le nom de
        // la machine, le nombre de cœurs et la mémoire sont des faits qu'on
        // subit — on ne « veut » pas seize cœurs, on en a seize. Le temps de
        // fonctionnement et le taux d'occupation, eux, bougent seuls : les
        // déclarer par égalité produirait un écart à chaque scan.
        let mut items = vec![
            observed(
                "inventory.os.name",
                Domain::Inventory,
                Nature::Constat,
                ItemValue::Text(System::long_os_version().unwrap_or_else(|| "inconnu".into())),
                "Système et version, pour situer la machine dans la matrice de compatibilité.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.os.kernel",
                Domain::Inventory,
                Nature::Constat,
                ItemValue::Text(System::kernel_version().unwrap_or_else(|| "inconnu".into())),
                "Version du noyau — sert à détecter une distro WSL en retard.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.host.name",
                Domain::Inventory,
                Nature::Constat,
                ItemValue::Text(System::host_name().unwrap_or_else(|| "inconnu".into())),
                "Nom de la machine, clé de la surcouche de flotte (D13-01).",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.cpu.cores",
                Domain::Inventory,
                Nature::Constat,
                ItemValue::Int(i64::try_from(sys.cpus().len()).unwrap_or(-1)),
                "Nombre de cœurs logiques, base des plafonds de ressources des VM.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.memory.total_bytes",
                Domain::Inventory,
                Nature::Constat,
                ItemValue::Int(i64::try_from(sys.total_memory()).unwrap_or(-1)),
                "Mémoire totale, contrainte des profils WSL et Hyper-V.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.uptime_seconds",
                Domain::Inventory,
                // Il augmente d'une seconde par seconde : l'archétype de la mesure.
                Nature::Mesure,
                ItemValue::Int(i64::try_from(System::uptime()).unwrap_or(-1)),
                "Temps depuis le dernier démarrage — utile pour les correctifs en attente.",
                "Aucun — lecture seule.",
            ),
        ];

        // Occupation par volume : le socle du domaine D4 (attribution de l'espace).
        // Ici on ne fait que constater. L'attribution par consommateur — qui mange
        // réellement le disque — arrive en Phase 3.
        for disk in Disks::new_with_refreshed_list().list() {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let total = disk.total_space();
            let used = total.saturating_sub(disk.available_space());
            // Un volume de taille nulle (lecteur amovible absent) ne donne pas 0 % :
            // il ne donne rien. Un inventaire qui invente est pire qu'un inventaire
            // incomplet — d'où la chaîne de `checked_*` plutôt qu'une valeur par défaut.
            let pct = used
                .checked_mul(100)
                .and_then(|v| v.checked_div(total))
                .and_then(|v| i64::try_from(v).ok());
            let Some(pct) = pct else { continue };
            items.push(observed(
                &format!("space.volume[{mount}].used_percent"),
                Domain::Space,
                // Le seul cas d'usage réel d'une mesure contrainte (« au plus
                // 85 »). Un seul ne suffit pas à écrire le vocabulaire qui
                // permettrait de la déclarer : c'est la dette prise par
                // l'ADR-0009, et sa condition de remboursement.
                Nature::Mesure,
                ItemValue::Int(pct),
                "Taux d'occupation du volume, base de la projection de saturation.",
                "Aucun — lecture seule.",
            ));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_collecteur_materiel_produit_des_items() {
        let items = HardwareCollector.collect();
        assert!(
            !items.is_empty(),
            "un scan doit toujours produire quelque chose"
        );
    }

    #[test]
    fn aucun_item_collecte_nest_declare_desire() {
        // Un collecteur observe ; il ne décide pas de ce qui est voulu.
        // Si cette assertion casse, quelqu'un a mélangé collecte et intention.
        for item in HardwareCollector.collect() {
            assert!(
                item.desired.is_none(),
                "« {} » : un collecteur n'écrit pas d'état désiré",
                item.path
            );
            assert!(
                !item.verdict().demande_convergence(),
                "et il ne peut donc pas produire d'écart"
            );
        }
    }

    /// Ce qu'un relevé n'a **jamais** le droit de porter, et ce qu'il encadre.
    ///
    /// Ce test remplace une égalité à `Provenance::Observed`. Elle tenait
    /// **trois** choses à la fois, et deux seulement méritaient d'être tenues :
    ///
    /// * `Keystone` est faux — Keystone n'a pas *fait* la version de l'OS ni le
    ///   remplissage du disque, il les a *lus*. C'est la faute déjà commise, et
    ///   sa conséquence était lourde : `Unknown` devenait inatteignable, donc
    ///   `is_security_signal` toujours faux, donc le signal le plus valorisé du
    ///   modèle structurellement mort ;
    /// * `Unknown` est faux aussi, et pour la raison symétrique — un **relevé**
    ///   n'est pas un **changement**. Le signal de D2-05 naît d'une comparaison
    ///   entre deux relevés, jamais d'un seul.
    ///
    /// La troisième chose, l'interdiction de `Managed`, ne se tenait pas : elle
    /// maintenait `DriftStatus::Conflict` inconstructible et le principe P10
    /// sans support (ADR-0018). Elle est remplacée par un **encadrement** —
    /// `Managed` ne se produit que sur un chemin de [`politique::CHEMINS`].
    ///
    /// # Le `match` est exhaustif **sans bras `_`**
    ///
    /// Et sans bras de liaison non plus : `autre => panic!(…)` aurait été un
    /// `_` déguisé, qui laisserait passer une variante nouvelle sans casser la
    /// compilation. Les cinq variantes interdites sont donc nommées une par
    /// une. Ajouter une valeur à `Provenance` casse **ici** la compilation,
    /// donc la CI, avant qu'un test s'exécute : le contributeur est forcé de
    /// venir décider si un collecteur a le droit de la produire.
    #[test]
    fn un_releve_nest_ni_lauteur_de_la_valeur_ni_un_signal() {
        let items = Inventory::collect_all().items;
        assert!(!items.is_empty(), "sans item, ce test n'éprouve rien");

        for item in &items {
            assert_ne!(
                item.provenance,
                Provenance::Keystone,
                "« {} » : l'outil n'est l'auteur d'aucune valeur qu'il a lue",
                item.path
            );
            assert!(
                !item.provenance.is_security_signal(),
                "« {} » : un simple relevé ne doit rien déclencher — le signal \
                 naît d'une comparaison entre deux relevés",
                item.path
            );

            match &item.provenance {
                Provenance::Observed => {}
                Provenance::Managed(autorite) => {
                    assert!(
                        politique::CHEMINS.contains(&item.path.as_str()),
                        "« {} » se dit imposé par une autorité sans être lu sous une \
                         ruche de politique — voir `politique::CHEMINS`",
                        item.path
                    );
                    assert_eq!(
                        autorite,
                        politique::AUTORITE,
                        "« {} » nomme une autorité qu'on n'a pas identifiée",
                        item.path
                    );
                }
                Provenance::Keystone
                | Provenance::Unknown
                | Provenance::Human(_)
                | Provenance::WindowsUpdate
                | Provenance::Application(_) => panic!(
                    "« {} » : provenance interdite pour un relevé — {:?}",
                    item.path, item.provenance
                ),
            }
        }
    }

    /// Un chemin de la ruche dont la valeur **a été lue** est marqué `Managed`.
    ///
    /// C'est la barrière qui garde `Managed` **atteignable**. Sans elle,
    /// retirer l'appel à `politique::marquer` du collecteur laisserait tous les
    /// tests verts : le test ci-dessus n'exige aucune occurrence de `Managed`,
    /// il encadre celles qui existent. On retomberait alors dans la situation
    /// d'avant — un mécanisme mort qu'on croit vivant parce qu'il est testé.
    #[test]
    fn un_chemin_de_la_ruche_dont_la_valeur_est_lue_ressort_managed() {
        let items = Inventory::collect_all().items;

        for chemin in politique::CHEMINS {
            let Some(item) = items.iter().find(|i| i.path == *chemin) else {
                // Hors Windows, aucun collecteur de posture ne tourne. L'absence
                // du chemin est alors normale, et l'assertion de présence vit
                // sous `cfg(windows)` juste en dessous.
                continue;
            };
            // La règle, appliquée à l'item réellement produit : si une valeur a
            // été lue, l'autorité est attestée ; sinon, rien ne l'est.
            let attendu =
                if item.observed == ks_core::ItemValue::Absent || !item.observed.est_constat() {
                    Provenance::Observed
                } else {
                    Provenance::Managed(politique::AUTORITE.to_owned())
                };
            assert_eq!(
                item.provenance, attendu,
                "« {chemin} » vaut {} : la provenance ne suit pas la lecture",
                item.observed
            );
        }

        // Et l'ancrage ne pourrit pas : chaque chemin déclaré existe encore
        // dans l'inventaire. Un item renommé y laisserait sinon une liste qui
        // ne désigne plus rien, donc une barrière qui ne barre plus.
        #[cfg(windows)]
        for chemin in politique::CHEMINS {
            assert!(
                items.iter().any(|i| i.path == *chemin),
                "« {chemin} » est déclaré lu sous la ruche de politique mais \
                 n'existe plus dans l'inventaire"
            );
        }
    }

    #[test]
    fn tout_item_porte_son_explication() {
        // Principe P6 : un item qu'on ne peut pas expliquer n'est pas affichable.
        for item in Inventory::collect_all().items {
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert!(
                !item.risk.is_empty(),
                "« {} » sans risque documenté",
                item.path
            );
        }
    }

    #[test]
    fn aucun_item_dinventaire_ni_despace_na_vocation_a_etre_declare() {
        // La moitié « ce qui ne se déclare pas » de l'ADR-0009, éprouvée là où
        // elle porte le plus : ces deux collecteurs produisent 62 des 115 items
        // de la machine de référence, dont les 54 versions d'applications.
        //
        // Les y déclarer donnerait un `workstation.yaml` que personne ne relit,
        // et un écart par mise à jour de navigateur — pire, un écart que la
        // valeur ne montre même pas, la version vivant dans le CHEMIN de l'item
        // (`inventory.software[dbeaver2530currentuser].version`).
        let items: Vec<Item> = HardwareCollector
            .collect()
            .into_iter()
            .chain(SoftwareCollector::items())
            .collect();
        assert!(!items.is_empty(), "sans item, ce test n'éprouve rien");

        for item in &items {
            // Le `match` exhaustif sans bras `_` est la barrière : ajouter une
            // variante à `Nature` casse ici la COMPILATION, donc la CI, avant
            // qu'un test s'exécute. Le contributeur doit venir décider ce que
            // sa nouvelle nature fait de ces items-là.
            let attendu = match item.nature {
                Nature::Constat | Nature::Mesure => false,
                Nature::Reglage | Nature::Objectif => true,
            };
            assert!(
                !attendu,
                "« {} » : {:?} — l'inventaire logiciel et l'espace se constatent, \
                 ils ne se déclarent pas",
                item.path, item.nature
            );
            assert_eq!(item.nature.est_declarable(), attendu);
            assert!(
                !item.nature.est_convergeable(),
                "« {} » : aucun verbe n'écrira la version d'une application",
                item.path
            );
        }
    }

    /// Les items déclarables dont la valeur textuelle est **recopiée du système**.
    ///
    /// L'exemption est nommée, courte, et justifiée un par un — pas un
    /// élargissement du contrôle. Ces deux valeurs ne sortent d'aucune table de
    /// codes : ce sont des identifiants Windows rendus tels quels, exactement
    /// comme `inventory.os.name` vaut « Windows 11 Home ». Les traduire en
    /// jetons reviendrait à inventer un vocabulaire par-dessus celui de
    /// Microsoft, et à le faire dériver au premier fuseau ajouté.
    ///
    /// L'ADR-0015 les rangeait parmi les `Constat`, ce qu'ils ne sont pas : ils
    /// se déclarent, et c'est bien pour cela qu'ils apparaissent ici plutôt que
    /// d'être exemptés par leur nature.
    const VALEURS_BRUTES_DU_SYSTEME: &[&str] = &[
        // `NtpServer` vaut « time.windows.com,0x9 » : un nom d'hôte suivi d'un
        // masque de drapeaux, tous deux définis par Windows.
        "security.clock.ntp_server",
        // `TimeZoneKeyName` vaut « Romance Standard Time » : la clé de fuseau
        // de Windows, avec ses majuscules et ses espaces.
        "security.clock.timezone",
    ];

    /// **Aucune phrase française n'entre dans un item déclarable** (ADR-0015).
    ///
    /// Le contrôle est mécanique, et il porte là où la règle se viole : au
    /// point de construction. Douze items se comparaient sur une phrase — une
    /// virgule retirée les faisait tous basculer en écart, sur une machine où
    /// rien n'avait bougé, et l'effet était rétroactif sur le magasin
    /// d'observations.
    ///
    /// Les `Constat` en sont **exemptés** : `inventory.os.name` vaut « Windows
    /// 11 Home », et c'est la valeur exacte donnée par le système, pas une
    /// traduction que nous aurions choisie.
    ///
    /// Sa limite est assumée, comme celle de la barrière SEC-02 : il refuse
    /// l'espace, la majuscule et l'accent, il ne refuse ni un jeton mal choisi
    /// ni deux codes distincts traduits par le même jeton. C'est la revue qui
    /// les attrape.
    #[test]
    fn aucun_item_declarable_ne_porte_une_phrase_francaise() {
        let items = Inventory::collect_all().items;
        assert!(!items.is_empty(), "sans item, ce test n'éprouve rien");

        let mut controles = 0;
        for item in &items {
            // `match` exhaustif sans bras `_` : ajouter une variante à `Nature`
            // casse ici la COMPILATION, donc la CI, avant qu'un test s'exécute.
            // Le contributeur doit venir décider si sa nouvelle nature se
            // compare — donc si elle exige un jeton.
            let se_compare = match item.nature {
                Nature::Reglage | Nature::Objectif => true,
                Nature::Mesure | Nature::Constat => false,
            };
            let ItemValue::Text(valeur) = &item.observed else {
                continue;
            };
            if !se_compare || VALEURS_BRUTES_DU_SYSTEME.contains(&item.path.as_str()) {
                continue;
            }
            controles += 1;
            assert!(
                jetons::est_bien_forme(valeur),
                "« {} » vaut « {valeur} » : un item {:?} se compare d'un scan à \
                 l'autre, il porte donc un jeton — minuscules, sans accent, sans \
                 espace — et sa phrase française vit dans `ks_cli::lisible`",
                item.path,
                item.nature
            );
        }

        // Un contrôle qui ne contrôle rien passerait tout aussi vert. Sur
        // Windows, les items de posture en fournissent une douzaine ; ailleurs
        // le collecteur ne produit rien et l'assertion serait fausse.
        #[cfg(windows)]
        assert!(
            controles >= 10,
            "seulement {controles} valeurs textuelles déclarables contrôlées : \
             la barrière ne barre plus grand-chose"
        );
        #[cfg(not(windows))]
        let _ = controles;

        // Et l'exemption ne pourrit pas : chaque chemin cité existe encore, et
        // il est encore déclarable. Un item renommé ou reclassé y laisserait
        // sinon un trou dont plus personne ne connaîtrait la raison.
        #[cfg(windows)]
        for chemin in VALEURS_BRUTES_DU_SYSTEME {
            let item = items
                .iter()
                .find(|i| i.path == *chemin)
                .unwrap_or_else(|| panic!("« {chemin} » est exempté mais n'existe plus"));
            assert!(
                item.nature.est_declarable(),
                "« {chemin} » n'est plus déclarable : l'exemption n'a plus d'objet"
            );
        }
    }

    #[test]
    fn linventaire_agrege_plusieurs_domaines() {
        let inv = Inventory::collect_all();
        assert!(!inv.by_domain(Domain::Inventory).is_empty());
    }

    /// Le commentaire de tête cite-t-il tous les collecteurs qui existent ?
    ///
    /// Trois modules ont été ajoutés sans que la liste bouge, malgré une
    /// consigne de relecture écrite en toutes lettres au-dessus d'elle. Une
    /// consigne ne retient rien : elle s'adresse à quelqu'un qui a déjà oublié
    /// de la lire.
    ///
    /// La barrière est textuelle et autonome — les quatre `impl Collector for`
    /// vivent dans ce fichier, donc `include_str!` suffit à confronter la
    /// documentation à son propre code, sans dépendance ni macro. Elle est
    /// éprouvée par falsification : ajouter un `impl Collector for` non cité
    /// fait échouer ce test.
    #[test]
    fn la_liste_des_collecteurs_est_a_jour() {
        const SOURCE: &str = include_str!("lib.rs");

        let implementes: std::collections::BTreeSet<&str> = SOURCE
            .lines()
            .filter_map(|l| l.trim().strip_prefix("impl Collector for "))
            .filter_map(|reste| reste.split_whitespace().next())
            .collect();

        // La documentation cite chaque collecteur par un lien intra-doc.
        let cites: std::collections::BTreeSet<&str> = SOURCE
            .lines()
            .take_while(|l| l.starts_with("//!"))
            .flat_map(|l| {
                l.match_indices("[`").map(move |(i, _)| {
                    let reste = &l[i + 2..];
                    &reste[..reste.find('`').unwrap_or(reste.len())]
                })
            })
            .filter(|nom| nom.ends_with("Collector") && *nom != "Collector")
            .collect();

        assert!(
            !implementes.is_empty(),
            "aucun `impl Collector for` trouvé — la barrière ne barre plus rien"
        );
        assert_eq!(
            implementes, cites,
            "le commentaire de tête et les implémentations divergent : 
               implémentés {implementes:?}
  cités        {cites:?}"
        );
    }
}

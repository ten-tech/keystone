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

pub mod etat_effectif;
pub mod posture;
pub mod software;
pub mod virtualisation;
pub mod winget;

use chrono::Utc;
use ks_core::{Domain, Item, ItemValue, Provenance};
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

/// Un inventaire complet, à un instant donné.
#[derive(Debug, Default)]
pub struct Inventory {
    /// Les items observés.
    pub items: Vec<Item>,
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
        for c in &collectors {
            items.extend(c.collect());
        }
        Self { items }
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

/// Construit un item observé, avec sa provenance et son explication.
///
/// Les champs `purpose` et `risk` ne sont pas décoratifs : l'exigence D1-10 interdit
/// d'afficher un item sans provenance, et le principe P6 (explicabilité) interdit
/// d'afficher un item que l'utilisateur ne peut pas comprendre. Le constructeur les
/// rend donc obligatoires — on ne peut pas les oublier.
fn observed(path: &str, domain: Domain, value: ItemValue, purpose: &str, risk: &str) -> Item {
    Item {
        path: path.to_owned(),
        domain,
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

        let mut items = vec![
            observed(
                "inventory.os.name",
                Domain::Inventory,
                ItemValue::Text(System::long_os_version().unwrap_or_else(|| "inconnu".into())),
                "Système et version, pour situer la machine dans la matrice de compatibilité.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.os.kernel",
                Domain::Inventory,
                ItemValue::Text(System::kernel_version().unwrap_or_else(|| "inconnu".into())),
                "Version du noyau — sert à détecter une distro WSL en retard.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.host.name",
                Domain::Inventory,
                ItemValue::Text(System::host_name().unwrap_or_else(|| "inconnu".into())),
                "Nom de la machine, clé de la surcouche de flotte (D13-01).",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.cpu.cores",
                Domain::Inventory,
                ItemValue::Int(i64::try_from(sys.cpus().len()).unwrap_or(-1)),
                "Nombre de cœurs logiques, base des plafonds de ressources des VM.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.memory.total_bytes",
                Domain::Inventory,
                ItemValue::Int(i64::try_from(sys.total_memory()).unwrap_or(-1)),
                "Mémoire totale, contrainte des profils WSL et Hyper-V.",
                "Aucun — lecture seule.",
            ),
            observed(
                "inventory.uptime_seconds",
                Domain::Inventory,
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

    #[test]
    fn un_releve_ne_sattribue_pas_la_paternite_de_la_valeur() {
        // Keystone n'a pas *fait* la version de l'OS ni le remplissage du disque :
        // il les a *lus*. Marquer cela `Keystone` était faux, et avait une
        // conséquence lourde — `Unknown` devenait inatteignable, donc
        // `is_security_signal` toujours faux, donc le signal le plus valorisé du
        // modèle structurellement mort en Phase 0.
        for item in Inventory::collect_all().items {
            assert_eq!(
                item.provenance,
                Provenance::Observed,
                "« {} » : un item collecté est relevé, pas produit",
                item.path
            );
            assert!(
                !item.provenance.is_security_signal(),
                "« {} » : un simple relevé ne doit rien déclencher",
                item.path
            );
            assert!(
                !item.provenance.is_sovereign(),
                "« {} » : un relevé n'est pas une politique gérée",
                item.path
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

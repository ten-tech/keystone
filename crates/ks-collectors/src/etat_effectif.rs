//! État **effectif** des protections, par WMI (D5, Phase 0.2, ADR-0005).
//!
//! ## Pourquoi ce module double le collecteur de registre
//!
//! Le registre ne dit que la configuration. Une machine où l'intégrité mémoire
//! est configurée mais dont le noyau sécurisé n'a pas démarré — pilote
//! incompatible, refus côté hyperviseur, matériel non éligible — porte
//! **exactement la même configuration** qu'une machine protégée.
//!
//! `Win32_DeviceGuard` les sépare, et la table documentée par Microsoft le dit
//! sans ambiguïté : `VirtualizationBasedSecurityStatus` vaut `1` pour « enabled
//! but not running » et `2` pour « enabled and running ». Le registre ne porte
//! pas cette nuance. C'est toute la raison de ce module.
//!
//! ## Ce qui est lu, et ce qui ne l'est pas
//!
//! Trois classes, mesurées **lisibles en session non élevée** sur une machine
//! réelle — c'est-à-dire dans les conditions où la CLI s'exécute (SEC-01) :
//! `Win32_DeviceGuard`, `MSFT_MpComputerStatus` et `Win32_Service`.
//!
//! `Win32_Tpm` et `Win32_EncryptableVolume` renvoient « accès refusé ». Ils ne
//! butent pas sur une API manquante mais sur une liste de contrôle d'accès :
//! aucun choix de bibliothèque ne les rendrait lisibles ici. Ils appartiennent
//! au broker, donc à la Phase 2.
//!
//! ## `Win32_Service`, et l'angle mort qu'il comble
//!
//! Le registre porte le **type de démarrage** d'un service, jamais son
//! exécution. Un service réglé sur « automatique » puis arrêté à la main garde
//! sa valeur `Start` inchangée : le collecteur de registre l'affiche conforme,
//! et il ne protège rien. Mesuré sur la machine de référence le 2026-08-17 :
//! **neuf services y sont en démarrage automatique sans s'exécuter.**
//!
//! C'est le même écart que `vbs_policy` contre `vbs_running`, sur les services
//! que le §6 du modèle de menace place au premier rang. `Win32_Service` porte
//! `StartMode` **et** `State`, et il répond sans élévation.
//!
//! ## Aucune méthode WMI n'est invoquée
//!
//! Uniquement des requêtes de lecture. Invoquer une méthode WMI serait un effet
//! de bord, ce qu'un collecteur n'a pas le droit de produire — la règle du crate
//! ne parle pas que d'écriture disque.
//!
//! ## Le fil dédié, et pourquoi il n'est pas une précaution excessive
//!
//! Initialiser COM est un effet **observable sur le fil appelant**, et la
//! bibliothèque ne le défait jamais : elle documente que `CoUninitialize` n'est
//! délibérément pas appelé au `Drop`, pour éviter d'invalider des pointeurs COM
//! encore vivants ailleurs dans le processus.
//!
//! Conséquence : le fil qui interroge WMI reste initialisé en appartement
//! multifilière jusqu'à sa mort. On lui en dédie donc un, qu'on laisse mourir.
//! Le fil principal de la CLI n'est jamais touché, et un composant qui
//! exigerait un appartement cloisonné ne se retrouve pas empêché par un
//! collecteur.

use ks_core::ItemValue;

use crate::jetons::TableDeCodes;
use crate::posture::Lecture;

/// Ce que `Win32_DeviceGuard` rapporte, réduit à ce qu'on publie.
///
/// Tous les champs sont optionnels : une classe WMI peut omettre une propriété
/// selon la version de Windows, et un champ manquant doit donner une absence,
/// jamais une valeur par défaut. `0` voudrait dire « éteint », ce qui serait le
/// faux négatif habituel.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EtatPlateforme {
    /// `VirtualizationBasedSecurityStatus` : 0 éteint, 1 configuré sans tourner, 2 en cours.
    pub vbs: Lecture<u32>,
    /// `SecurityServicesRunning` — les services réellement en cours d'exécution.
    pub services_actifs: Lecture<Vec<u32>>,
    /// `SecurityServicesConfigured` — ceux qui sont demandés.
    pub services_configures: Lecture<Vec<u32>>,
    /// `CodeIntegrityPolicyEnforcementStatus` : 0 éteint, 1 audit, 2 imposé.
    pub integrite_code: Lecture<u32>,
    /// `AvailableSecurityProperties` — ce que le matériel sait faire.
    pub proprietes_disponibles: Lecture<Vec<u32>>,
}

/// L'état d'exécution des services de la machine, par nom **en minuscules**.
///
/// # La casse, et pourquoi elle est normalisée à la lecture
///
/// Les noms de services Windows sont insensibles à la casse, et les sources ne
/// s'accordent pas : `SERVICES_SURVEILLES` écrit `MpsSvc`, `Win32_Service`
/// rapporte `mpssvc`. Une comparaison littérale ne trouverait pas la ligne,
/// donc publierait une **absence** — c'est-à-dire « ce service n'est pas
/// installé » — sur un pare-feu qui tourne. C'est le défaut `wscvc` à
/// l'identique, avec la casse à la place d'une lettre manquante.
///
/// # Pourquoi l'état reste une chaîne brute
///
/// La valeur est `Win32_Service.State` telle que WMI la rend, sans
/// interprétation. Celle-ci vit dans [`execution_service`], donc dans une
/// fonction pure qu'un test éprouve sans WMI ni machine Windows — c'est la
/// règle que le collecteur de registre applique déjà à ses valeurs.
///
/// `None` dit que la ligne existe mais que la classe n'a rapporté **aucun**
/// état pour elle. Ce n'est pas « à l'arrêt » : c'est une lecture manquante, et
/// elle ressort illisible.
pub type ServicesObserves = std::collections::BTreeMap<String, Option<String>>;

/// La valeur de `Win32_Service.State` qui signifie « le service s'exécute ».
///
/// Mesurée sur la machine de référence le 2026-08-17, en session non élevée et
/// sur un Windows en français : la propriété rend `Running` et `Stopped`, non
/// traduits. La comparaison est néanmoins **insensible à la casse**, parce que
/// rien ne contractualise l'inverse.
const ETAT_EN_EXECUTION: &str = "Running";

/// Ce que `MSFT_MpComputerStatus` rapporte, réduit à ce qu'on publie.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EtatDefender {
    /// La protection en temps réel tourne-t-elle **réellement** ?
    pub temps_reel: Lecture<bool>,
    /// La protection contre l'altération est-elle active ?
    pub anti_alteration: Lecture<bool>,
    /// La surveillance comportementale tourne-t-elle ?
    pub surveillance_comportementale: Lecture<bool>,
}

/// Lit les deux classes.
///
/// Ne renvoie jamais d'erreur : un échec WMI — service arrêté, dépôt corrompu,
/// accès refusé — se range en [`Lecture::Refusee`], et l'item affiche
/// « illisible ». Faire échouer le scan entier pour une classe indisponible
/// priverait l'utilisateur des cent autres items.
#[must_use]
pub fn lire() -> (EtatPlateforme, EtatDefender) {
    #[cfg(windows)]
    {
        windows_impl::lire()
    }
    #[cfg(not(windows))]
    {
        (EtatPlateforme::default(), EtatDefender::default())
    }
}

/// Lit l'exécution effective de **tous** les services de la machine.
///
/// # Une seule requête, et pas une par service
///
/// `Win32_Service` s'énumère d'un coup, et la bibliothèque ne demande que les
/// colonnes de la projection (`SELECT Name,State FROM Win32_Service`). Six
/// requêtes filtrées coûteraient six allers-retours COM pour la même
/// information ; la liste des services surveillés grandira, la requête non.
///
/// # Pourquoi ce n'est pas [`lire`] qui la rend
///
/// Le fil dédié porte un **délai** (`DELAI_WMI`, propre à Windows, donc cité
/// sans lien pour que cette page se construise aussi ailleurs) qui borne
/// l'attente. Le
/// partager avec les deux autres classes rendrait une énumération lente des
/// services capable de faire passer VBS, l'intégrité mémoire, Credential Guard
/// et la protection DMA en « illisible » d'un coup. Un budget par lecture garde
/// les échecs indépendants, ce qui est exactement ce que [`crate::attribution`]
/// fait déjà de son côté.
///
/// Ne renvoie jamais d'erreur : un échec se range en [`Lecture::Refusee`], et
/// l'item affiche « illisible » plutôt qu'un service à l'arrêt qu'on n'a pas vu.
#[must_use]
pub fn lire_services() -> Lecture<ServicesObserves> {
    #[cfg(windows)]
    {
        windows_impl::lire_services()
    }
    #[cfg(not(windows))]
    {
        // Hors Windows il n'y a pas de service Windows à lire. `Absente` et
        // non `Trouvee(vide)` : une table vide dirait « aucun de ces services
        // n'est installé », ce qui est un constat qu'on n'a pas fait.
        Lecture::Absente
    }
}

/// Les codes de `AvailableSecurityProperties` dont on se sert.
pub mod propriete_materielle {
    /// La protection DMA est **disponible** — ce qui ne dit pas qu'elle est active.
    pub const PROTECTION_DMA: u32 = 3;
}

/// Délai au-delà duquel on cesse d'attendre WMI.
///
/// La bibliothèque `wmi` énumère ses résultats avec `WBEM_INFINITE`
/// (`result_enumerator.rs`, vérifié dans la version verrouillée) : un dépôt
/// WMI en réparation ou un `winmgmt` figé suspend l'appel sans borne, et un
/// fil Rust bloqué ne se tue pas. Sans ce délai, `ks status` attendait
/// indéfiniment, en silence — ce que NF-01 interdit.
///
/// **Cinq secondes, et la marge est délibérée.** La lecture des deux classes
/// a été mesurée à 145, 166 et 193 ms sur la machine de référence, dépôt
/// WMI chaud — soit un rapport de 25 à 1. La marge ne sert pas au cas
/// nominal : elle couvre un `winmgmt` qui démarre à froid, et surtout elle
/// reconnaît que le coût d'un délai trop court n'est pas symétrique. Trop
/// court, on affiche « illisible » sur un item de sécurité parfaitement
/// lisible ; trop long, on attend. Le premier est un faux constat, le second
/// une gêne. Ce délai n'existe que pour borner une attente **infinie**, pas
/// pour optimiser le cas courant.
#[cfg(windows)]
pub(crate) const DELAI_WMI: std::time::Duration = std::time::Duration::from_secs(5);

/// Interroge WMI sur un fil dédié, puis laisse ce fil mourir.
///
/// L'isolement n'est pas cosmétique. `COMLibrary::new()` initialise COM en
/// appartement multifilière **sur le fil appelant**, et la bibliothèque
/// documente qu'elle n'appelle jamais `CoUninitialize` au `Drop` — pour ne
/// pas invalider des pointeurs COM encore vivants ailleurs. L'initialisation
/// survit donc à la requête. En la confinant à un fil qu'on abandonne
/// ensuite, le processus hôte ressort exactement comme il est entré.
///
/// Le fil n'est **pas** attendu par `join` : au-delà de [`DELAI_WMI`] on
/// l'abandonne et l'on rend `repli`. Un fil abandonné ne retient pas le
/// processus, et le repli dit la vérité — on n'a pas lu.
///
/// # Ce que ce repli ne couvre pas
///
/// Une **panique** à l'intérieur d'`interroger` ne se rattrape pas ici : le
/// profil de release porte `panic = "abort"` (`Cargo.toml`), donc il n'y a pas
/// de déroulement de pile et le processus meurt. La rédaction précédente
/// affirmait l'inverse — qu'un `join` en erreur suffisait — ce qui était vrai
/// en `dev` et faux dans le binaire livré. La sûreté repose donc sur le fait
/// qu'`interroger` ne panique pas : aucun `unwrap`, aucune indexation, aucune
/// arithmétique non bornée. Cette propriété est une **contrainte de
/// relecture**, pas une garantie du compilateur.
///
/// # Pourquoi cette fonction est partagée
///
/// Deux appelants l'utilisent : la lecture d'état effectif ci-dessous et la
/// liste des correctifs de [`crate::attribution`]. Deux copies de ce raisonnement
/// divergeraient au premier correctif, et la seconde perdrait le délai ou le
/// fil dédié sans que rien ne le signale.
#[cfg(windows)]
pub(crate) fn sur_un_fil_dedie<T: Send + 'static>(
    interroger: impl FnOnce() -> T + Send + 'static,
    repli: impl FnOnce() -> T,
) -> T {
    let (envoi, reception) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // L'envoi échoue si le récepteur a déjà rendu la main sur délai.
        // C'est le cas normal, pas une erreur : le fil n'a plus de lecteur.
        let _ = envoi.send(interroger());
    });
    reception
        .recv_timeout(DELAI_WMI)
        .unwrap_or_else(|_| repli())
}

#[cfg(windows)]
mod windows_impl {
    use super::{EtatDefender, EtatPlateforme};
    use crate::posture::Lecture;
    use serde::Deserialize;

    /// Projection de `Win32_DeviceGuard`.
    ///
    /// `PascalCase` correspond à la convention de nommage des propriétés WMI.
    /// Les champs restent optionnels : une propriété absente d'une version de
    /// Windows ne doit pas faire échouer la désérialisation de tout le reste.
    /// **Le nom sérialisé est le nom de la classe WMI interrogée**, pas une
    /// coquetterie : la bibliothèque construit `SELECT * FROM <nom>` à partir de
    /// lui. Sans ce `rename`, elle cherchait `DeviceGuard` et Windows répondait
    /// `0x80041010`, « classe invalide » — un échec que le collecteur traduisait
    /// fidèlement en « illisible », donc invisible sans instrumenter.
    #[derive(Deserialize, Debug)]
    #[serde(rename = "Win32_DeviceGuard", rename_all = "PascalCase")]
    struct DeviceGuard {
        virtualization_based_security_status: Option<u32>,
        security_services_running: Option<Vec<u32>>,
        security_services_configured: Option<Vec<u32>>,
        code_integrity_policy_enforcement_status: Option<u32>,
        available_security_properties: Option<Vec<u32>>,
    }

    /// Projection de `MSFT_MpComputerStatus`.
    #[derive(Deserialize, Debug)]
    #[serde(rename = "MSFT_MpComputerStatus", rename_all = "PascalCase")]
    struct MpComputerStatus {
        real_time_protection_enabled: Option<bool>,
        is_tamper_protected: Option<bool>,
        behavior_monitor_enabled: Option<bool>,
    }

    /// Projection de `Win32_Service`.
    ///
    /// Deux colonnes, et pas une de plus : la bibliothèque construit sa requête
    /// à partir des champs de la structure, donc chaque champ ajouté ici est une
    /// colonne demandée à WMI pour les quelque trois cents services de la
    /// machine. `StartMode` n'y figure pas — le type de démarrage se lit déjà au
    /// registre, et deux sources pour un même fait divergeraient.
    #[derive(Deserialize, Debug)]
    #[serde(rename = "Win32_Service", rename_all = "PascalCase")]
    struct Service {
        name: Option<String>,
        state: Option<String>,
    }

    /// Une valeur présente est un constat ; une valeur absente n'en est pas un.
    fn depuis<T>(valeur: Option<T>) -> Lecture<T> {
        valeur.map_or(Lecture::Absente, Lecture::Trouvee)
    }

    /// Lit les deux classes sur un fil dédié — voir [`super::sur_un_fil_dedie`],
    /// qui porte le raisonnement sur COM, sur le délai, et sur ce que le repli
    /// ne couvre pas.
    pub(super) fn lire() -> (EtatPlateforme, EtatDefender) {
        super::sur_un_fil_dedie(interroger, || (refus_plateforme(), refus_defender()))
    }

    fn interroger() -> (EtatPlateforme, EtatDefender) {
        use wmi::{COMLibrary, WMIConnection};

        // Un échec ici — COM déjà initialisé dans un autre appartement sur ce
        // fil, par exemple — est un refus de lecture, pas une absence de
        // protection. La distinction est la doctrine du module voisin.
        let Ok(com) = COMLibrary::new() else {
            return (refus_plateforme(), refus_defender());
        };

        let plateforme =
            WMIConnection::with_namespace_path(r"root\Microsoft\Windows\DeviceGuard", com)
                .ok()
                .and_then(|c| c.query::<DeviceGuard>().ok())
                .and_then(|mut v| v.pop())
                .map_or_else(refus_plateforme, |d| EtatPlateforme {
                    vbs: depuis(d.virtualization_based_security_status),
                    services_actifs: depuis(d.security_services_running),
                    services_configures: depuis(d.security_services_configured),
                    integrite_code: depuis(d.code_integrity_policy_enforcement_status),
                    proprietes_disponibles: depuis(d.available_security_properties),
                });

        // Une seconde *connexion*, pas une seconde initialisation : les deux
        // classes vivent dans des espaces de noms distincts et une connexion WMI
        // en vise un seul, mais COM n'a besoin d'être initialisé qu'une fois par
        // fil. `COMLibrary` est `Copy`, on réemploie donc le jeton. Le second
        // `COMLibrary::new()` qui figurait ici refaisait `CoInitializeSecurity`,
        // lequel renvoyait `RPC_E_TOO_LATE` — avalé par la bibliothèque, donc
        // sans effet visible, mais le commentaire qui le justifiait confondait
        // « une connexion par espace de noms » et « une initialisation par fil ».
        let defender = WMIConnection::with_namespace_path(r"root\Microsoft\Windows\Defender", com)
            .ok()
            .and_then(|c| c.query::<MpComputerStatus>().ok())
            .and_then(|mut v| v.pop())
            .map_or_else(refus_defender, |m| EtatDefender {
                temps_reel: depuis(m.real_time_protection_enabled),
                anti_alteration: depuis(m.is_tamper_protected),
                surveillance_comportementale: depuis(m.behavior_monitor_enabled),
            });

        (plateforme, defender)
    }

    /// Lit `Win32_Service` sur un fil dédié — voir [`super::sur_un_fil_dedie`],
    /// qui porte le raisonnement sur COM, sur le délai, et sur ce que le repli
    /// ne couvre pas.
    pub(super) fn lire_services() -> Lecture<super::ServicesObserves> {
        super::sur_un_fil_dedie(interroger_services, || Lecture::Refusee)
    }

    fn interroger_services() -> Lecture<super::ServicesObserves> {
        use wmi::{COMLibrary, WMIConnection};

        // Un échec ici est un refus de lecture, jamais un constat : c'est la
        // doctrine du module, et elle vaut d'autant plus ici qu'un service
        // faussement rapporté à l'arrêt est un écart de sécurité inventé.
        let Ok(com) = COMLibrary::new() else {
            return Lecture::Refusee;
        };
        // `Win32_Service` vit dans `root\CIMV2`, l'espace de noms par défaut —
        // le même que les correctifs de `crate::attribution`.
        let Ok(connexion) = WMIConnection::new(com) else {
            return Lecture::Refusee;
        };
        let Ok(lignes) = connexion.query::<Service>() else {
            return Lecture::Refusee;
        };

        // Une ligne sans nom est **écartée** : elle ne peut être rapprochée
        // d'aucun service surveillé, donc la garder ne dirait rien de plus. Son
        // état, lui, est conservé tel quel jusqu'à la fonction pure, `None`
        // compris — voir [`super::ServicesObserves`].
        Lecture::Trouvee(
            lignes
                .into_iter()
                .filter_map(|l| Some((l.name?.to_lowercase(), l.state)))
                .collect(),
        )
    }

    fn refus_plateforme() -> EtatPlateforme {
        EtatPlateforme {
            vbs: Lecture::Refusee,
            services_actifs: Lecture::Refusee,
            services_configures: Lecture::Refusee,
            integrite_code: Lecture::Refusee,
            proprietes_disponibles: Lecture::Refusee,
        }
    }

    fn refus_defender() -> EtatDefender {
        EtatDefender {
            temps_reel: Lecture::Refusee,
            anti_alteration: Lecture::Refusee,
            surveillance_comportementale: Lecture::Refusee,
        }
    }
}

/// Le matériel sait-il faire cette chose ?
///
/// Réponse **textuelle**, pas booléenne. Un booléen s'afficherait « activé », ce
/// qui serait faux : la question est « ce matériel en est-il capable », jamais
/// « est-ce en service ». Sur une machine où la protection DMA est disponible
/// mais non activée, « activé » serait un faux positif de sécurité.
///
/// La valeur publiée est un **jeton** ([`crate::jetons::ProprieteMaterielle`]) :
/// la phrase française vit dans `ks_cli::lisible`, avec la conversion d'octets
/// et pour la même raison (ADR-0015).
#[must_use]
pub fn propriete_disponible(etat: &EtatPlateforme, code: u32) -> ItemValue {
    match &etat.proprietes_disponibles {
        Lecture::Trouvee(codes) => ItemValue::Text(
            crate::jetons::ProprieteMaterielle::depuis_presence(codes.contains(&code)).jeton(),
        ),
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible("WMI n'a pas répondu"),
    }
}

/// Ce service s'exécute-t-il **réellement** ?
///
/// Réponse **textuelle**, pas booléenne, et pour la raison qui gouverne déjà
/// [`crate::posture::service_vbs_present`] : un booléen s'affiche « activé », ce
/// qui est le vocabulaire d'un interrupteur, donc celui de la configuration. La
/// question posée ici est « est-ce que ça tourne ? ».
///
/// # Les quatre réponses, et celle qu'il ne faut jamais confondre
///
/// | Cas | Valeur | Sens |
/// |---|---|---|
/// | la ligne existe, l'état vaut `Running` | `en-execution` | le service tourne |
/// | la ligne existe, l'état vaut autre chose | `arrete` | on a regardé, il ne tourne pas |
/// | **aucune ligne à ce nom** | [`ItemValue::Absent`] | le service n'est pas installé ici |
/// | rien n'a pu être lu | [`ItemValue::Illisible`] | on n'a pas regardé |
///
/// La troisième ligne est celle qui compte. `Sense` n'existe que là où Defender
/// for Endpoint est déployé : publier « à l'arrêt » sur une machine qui ne le
/// porte pas fabriquerait un écart de sécurité de toutes pièces, sur un item
/// que le §6 du modèle de menace place au premier rang. Une absence n'est ni
/// vraie ni fausse, et c'est exactement ce qu'il faut en dire.
///
/// # La limite, nommée plutôt que découverte
///
/// `Win32_Service.State` connaît des états transitoires — `Start Pending`,
/// `Stop Pending`, `Paused`. Ils tombent tous dans `arrete`, ce qui est vrai au
/// sens de la question (le service ne tourne pas) mais grossier au sens de
/// l'état. Le vocabulaire fermé de [`crate::jetons`] n'a pas de jeton pour eux,
/// et lui en ajouter un pour une fenêtre de quelques secondes au démarrage
/// serait payer un vocabulaire permanent pour un état fugace. Le sens de
/// l'erreur est le bon : on ne déclare jamais protégé ce qui ne l'est pas.
#[must_use]
pub fn execution_service(services: &Lecture<ServicesObserves>, nom: &str) -> ItemValue {
    match services {
        Lecture::Trouvee(observes) => match observes.get(&nom.to_lowercase()) {
            Some(Some(etat)) => ItemValue::Text(
                crate::jetons::ExecutionService::depuis_execution(
                    etat.eq_ignore_ascii_case(ETAT_EN_EXECUTION),
                )
                .jeton(),
            ),
            // La ligne existe, son état non. Ce n'est pas « à l'arrêt » : rien
            // ne l'atteste, et un arrêt inventé est un écart inventé.
            Some(None) => ItemValue::illisible("Win32_Service n'a pas rapporté l'état du service"),
            // Aucune ligne à ce nom : le service n'est pas installé ici.
            None => ItemValue::Absent,
        },
        // `Absente` comme `Refusee` : on n'a pas lu. Ni un arrêt, ni une
        // absence — le dépôt a déjà payé cette confusion sur les exclusions
        // Defender, où « 0 élément » s'affichait sur une clé jamais ouverte.
        Lecture::Absente | Lecture::Refusee => ItemValue::illisible("WMI n'a pas répondu"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_lecture_par_defaut_est_une_absence_pas_un_zero() {
        // La règle du module voisin, appliquée ici : un champ que WMI n'a pas
        // renvoyé n'est pas « éteint ». `0` voudrait dire « éteint », ce qui
        // serait un faux négatif de sécurité sur les items les mieux notés du
        // modèle de menace.
        let vide = EtatPlateforme::default();
        assert_eq!(vide.vbs, Lecture::Absente);
        assert_ne!(vide.vbs, Lecture::Trouvee(0));
        assert_eq!(EtatDefender::default().temps_reel, Lecture::Absente);
    }

    #[test]
    fn une_propriete_materielle_absente_dune_liste_lue_est_un_constat() {
        // Comme pour les services VBS : la liste a été lue, donc l'absence du
        // code est une information. Un refus, lui, n'en est jamais une.
        let materiel = EtatPlateforme {
            proprietes_disponibles: Lecture::Trouvee(vec![1, 2, 3]),
            ..EtatPlateforme::default()
        };
        let presente = propriete_disponible(&materiel, propriete_materielle::PROTECTION_DMA);
        assert_eq!(
            presente,
            ItemValue::Text("disponible-sur-ce-materiel".into())
        );
        assert_eq!(
            propriete_disponible(&materiel, 99),
            ItemValue::Text("absente-de-ce-materiel".into())
        );
        // Surtout pas le vocabulaire de l'interrupteur : disponible ne veut pas
        // dire en service, et la confusion serait un faux positif de sécurité.
        assert!(!presente.to_string().contains("activ"));

        let refuse = EtatPlateforme {
            proprietes_disponibles: Lecture::Refusee,
            ..EtatPlateforme::default()
        };
        assert!(!propriete_disponible(&refuse, 3).est_constat());
    }

    /// Les services relevés le 2026-08-17, réduits à ce que les tests éprouvent.
    ///
    /// La casse est celle que `Win32_Service` rapporte réellement — `mpssvc` en
    /// minuscules là où `SERVICES_SURVEILLES` écrit `MpsSvc`. C'est ce
    /// désaccord, mesuré, qui rend le rapprochement insensible à la casse
    /// nécessaire ; l'écrire ici plutôt qu'en minuscules partout est ce qui
    /// permet au test de le prouver.
    fn services_de_reference() -> Lecture<ServicesObserves> {
        Lecture::Trouvee(
            [
                ("windefend", Some("Running")),
                ("mpssvc", Some("Running")),
                ("eventlog", Some("Running")),
                ("wscsvc", Some("Running")),
                ("bits", Some("Running")),
                // Neuf services de la machine de référence sont ainsi : réglés
                // sur « automatique », et pourtant à l'arrêt.
                ("w32time", Some("Stopped")),
                // Une ligne sans état. Elle existe, on ne sait rien d'elle.
                ("sansetat", None),
                // Et `Sense` est volontairement ABSENT de cette table : il n'est
                // installé qu'avec Defender for Endpoint.
            ]
            .into_iter()
            .map(|(nom, etat)| ((*nom).to_owned(), etat.map(str::to_owned)))
            .collect(),
        )
    }

    #[test]
    fn un_service_absent_de_la_machine_ne_se_publie_jamais_comme_arrete() {
        // **La barrière du lot.** `Sense` n'existe que là où Defender for
        // Endpoint est déployé. Publier « à l'arrêt » sur une machine qui ne le
        // porte pas fabriquerait un écart de sécurité de toutes pièces, sur un
        // item que le §6 du modèle de menace place au premier rang.
        //
        // Une absence n'est ni vraie ni fausse : c'est ce que dit `Absent`, et
        // c'est la seule chose honnête à en dire.
        let lus = services_de_reference();
        let sense = execution_service(&lus, "Sense");

        assert_eq!(sense, ItemValue::Absent);
        assert_ne!(
            sense,
            ItemValue::Text("arrete".into()),
            "un service non installé n'est pas un service arrêté"
        );
        assert_ne!(sense, ItemValue::Bool(false));

        // Ce qui a été lu, lui, est un constat dans les deux sens.
        assert_eq!(
            execution_service(&lus, "WinDefend"),
            ItemValue::Text("en-execution".into())
        );
        assert_eq!(
            execution_service(&lus, "W32Time"),
            ItemValue::Text("arrete".into()),
            "la ligne a été lue : « Stopped » est un constat, pas une lacune"
        );

        // Jamais le vocabulaire de l'interrupteur : « activé » redirait la
        // confusion entre configuration et exécution que cet item lève.
        assert!(!execution_service(&lus, "WinDefend")
            .to_string()
            .contains("activ"));

        // La casse ne décide de rien. `SERVICES_SURVEILLES` écrit « MpsSvc »,
        // `Win32_Service` rapporte « mpssvc » : une comparaison littérale
        // publierait « absent » sur un pare-feu qui tourne.
        for ecriture in ["MpsSvc", "mpssvc", "MPSSVC"] {
            assert_eq!(
                execution_service(&lus, ecriture),
                ItemValue::Text("en-execution".into()),
                "« {ecriture} » ne se rapproche pas de la ligne relevée"
            );
        }

        // Un état transitoire ne passe jamais pour une exécution — le sens de
        // l'erreur est le bon, et c'est la limite nommée dans la documentation.
        let transitoire = Lecture::Trouvee(
            [("windefend".to_owned(), Some("Start Pending".to_owned()))]
                .into_iter()
                .collect(),
        );
        assert_eq!(
            execution_service(&transitoire, "WinDefend"),
            ItemValue::Text("arrete".into())
        );
    }

    #[test]
    fn une_lecture_de_services_refusee_ne_produit_ni_arret_ni_absence() {
        // Le défaut que ce dépôt a déjà payé, transposé : « 0 exclusion »
        // s'affichait sur une clé jamais ouverte. Ici, « à l'arrêt »
        // s'afficherait sur un service jamais regardé — donc un écart inventé —
        // et « absent » prétendrait qu'il n'est pas installé.
        for muet in [
            Lecture::<ServicesObserves>::Refusee,
            Lecture::<ServicesObserves>::Absente,
        ] {
            let valeur = execution_service(&muet, "WinDefend");
            assert_ne!(valeur, ItemValue::Text("arrete".into()));
            assert_ne!(valeur, ItemValue::Text("en-execution".into()));
            assert_ne!(valeur, ItemValue::Absent);
            assert!(
                !valeur.est_constat(),
                "un refus ne doit jamais alimenter un décompte"
            );
        }

        // Et une ligne présente dont l'état manque est illisible elle aussi :
        // la ligne existe, donc « absent » serait faux, et rien n'atteste un
        // arrêt.
        let sans_etat = execution_service(&services_de_reference(), "SansEtat");
        assert!(!sans_etat.est_constat());
        assert_ne!(sans_etat, ItemValue::Absent);
    }

    #[test]
    fn la_lecture_ne_fait_jamais_tomber_lappelant() {
        // Sur toute plateforme, y compris sans WMI : la fonction rend la main.
        // Un collecteur qui paniquerait priverait l'utilisateur des cent autres
        // items pour une classe indisponible.
        let (plateforme, defender) = lire();
        // On n'assert pas de valeur : elle dépend de la machine. On assert que
        // l'appel revient, et que rien n'a été inventé.
        assert!(matches!(
            plateforme.vbs,
            Lecture::Trouvee(_) | Lecture::Absente | Lecture::Refusee
        ));
        assert!(matches!(
            defender.temps_reel,
            Lecture::Trouvee(_) | Lecture::Absente | Lecture::Refusee
        ));

        // La lecture des services obéit à la même règle, et sur un second fil
        // dédié : elle rend la main, quoi qu'ait répondu WMI.
        assert!(matches!(
            lire_services(),
            Lecture::Trouvee(_) | Lecture::Absente | Lecture::Refusee
        ));
    }
}

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
//! Deux classes, mesurées **lisibles en session non élevée** sur une machine
//! réelle — c'est-à-dire dans les conditions où la CLI s'exécute (SEC-01) :
//! `Win32_DeviceGuard` et `MSFT_MpComputerStatus`.
//!
//! `Win32_Tpm` et `Win32_EncryptableVolume` renvoient « accès refusé ». Ils ne
//! butent pas sur une API manquante mais sur une liste de contrôle d'accès :
//! aucun choix de bibliothèque ne les rendrait lisibles ici. Ils appartiennent
//! au broker, donc à la Phase 2.
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

/// Les codes de `AvailableSecurityProperties` dont on se sert.
pub mod propriete_materielle {
    /// La protection DMA est **disponible** — ce qui ne dit pas qu'elle est active.
    pub const PROTECTION_DMA: u32 = 3;
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

    /// Une valeur présente est un constat ; une valeur absente n'en est pas un.
    fn depuis<T>(valeur: Option<T>) -> Lecture<T> {
        valeur.map_or(Lecture::Absente, Lecture::Trouvee)
    }

    /// Interroge WMI sur un fil dédié, puis laisse ce fil mourir.
    ///
    /// L'isolement n'est pas cosmétique. `COMLibrary::new()` initialise COM en
    /// appartement multifilière **sur le fil appelant**, et la bibliothèque
    /// documente qu'elle n'appelle jamais `CoUninitialize` au `Drop` — pour ne
    /// pas invalider des pointeurs COM encore vivants ailleurs. L'initialisation
    /// survit donc à la requête. En la confinant à un fil qu'on abandonne
    /// ensuite, le processus hôte ressort exactement comme il est entré.
    ///
    /// Si le fil panique, `join` renvoie une erreur et l'on retombe sur des
    /// lectures absentes : un collecteur ne fait jamais tomber son appelant.
    pub(super) fn lire() -> (EtatPlateforme, EtatDefender) {
        std::thread::spawn(interroger)
            .join()
            .unwrap_or_else(|_| (EtatPlateforme::default(), EtatDefender::default()))
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

        // Une seconde connexion : les deux classes vivent dans des espaces de
        // noms distincts, et une connexion WMI en vise un seul.
        let com_defender = match COMLibrary::new() {
            Ok(c) => c,
            Err(_) => return (plateforme, refus_defender()),
        };

        let defender =
            WMIConnection::with_namespace_path(r"root\Microsoft\Windows\Defender", com_defender)
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
#[must_use]
pub fn propriete_disponible(etat: &EtatPlateforme, code: u32) -> ItemValue {
    match &etat.proprietes_disponibles {
        Lecture::Trouvee(codes) => ItemValue::Text(
            if codes.contains(&code) {
                "disponible sur ce matériel"
            } else {
                "absente de ce matériel"
            }
            .to_owned(),
        ),
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible("WMI n'a pas répondu"),
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
            ItemValue::Text("disponible sur ce matériel".into())
        );
        assert_eq!(
            propriete_disponible(&materiel, 99),
            ItemValue::Text("absente de ce matériel".into())
        );
        // Surtout pas « activé » : disponible ne veut pas dire en service, et la
        // confusion serait un faux positif de sécurité.
        assert!(!presente.to_string().contains("activé"));

        let refuse = EtatPlateforme {
            proprietes_disponibles: Lecture::Refusee,
            ..EtatPlateforme::default()
        };
        assert!(!propriete_disponible(&refuse, 3).est_constat());
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
    }
}

//! Distributions WSL2 et machines virtuelles, en lecture (D9, Phase 0.4).
//!
//! ## Ce que ce collecteur voit
//!
//! WSL déclare ses distributions dans le registre de l'utilisateur, sous `Lxss`.
//! Tout s'y lit : le nom, la version du sous-système, l'état, le dossier de base
//! et les drapeaux d'intégration. Aucun processus n'est lancé — `wsl --list` coûte
//! des secondes et démarre la machine virtuelle utilitaire, ce qui serait un effet
//! de bord, donc interdit à un collecteur.
//!
//! ## La taille du disque, et pourquoi elle vaut d'être remontée
//!
//! Un disque WSL2 est un `ext4.vhdx` à allocation dynamique : il **grossit et ne
//! se réduit jamais tout seul**. Supprimer 30 Go dans la distribution ne rend pas
//! un octet à l'hôte. Sur la machine de référence, ce fichier pèse 56 Go, et c'est
//! typiquement le premier poste d'occupation d'un poste de développement — invisible
//! depuis l'explorateur, puisqu'il vit dans un dossier de paquet.
//!
//! On lit donc la **taille réelle du fichier**, pas ce que la distribution croit
//! occuper. Les deux diffèrent, souvent d'un facteur deux, et c'est l'écart qui
//! porte l'information.
//!
//! ## Ce qui n'est pas là
//!
//! L'âge des points de contrôle Hyper-V et les chaînes de disques différentiels
//! demandent le module `Hyper-V`, donc PowerShell ou WMI. Ni l'un ni l'autre n'est
//! une lecture de registre, et les deux attendent l'arbitrage sur les API natives.

use ks_core::{Domain, Item, ItemValue, Provenance};

/// Une distribution WSL déclarée sur ce poste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distribution {
    /// Nom tel que l'utilisateur le connaît.
    pub nom: String,
    /// 1 ou 2 — la génération du sous-système.
    pub version: Option<u32>,
    /// Taille réelle du disque virtuel, en octets.
    pub disque_octets: Option<u64>,
    /// L'intégration avec Windows est-elle activée ?
    pub interop: Option<bool>,
    /// Les lecteurs Windows sont-ils montés dans la distribution ?
    pub montage_lecteurs: Option<bool>,
}

/// Drapeaux d'intégration documentés par Microsoft (`WSL_DISTRIBUTION_FLAGS`).
///
/// Seuls ces deux-là sont interprétés. Le bit 3, observé à 1 sur cette machine,
/// n'est documenté nulle part : le traduire serait inventer.
#[cfg(windows)]
const DRAPEAU_INTEROP: u32 = 0x1;
#[cfg(windows)]
const DRAPEAU_MONTAGE_LECTEURS: u32 = 0x4;

impl Distribution {
    /// Le disque a-t-il dépassé un seuil qui mérite qu'on en parle ?
    ///
    /// Le seuil est **assumé et arbitraire** : 20 Go. Il ne dit pas qu'il y a un
    /// écart, seulement que le fichier pèse assez pour qu'un compactage rende
    /// quelque chose. Le brief impose le chiffre avant l'adjectif, donc c'est la
    /// taille qui s'affiche, jamais « volumineux ».
    #[must_use]
    pub fn disque_notable(&self) -> bool {
        self.disque_octets
            .is_some_and(|o| o > 20 * 1024 * 1024 * 1024)
    }
}

/// Collecteur de virtualisation.
pub struct VirtualisationCollector;

impl VirtualisationCollector {
    /// Identifiant, pour le journal.
    #[must_use]
    pub const fn nom() -> &'static str {
        "virtualisation"
    }

    /// Les distributions WSL déclarées.
    #[must_use]
    pub fn distributions() -> Vec<Distribution> {
        #[cfg(windows)]
        {
            windows_impl::distributions()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// Les items du domaine D9.
    #[must_use]
    pub fn items() -> Vec<Item> {
        let distros = Self::distributions();
        let maintenant = chrono::Utc::now();
        let mut items = Vec::new();

        let mut item = |chemin: String, valeur: ItemValue, but: &str, risque: &str| {
            items.push(Item {
                path: chemin,
                domain: Domain::Virtualization,
                desired: None,
                observed: valeur,
                observed_at: maintenant,
                provenance: Provenance::Observed,
                purpose: but.to_owned(),
                risk: risque.to_owned(),
                reference: None,
            });
        };

        item(
            "virtualization.wsl.distro_count".to_owned(),
            ItemValue::Int(i64::try_from(distros.len()).unwrap_or(-1)),
            "Nombre de distributions WSL déclarées sur ce poste.",
            "Aucun — cet item est un constat.",
        );

        for d in &distros {
            let clef = crate::software::Application::clef(&d.nom);

            item(
                format!("virtualization.wsl[{clef}].version"),
                d.version
                    .map_or(ItemValue::Absent, |v| ItemValue::Int(i64::from(v))),
                "Génération du sous-système. Une distribution restée en version 1 \
                 n'a ni noyau réel ni disque virtuel.",
                "Aucun — cet item est un constat.",
            );

            item(
                format!("virtualization.wsl[{clef}].disk_bytes"),
                d.disque_octets
                    .and_then(|o| i64::try_from(o).ok())
                    .map_or(ItemValue::Absent, ItemValue::Int),
                "Taille RÉELLE du disque virtuel sur l'hôte, et non ce que la \
                 distribution croit occuper. Un « ext4.vhdx » grossit et ne se \
                 réduit jamais seul : supprimer des données à l'intérieur ne rend \
                 aucun octet à Windows.",
                "Un disque qui grossit sans retour est la première cause de \
                 saturation silencieuse d'un poste de développement.",
            );

            item(
                format!("virtualization.wsl[{clef}].interop"),
                d.interop.map_or(ItemValue::Absent, ItemValue::Bool),
                "Exécution de binaires Windows depuis la distribution.",
                "L'intégration élargit la surface entre les deux systèmes : un \
                 script Linux peut lancer un exécutable Windows.",
            );

            item(
                format!("virtualization.wsl[{clef}].drive_mounting"),
                d.montage_lecteurs
                    .map_or(ItemValue::Absent, ItemValue::Bool),
                "Montage automatique des lecteurs Windows dans la distribution.",
                "Le disque de l'hôte devient accessible en écriture depuis la \
                 distribution, avec les droits de l'utilisateur.",
            );
        }

        items
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{Distribution, DRAPEAU_INTEROP, DRAPEAU_MONTAGE_LECTEURS};
    use std::path::{Path, PathBuf};
    use windows_registry::CURRENT_USER;

    /// Là où WSL déclare ses distributions.
    const LXSS: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";

    /// Taille réelle du disque d'une distribution, si on la trouve.
    ///
    /// Le chemin déclaré peut porter le préfixe `\\?\`, que Windows accepte mais
    /// qui gêne la comparaison ; on le retire avant de composer. Le fichier n'est
    /// jamais ouvert — seules ses métadonnées sont lues.
    fn taille_disque(base: &str) -> Option<u64> {
        let nettoye = base.strip_prefix(r"\\?\").unwrap_or(base);
        let dossier = PathBuf::from(nettoye);
        for nom in ["ext4.vhdx", "docker_data.vhdx"] {
            let candidat: &Path = &dossier.join(nom);
            if let Ok(meta) = std::fs::metadata(candidat) {
                return Some(meta.len());
            }
        }
        None
    }

    pub(super) fn distributions() -> Vec<Distribution> {
        let Ok(racine) = CURRENT_USER.open(LXSS) else {
            return Vec::new();
        };
        let Ok(sous_clefs) = racine.keys() else {
            return Vec::new();
        };

        let mut trouvees = Vec::new();
        for identifiant in sous_clefs {
            let Ok(clef) = racine.open(&identifiant) else {
                continue;
            };
            // Sans nom, l'entrée n'est montrable à personne (principe P6).
            let Ok(nom) = clef.get_string("DistributionName") else {
                continue;
            };

            let drapeaux = clef.get_u32("Flags").ok();
            trouvees.push(Distribution {
                nom,
                version: clef.get_u32("Version").ok(),
                disque_octets: clef
                    .get_string("BasePath")
                    .ok()
                    .and_then(|b| taille_disque(&b)),
                interop: drapeaux.map(|f| f & DRAPEAU_INTEROP != 0),
                montage_lecteurs: drapeaux.map(|f| f & DRAPEAU_MONTAGE_LECTEURS != 0),
            });
        }
        trouvees
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distro(nom: &str, octets: Option<u64>) -> Distribution {
        Distribution {
            nom: nom.to_owned(),
            version: Some(2),
            disque_octets: octets,
            interop: Some(true),
            montage_lecteurs: Some(true),
        }
    }

    #[test]
    fn une_taille_inconnue_ne_declenche_rien() {
        // Un disque qu'on n'a pas su mesurer n'est pas un petit disque. La
        // confusion produirait exactement le faux négatif que le collecteur de
        // posture combat par ailleurs.
        assert!(!distro("Debian", None).disque_notable());
    }

    #[test]
    fn le_seuil_de_taille_est_franc() {
        const GO: u64 = 1024 * 1024 * 1024;
        assert!(
            !distro("Alpine", Some(20 * GO)).disque_notable(),
            "au seuil"
        );
        assert!(distro("Debian", Some(20 * GO + 1)).disque_notable());
        // La valeur relevée sur la machine de référence : 56 Go.
        assert!(distro("Debian", Some(56_043_241_472)).disque_notable());
    }

    #[test]
    fn tout_item_de_virtualisation_porte_son_explication() {
        for item in VirtualisationCollector::items() {
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert!(!item.risk.is_empty(), "« {} » sans risque", item.path);
            assert_eq!(item.provenance, Provenance::Observed);
            assert!(item.desired.is_none(), "un collecteur ne décide de rien");
            assert_eq!(item.domain, Domain::Virtualization);
        }
    }

    #[test]
    fn le_decompte_existe_meme_sans_distribution() {
        // Une absence de mesure se dit, elle ne se tait pas : sur un poste sans
        // WSL, l'item vaut zéro et l'utilisateur sait qu'on a regardé.
        let chemins: Vec<String> = VirtualisationCollector::items()
            .into_iter()
            .map(|i| i.path)
            .collect();
        assert!(chemins
            .iter()
            .any(|c| c == "virtualization.wsl.distro_count"));
    }
}

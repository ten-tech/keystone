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
//!
//! ## La règle que ce module partage avec la posture
//!
//! **Un refus de lecture n'est pas une liste vide.** Le premier jet renvoyait
//! `Vec::new()` sur tout échec d'ouverture de `Lxss` ou d'énumération, et
//! `virtualization.wsl.distro_count` publiait alors `0` en l'annonçant comme un
//! constat. Zéro y recouvrait deux réponses opposées : « aucune distribution » et
//! « je n'ai pas pu regarder ». C'est le défaut corrigé dans [`crate::posture`],
//! réintroduit ici mot pour mot — d'où l'emploi du même type [`Lecture`], et du
//! même classement, plutôt qu'une seconde version de la règle.

use ks_core::{Domain, Item, ItemValue, Provenance};

use crate::posture::Lecture;

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

    /// Les distributions WSL déclarées, ou l'aveu qu'on n'a pas su les lire.
    ///
    /// **Trois états, jamais deux.** Un `Vec` vide ne saurait pas dire lequel des
    /// deux il décrit, et c'est précisément la confusion qui rendait
    /// `distro_count = 0` mensonger sur une machine dont la clé est protégée.
    #[must_use]
    pub fn distributions() -> Lecture<Vec<Distribution>> {
        #[cfg(windows)]
        {
            windows_impl::distributions()
        }
        #[cfg(not(windows))]
        {
            // WSL n'existe pas hors de Windows : la clé n'est pas refusée, elle
            // n'a aucune raison d'exister. C'est une absence, pas un aveu — et
            // surtout pas un zéro, qui prétendrait qu'on a compté.
            Lecture::Absente
        }
    }

    /// Les items du domaine D9.
    #[must_use]
    pub fn items() -> Vec<Item> {
        items_depuis(&Self::distributions())
    }
}

/// Ce qu'on dit à l'utilisateur quand la clé des distributions lui est refusée.
///
/// Pas d'élévation dans le message : `Lxss` vit sous la ruche de l'utilisateur,
/// et un refus y vient d'une liste de contrôle d'accès posée sur la clé, pas du
/// niveau d'intégrité du processus. Le brief interdit le code d'erreur nu ; il
/// interdit tout autant l'explication fausse.
const REFUS: &str = "accès refusé au registre des distributions WSL";

/// Traduit une lecture en items. **Pure** : c'est ce qui la rend éprouvable.
///
/// Séparée de [`VirtualisationCollector::items`] pour que les deux cas qui
/// comptent — l'absence réelle et le refus — s'éprouvent sur toute plateforme,
/// sans dépendre d'une machine où WSL serait installé ou d'une clé qu'il
/// faudrait protéger à la main.
fn items_depuis(lecture: &Lecture<Vec<Distribution>>) -> Vec<Item> {
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
        match lecture {
            Lecture::Trouvee(d) => ItemValue::Int(i64::try_from(d.len()).unwrap_or(-1)),
            Lecture::Absente => ItemValue::Absent,
            Lecture::Refusee => ItemValue::illisible(REFUS),
        },
        "Nombre de distributions WSL déclarées sur ce poste. Zéro veut dire \
         « aucune », et rien d'autre : une clé qu'on n'a pas pu lire se dit \
         « illisible », une clé qui n'existe pas se dit « absent ».",
        "Aucun — cet item est un constat.",
    );

    // Un refus ou une absence ne fabrique aucun item par distribution : il n'y
    // a rien à décrire, et inventer une liste vide serait le même mensonge d'un
    // cran plus bas.
    if let Lecture::Trouvee(distros) = lecture {
        for d in distros {
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
    }

    items
}

#[cfg(windows)]
mod windows_impl {
    use super::{Distribution, Lecture, DRAPEAU_INTEROP, DRAPEAU_MONTAGE_LECTEURS};
    use crate::posture::{classer, ACCES_REFUSE};
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

    pub(super) fn distributions() -> Lecture<Vec<Distribution>> {
        // Le classement vient de `posture` : un refus d'accès et une clé absente
        // sont deux réponses, et le premier jet les avalait toutes deux dans un
        // `Vec::new()`.
        let racine = match classer(CURRENT_USER.open(LXSS)) {
            Lecture::Trouvee(k) => k,
            Lecture::Absente => return Lecture::Absente,
            Lecture::Refusee => return Lecture::Refusee,
        };
        let sous_clefs = match classer(racine.keys()) {
            Lecture::Trouvee(s) => s,
            Lecture::Absente => return Lecture::Absente,
            Lecture::Refusee => return Lecture::Refusee,
        };

        let mut trouvees = Vec::new();
        for identifiant in sous_clefs {
            let clef = match racine.open(&identifiant) {
                Ok(c) => c,
                // Une seule entrée protégée par une liste de contrôle d'accès
                // suffit à fausser le décompte. On refuse l'ensemble plutôt que
                // de publier un nombre dont on sait qu'il manque quelqu'un —
                // c'est exactement le cas qu'un attaquant fabriquerait.
                Err(e) if e.code().0 == ACCES_REFUSE => return Lecture::Refusee,
                // Sous-clé disparue entre l'énumération et l'ouverture : une
                // course bénigne, et non un aveu.
                Err(_) => continue,
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
        Lecture::Trouvee(trouvees)
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

    /// L'item de décompte produit par une lecture donnée.
    fn decompte(lecture: &Lecture<Vec<Distribution>>) -> Item {
        items_depuis(lecture)
            .into_iter()
            .find(|i| i.path == "virtualization.wsl.distro_count")
            .expect("le décompte se dit toujours, quelle que soit la lecture")
    }

    #[test]
    fn une_absence_de_distribution_se_compte_et_reste_un_constat() {
        // Le cas nominal d'un poste où WSL est installé sans distribution : la
        // clé s'ouvre, elle n'a aucune sous-clé. Zéro est alors une MESURE, et
        // l'utilisateur sait qu'on a regardé. C'est la moitié de la nuance qu'il
        // ne faut pas perdre en réparant l'autre.
        let item = decompte(&Lecture::Trouvee(Vec::new()));

        assert_eq!(item.observed, ItemValue::Int(0));
        assert!(
            item.observed.est_constat(),
            "un décompte réel alimente les indicateurs"
        );
        assert!(item.verdict().est_concluant());

        // Une distribution lue produit ses quatre items, et le décompte suit.
        let peuple = Lecture::Trouvee(vec![distro("Debian", Some(56_043_241_472))]);
        assert_eq!(decompte(&peuple).observed, ItemValue::Int(1));
        assert_eq!(
            items_depuis(&peuple).len(),
            5,
            "le décompte plus quatre items"
        );
    }

    #[test]
    fn un_refus_de_lecture_ne_se_publie_jamais_en_zero_distribution() {
        // **Le défaut, et le test qui le verrouillait.** Tout échec d'ouverture
        // de `Lxss` ou d'énumération renvoyait `Vec::new()` :
        // `virtualization.wsl.distro_count` publiait `0` en l'annonçant « un
        // constat », alors que zéro y recouvrait « aucune distribution » ET « je
        // n'ai pas pu lire ». C'est le défaut réparé dans `posture.rs`,
        // réintroduit ici — et l'ancien test, qui se contentait d'exiger la
        // présence du chemin, le tenait en place.
        let item = decompte(&Lecture::Refusee);

        assert!(
            !item.observed.est_constat(),
            "un aveu ne doit jamais alimenter un décompte"
        );
        assert_ne!(item.observed, ItemValue::Int(0), "zéro serait un mensonge");
        assert_ne!(item.observed, ItemValue::Absent, "ni une absence");
        assert!(
            item.observed.to_string().contains("refusé"),
            "la raison doit remonter à l'utilisateur : {}",
            item.observed
        );

        // Et rien n'est fabriqué par-dessus : une liste de distributions
        // inventée à partir d'un refus serait le même mensonge, un cran plus bas.
        assert_eq!(items_depuis(&Lecture::Refusee).len(), 1);

        // Une clé qui n'existe pas est un troisième cas, distinct des deux
        // autres : WSL n'est pas installé. Ce n'est ni un aveu, ni un zéro.
        let jamais_installe = decompte(&Lecture::Absente);
        assert_eq!(jamais_installe.observed, ItemValue::Absent);
        assert_ne!(jamais_installe.observed, ItemValue::Int(0));
    }

    #[test]
    fn le_decompte_se_dit_quelle_que_soit_la_lecture() {
        // Une absence de mesure se dit, elle ne se tait pas. Le chemin est
        // toujours produit ; ce qui change, c'est ce qu'il porte.
        for lecture in [
            Lecture::Trouvee(Vec::new()),
            Lecture::Trouvee(vec![distro("Debian", None)]),
            Lecture::Absente,
            Lecture::Refusee,
        ] {
            let item = decompte(&lecture);
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert_eq!(item.provenance, Provenance::Observed);
        }
    }
}

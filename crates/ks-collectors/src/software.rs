//! Réconciliation d'inventaire logiciel — le livrable le plus sous-estimé (D1, 0.3).
//!
//! ## Ce que ça produit, et pourquoi ça vaut le détour
//!
//! Tout le monde sait lister les applications installées. L'intérêt est ailleurs :
//! **croiser cette liste avec celle de chaque gestionnaire de paquets**, pour
//! produire les applications que *personne* ne met à jour.
//!
//! Sur un poste d'ingénieur, ces orphelines représentent typiquement 30 % du parc
//! installé, et c'est exactement là que dorment les versions vulnérables depuis
//! quatorze mois : rien ne les surveille, donc rien ne les signale.
//!
//! ## Ce que ce collecteur mesure AUJOURD'HUI, et ce qu'il ne mesure pas
//!
//! Il produit les applications **dont le gestionnaire n'a pas été identifié** —
//! ce qui n'est pas la même chose que « gérées par aucun gestionnaire ». La
//! première est une affirmation sur ce que ce code sait faire ; la seconde, une
//! affirmation sur le monde. Tant que winget n'est pas interrogeable, seule la
//! première est vraie, et le pourcentage n'est pas publié du tout.
//!
//! ## Lecture seule, sans exception
//!
//! Aucune écriture, aucun processus lancé, aucun effet de bord. On lit trois vues
//! du registre et quelques répertoires de gestionnaires. Ni `winget list` ni
//! `Get-AppxPackage` ne sont invoqués : lancer un processus pour observer coûte
//! des secondes, et un collecteur ne doit pas peser sur la machine (NF-01).
//!
//! ## Ce qui n'est pas encore fait, et pourquoi
//!
//! * **winget** — l'attribution fine exige de lire sa base `StoreEdgeFD`, dont le
//!   format n'est pas contractuel. On détecte sa *présence*, pas encore quelle
//!   application il gère.
//! * **MSIX / Store** — lisible via le dépôt `AppModel` du registre, mais mêlé à
//!   des centaines de paquets système ; le tri demande une liste de référence.
//! * **Dernier lancement (SRUM)** — base ESE, analyse lourde. Prévu en 0.3 bis.
//!
//! Ces trois manques sont *déclarés*, pas silencieux, et le modèle les porte :
//! `inventory.software.attribution` vaut « partielle » tant qu'un gestionnaire
//! détecté ne sait pas être interrogé, et `unqueryable_managers` le nomme. Le
//! décompte des non attribuées est alors un **majorant**, pas une mesure.
//!
//! C'est le seul traitement honnête : sur une machine où winget est présent, tout
//! afficher comme orphelin donnerait 100 %, et un indicateur qu'on ne peut pas
//! expliquer est une décoration (principe P6).

use chrono::Utc;
use ks_core::{Domain, Item, ItemValue, Provenance};

/// Qui met à jour cette application, si quelqu'un le fait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gestionnaire {
    /// Le gestionnaire de paquets de Windows.
    Winget,
    /// Scoop, installé par utilisateur.
    Scoop,
    /// Chocolatey, à l'échelle de la machine.
    Chocolatey,
    /// L'installeur Visual Studio.
    VisualStudio,
    /// JetBrains Toolbox.
    JetBrainsToolbox,
    /// **Non attribué** — et surtout pas « aucun ».
    ///
    /// La nuance décide de la valeur du livrable. « Aucun gestionnaire ne la met à
    /// jour » est une affirmation sur le monde ; « je n'ai pas su identifier son
    /// gestionnaire » est une affirmation sur ce que ce collecteur sait faire.
    /// Tant que l'attribution winget n'existe pas, seule la seconde est vraie —
    /// et la première rassurerait à tort dans un sens comme dans l'autre.
    NonAttribue,
}

impl Gestionnaire {
    /// Libellé destiné à l'utilisateur.
    #[must_use]
    pub const fn libelle(self) -> &'static str {
        match self {
            Self::Winget => "winget",
            Self::Scoop => "Scoop",
            Self::Chocolatey => "Chocolatey",
            Self::VisualStudio => "Visual Studio Installer",
            Self::JetBrainsToolbox => "JetBrains Toolbox",
            Self::NonAttribue => "non attribué",
        }
    }

    /// Ce gestionnaire sait-il dire **quelles** applications il gère ?
    ///
    /// winget est le contre-exemple : on détecte sa présence, mais son inventaire
    /// vit dans une base SQLite (`StoreEdgeFD`) dont le format n'est pas
    /// contractuel. Le détecter sans savoir l'interroger ne permet d'attribuer
    /// aucune application — et c'est ce qui rend l'inventaire incomplet aujourd'hui.
    #[must_use]
    pub const fn sait_attribuer(self) -> bool {
        matches!(
            self,
            Self::Scoop | Self::Chocolatey | Self::VisualStudio | Self::JetBrainsToolbox
        )
    }

    /// L'application a-t-elle un gestionnaire identifié ?
    #[must_use]
    pub const fn est_attribue(self) -> bool {
        !matches!(self, Self::NonAttribue)
    }
}

/// Une application installée, telle que la machine la déclare.
#[derive(Debug, Clone)]
pub struct Application {
    /// Nom affiché.
    pub nom: String,
    /// Version affichée, quand elle est renseignée.
    pub version: Option<String>,
    /// Éditeur déclaré.
    pub editeur: Option<String>,
    /// Qui la met à jour.
    pub gestionnaire: Gestionnaire,
}

impl Application {
    /// Clé de rapprochement entre sources.
    ///
    /// Volontairement grossière : minuscules, sans ponctuation ni espaces. Deux
    /// sources n'écrivent jamais un nom de la même façon — « Git » côté registre,
    /// « git » côté Scoop, « Git for Windows » côté éditeur. Une comparaison exacte
    /// ne rapprocherait rien, et c'est le rapprochement qui fait la valeur ici.
    #[must_use]
    pub fn clef(nom: &str) -> String {
        nom.to_lowercase()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    }
}

/// Résultat de la réconciliation.
#[derive(Debug, Default)]
pub struct Inventaire {
    /// Toutes les applications trouvées, dédoublonnées.
    pub applications: Vec<Application>,
    /// Gestionnaires effectivement présents sur la machine.
    pub gestionnaires: Vec<Gestionnaire>,
}

impl Inventaire {
    /// Les applications dont le gestionnaire n'a pas pu être identifié.
    #[must_use]
    pub fn non_attribuees(&self) -> Vec<&Application> {
        self.applications
            .iter()
            .filter(|a| !a.gestionnaire.est_attribue())
            .collect()
    }

    /// Un gestionnaire est-il présent sans qu'on sache l'interroger ?
    ///
    /// Tant que c'est vrai, « non attribué » ne peut pas se lire « orphelin » :
    /// une application gérée par ce gestionnaire-là serait comptée à tort.
    #[must_use]
    pub fn attribution_incomplete(&self) -> Vec<Gestionnaire> {
        self.gestionnaires
            .iter()
            .copied()
            .filter(|g| !g.sait_attribuer())
            .collect()
    }

    /// Part des non attribuées, en pourcentage entier.
    ///
    /// `None` dans deux cas, et le second est le plus important : quand
    /// l'inventaire est vide, et **quand un gestionnaire présent ne sait pas être
    /// interrogé**. Publier « 100 % » alors qu'on n'a simplement pas su regarder
    /// serait un indicateur qu'on ne peut pas expliquer — ce que le principe P6
    /// interdit, et ce que le brief appelle une décoration.
    #[must_use]
    pub fn part_non_attribuees(&self) -> Option<u8> {
        if !self.attribution_incomplete().is_empty() {
            return None;
        }
        let total = u32::try_from(self.applications.len()).ok()?;
        if total == 0 {
            return None;
        }
        let sans = u32::try_from(self.non_attribuees().len()).ok()?;
        u8::try_from(sans.saturating_mul(100) / total).ok()
    }
}

/// Collecteur d'inventaire logiciel.
///
/// Sur une plateforme autre que Windows, il ne produit rien plutôt que d'inventer :
/// l'agent Linux a son propre gestionnaire de paquets, et prétendre le contraire
/// remplirait l'inventaire de suppositions.
pub struct SoftwareCollector;

impl SoftwareCollector {
    /// Nom du collecteur, pour le journal.
    #[must_use]
    pub const fn nom() -> &'static str {
        "inventaire logiciel"
    }

    /// Construit l'inventaire réconcilié.
    #[must_use]
    pub fn inventorier() -> Inventaire {
        #[cfg(windows)]
        {
            windows_impl::inventorier()
        }
        #[cfg(not(windows))]
        {
            Inventaire::default()
        }
    }

    /// Traduit l'inventaire en items du modèle.
    #[must_use]
    pub fn items() -> Vec<Item> {
        let inv = Self::inventorier();
        let maintenant = Utc::now();
        let mut items = Vec::new();

        let mut item = |chemin: String, valeur: ItemValue, but: &str, risque: &str| {
            items.push(Item {
                path: chemin,
                domain: Domain::Inventory,
                desired: None,
                observed: valeur,
                observed_at: maintenant,
                // `Observed` et non `Keystone` : on relève, on ne produit pas.
                provenance: Provenance::Observed,
                purpose: but.to_owned(),
                risk: risque.to_owned(),
                reference: None,
            });
        };

        item(
            "inventory.software.total".to_owned(),
            ItemValue::Int(i64::try_from(inv.applications.len()).unwrap_or(-1)),
            "Nombre d'applications installées, toutes sources confondues.",
            "Aucun — cet item est un constat.",
        );

        let sans_gestionnaire = inv.non_attribuees();
        item(
            "inventory.software.unattributed".to_owned(),
            ItemValue::Int(i64::try_from(sans_gestionnaire.len()).unwrap_or(-1)),
            "Applications dont le gestionnaire n'a pas pu être identifié. À ne pas \
             lire « orphelines » tant que l'attribution est incomplète.",
            "Aucun — cet item est un constat.",
        );

        let aveugles = inv.attribution_incomplete();
        // Texte plutôt que booléen : « activé / désactivé » est le vocabulaire d'un
        // interrupteur, pas d'une question de complétude. Un item qu'on lit de
        // travers est un item mal conçu, même si sa valeur est juste.
        item(
            "inventory.software.attribution".to_owned(),
            ItemValue::Text(
                if aveugles.is_empty() {
                    "complète"
                } else {
                    "partielle"
                }
                .to_owned(),
            ),
            "L'attribution couvre-t-elle tous les gestionnaires présents ? Tant \
             qu'elle est partielle, « non attribué » ne veut pas dire « sans \
             gestionnaire ».",
            "Aucun — mais la lire comme un feu vert transformerait une lacune de \
             mesure en fausse assurance.",
        );

        if !aveugles.is_empty() {
            item(
                "inventory.software.unqueryable_managers".to_owned(),
                ItemValue::List(aveugles.iter().map(|g| g.libelle().to_owned()).collect()),
                "Gestionnaires détectés qu'on ne sait pas encore interroger. Les \
                 applications qu'ils gèrent apparaissent à tort comme non attribuées.",
                "Aucun — mais tant qu'ils figurent ici, le décompte des non \
                 attribuées est un majorant, pas une mesure.",
            );
        }

        // Le pourcentage n'est publié que lorsqu'il veut dire quelque chose. Un
        // « 100 % » qui signifie « on n'a pas su regarder » est une décoration.
        if let Some(part) = inv.part_non_attribuees() {
            item(
                "inventory.software.unattributed_percent".to_owned(),
                ItemValue::Int(i64::from(part)),
                "Part des applications sans gestionnaire identifié, en pourcentage.",
                "Aucun — cet item est un constat.",
            );
        }

        item(
            "inventory.software.managers".to_owned(),
            ItemValue::List(
                inv.gestionnaires
                    .iter()
                    .map(|g| g.libelle().to_owned())
                    .collect(),
            ),
            "Gestionnaires de paquets présents sur la machine.",
            "Aucun — mais un gestionnaire de moins, c'est une part du parc dont \
             personne ne suit les mises à jour.",
        );

        // Une entrée par application non attribuée : c'est la liste qui fait le
        // livrable. Les applications attribuées ne produisent pas d'item — elles
        // sont couvertes par leur gestionnaire, et les énumérer noierait le signal.
        for app in sans_gestionnaire {
            let valeur = app
                .version
                .clone()
                .map_or(ItemValue::Absent, ItemValue::Text);
            item(
                format!(
                    "inventory.software[{}].version",
                    Application::clef(&app.nom)
                ),
                valeur,
                "Application installée dont le gestionnaire n'a pas été identifié.",
                "Sa mise à jour dépend de toi, ou de l'application elle-même si elle \
                 sait le faire — sauf si un gestionnaire non interrogeable s'en charge.",
            );
        }

        items
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{Application, Gestionnaire, Inventaire};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use windows_registry::{Key, CURRENT_USER, LOCAL_MACHINE};

    /// Les trois vues du registre où Windows déclare les désinstallations.
    ///
    /// La vue 32 bits (`WOW6432Node`) n'est pas un doublon : une application 32 bits
    /// sur un Windows 64 bits n'apparaît que là. L'oublier fait manquer une part
    /// entière du parc — souvent les plus anciennes, donc les plus intéressantes.
    const CHEMINS: &[&str] = &[
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ];

    fn lire_texte(clef: &Key, nom: &str) -> Option<String> {
        clef.get_string(nom).ok().filter(|s| !s.trim().is_empty())
    }

    /// Une entrée du registre décrit-elle une application, ou du bruit ?
    ///
    /// Windows range dans la même clé les applications, leurs correctifs, et des
    /// composants système qui n'ont jamais été « installés » par personne. Les
    /// compter gonflerait l'inventaire d'entrées qu'aucun humain ne reconnaîtrait.
    fn est_application(clef: &Key) -> bool {
        // Un composant système n'est pas une application.
        if clef.get_u32("SystemComponent").unwrap_or(0) == 1 {
            return false;
        }
        // Une entrée rattachée à une autre est un correctif, pas une application.
        if lire_texte(clef, "ParentKeyName").is_some() {
            return false;
        }
        // Sans nom affichable, l'entrée n'est montrable à personne (principe P6).
        lire_texte(clef, "DisplayName").is_some()
    }

    fn applications_du_registre() -> Vec<Application> {
        let mut trouvees = Vec::new();

        for (racine, chemin) in CHEMINS
            .iter()
            .map(|c| (LOCAL_MACHINE, *c))
            .chain(std::iter::once((CURRENT_USER, CHEMINS[0])))
        {
            let Ok(base) = racine.open(chemin) else {
                continue;
            };
            let Ok(sous_clefs) = base.keys() else {
                continue;
            };
            for nom_clef in sous_clefs {
                let Ok(clef) = base.open(&nom_clef) else {
                    continue;
                };
                if !est_application(&clef) {
                    continue;
                }
                let Some(nom) = lire_texte(&clef, "DisplayName") else {
                    continue;
                };
                trouvees.push(Application {
                    nom,
                    version: lire_texte(&clef, "DisplayVersion"),
                    editeur: lire_texte(&clef, "Publisher"),
                    gestionnaire: Gestionnaire::NonAttribue,
                });
            }
        }

        trouvees
    }

    /// Les répertoires d'un gestionnaire, s'il est installé.
    ///
    /// On lit le système de fichiers plutôt que d'invoquer les commandes des
    /// gestionnaires : `winget list` ou `choco list` coûtent des secondes et
    /// lancent des processus, ce qu'un collecteur n'a pas à faire.
    fn paquets_du_repertoire(racine: Option<PathBuf>) -> Vec<String> {
        let Some(racine) = racine else {
            return Vec::new();
        };
        let Ok(entrees) = std::fs::read_dir(&racine) else {
            return Vec::new();
        };
        entrees
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .collect()
    }

    fn chemin_env(variable: &str, suite: &str) -> Option<PathBuf> {
        std::env::var_os(variable).map(|v| PathBuf::from(v).join(suite))
    }

    fn scoop() -> Vec<String> {
        let racine = std::env::var_os("SCOOP")
            .map(PathBuf::from)
            .or_else(|| chemin_env("USERPROFILE", "scoop"))
            .map(|p| p.join("apps"));
        paquets_du_repertoire(racine)
    }

    fn chocolatey() -> Vec<String> {
        let racine = std::env::var_os("ChocolateyInstall")
            .map(PathBuf::from)
            .or_else(|| chemin_env("ProgramData", "chocolatey"))
            .map(|p| p.join("lib"));
        paquets_du_repertoire(racine)
    }

    fn jetbrains() -> Vec<String> {
        paquets_du_repertoire(chemin_env("LOCALAPPDATA", r"JetBrains\Toolbox\apps"))
    }

    fn visual_studio() -> Vec<String> {
        let instances = chemin_env("ProgramData", r"Microsoft\VisualStudio\Packages\_Instances");
        if paquets_du_repertoire(instances).is_empty() {
            Vec::new()
        } else {
            // L'installeur ne nomme pas ses produits par répertoire : on ne prétend
            // pas savoir lesquels, seulement qu'il en gère.
            vec!["visualstudio".to_owned()]
        }
    }

    fn winget_present() -> bool {
        chemin_env("LOCALAPPDATA", r"Microsoft\WindowsApps\winget.exe").is_some_and(|p| p.exists())
    }

    pub(super) fn inventorier() -> Inventaire {
        let mut applications = applications_du_registre();

        // Index des paquets revendiqués par chaque gestionnaire, par clef de
        // rapprochement. `BTreeMap` plutôt que `HashMap` : l'ordre stable rend les
        // diffs de la Phase 1 lisibles, et le volume ne justifie pas mieux.
        let mut revendications: BTreeMap<String, Gestionnaire> = BTreeMap::new();
        let mut gestionnaires = Vec::new();

        for (source, paquets) in [
            (Gestionnaire::Scoop, scoop()),
            (Gestionnaire::Chocolatey, chocolatey()),
            (Gestionnaire::JetBrainsToolbox, jetbrains()),
            (Gestionnaire::VisualStudio, visual_studio()),
        ] {
            if paquets.is_empty() {
                continue;
            }
            gestionnaires.push(source);
            for p in paquets {
                revendications
                    .entry(Application::clef(&p))
                    .or_insert(source);
            }
        }

        if winget_present() {
            gestionnaires.push(Gestionnaire::Winget);
        }

        for app in &mut applications {
            if let Some(g) = revendications.get(&Application::clef(&app.nom)) {
                app.gestionnaire = *g;
            }
        }

        // Dédoublonnage : une même application peut figurer dans deux vues du
        // registre. On garde la première rencontrée, et on préfère toujours une
        // entrée attribuée à une entrée orpheline — sinon un doublon transformerait
        // une application gérée en fausse orpheline.
        applications.sort_by(|a, b| {
            Application::clef(&a.nom)
                .cmp(&Application::clef(&b.nom))
                .then(
                    b.gestionnaire
                        .est_attribue()
                        .cmp(&a.gestionnaire.est_attribue()),
                )
        });
        applications.dedup_by(|a, b| Application::clef(&a.nom) == Application::clef(&b.nom));

        Inventaire {
            applications,
            gestionnaires,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(nom: &str, g: Gestionnaire) -> Application {
        Application {
            nom: nom.to_owned(),
            version: Some("1.0".to_owned()),
            editeur: None,
            gestionnaire: g,
        }
    }

    #[test]
    fn la_clef_rapproche_deux_ecritures_du_meme_nom() {
        // Le rapprochement EST la valeur de ce collecteur : sans lui, chaque source
        // décrit son propre monde et rien ne se croise.
        assert_eq!(Application::clef("Git"), Application::clef("git"));
        assert_eq!(Application::clef("Node.js"), Application::clef("nodejs"));
        assert_eq!(
            Application::clef("Visual Studio Code"),
            Application::clef("visualstudiocode")
        );
        assert_ne!(Application::clef("git"), Application::clef("github"));
    }

    #[test]
    fn une_application_sans_gestionnaire_identifie_est_comptee() {
        let inv = Inventaire {
            applications: vec![
                app("Git", Gestionnaire::Scoop),
                app("Vieux Logiciel", Gestionnaire::NonAttribue),
                app("Autre Vieillerie", Gestionnaire::NonAttribue),
            ],
            // Scoop sait dire ce qu'il gère : l'attribution est complète, donc le
            // pourcentage veut dire quelque chose.
            gestionnaires: vec![Gestionnaire::Scoop],
        };
        assert_eq!(inv.non_attribuees().len(), 2);
        assert!(inv.attribution_incomplete().is_empty());
        assert_eq!(inv.part_non_attribuees(), Some(66));
    }

    #[test]
    fn un_gestionnaire_non_interrogeable_supprime_le_pourcentage() {
        // Le cas rencontré sur une machine réelle : winget présent, donc toutes les
        // applications remontent « non attribuées » — et « 100 % » signifierait
        // « on n'a pas su regarder ». Publier ce chiffre serait une décoration.
        let inv = Inventaire {
            applications: vec![
                app("Brave", Gestionnaire::NonAttribue),
                app("Obsidian", Gestionnaire::NonAttribue),
            ],
            gestionnaires: vec![Gestionnaire::Winget],
        };
        assert_eq!(inv.non_attribuees().len(), 2);
        assert_eq!(inv.attribution_incomplete(), vec![Gestionnaire::Winget]);
        assert_eq!(
            inv.part_non_attribuees(),
            None,
            "un pourcentage calculé sur une attribution incomplète ment"
        );
    }

    #[test]
    fn un_pourcentage_sur_un_inventaire_vide_ne_saffiche_pas() {
        // 0 sur 0 vaut 0 % arithmétiquement, et ne veut rien dire à l'écran.
        // Le brief interdit l'indicateur qu'on ne peut pas expliquer.
        let vide = Inventaire::default();
        assert_eq!(vide.part_non_attribuees(), None);
        assert!(vide.non_attribuees().is_empty());
    }

    #[test]
    fn tout_item_produit_porte_son_explication() {
        // Principe P6 : un item qu'on ne peut pas expliquer n'est pas affichable.
        for item in SoftwareCollector::items() {
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert!(!item.risk.is_empty(), "« {} » sans risque", item.path);
            assert_eq!(
                item.provenance,
                Provenance::Observed,
                "« {} » : un relevé n'est pas une écriture",
                item.path
            );
            assert!(item.desired.is_none(), "un collecteur ne décide de rien");
        }
    }

    #[test]
    fn linventaire_annonce_toujours_son_total_et_lhonnetete_de_sa_mesure() {
        // Ces deux items doivent exister même sur une machine sans rien d'installé,
        // et même hors Windows : une absence de mesure se dit, elle ne se tait pas.
        let chemins: Vec<String> = SoftwareCollector::items()
            .into_iter()
            .map(|i| i.path)
            .collect();
        assert!(chemins.iter().any(|c| c == "inventory.software.total"));
        assert!(chemins
            .iter()
            .any(|c| c == "inventory.software.unattributed"));
        assert!(chemins.iter().any(|c| c == "inventory.software.managers"));
        // Le plus important : le modèle DIT si sa propre mesure est complète.
        assert!(chemins
            .iter()
            .any(|c| c == "inventory.software.attribution"));
    }
}

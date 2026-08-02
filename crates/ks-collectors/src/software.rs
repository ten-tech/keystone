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
//! ## Trois catégories, et pourquoi elles ne se mélangent pas
//!
//! Mesuré sur la machine de référence : **150 applications**, dont
//!
//! * **10 attribuées** à un gestionnaire nommé, winget compris depuis que ses
//!   bases de suivi se lisent (voir [`crate::winget`]) ;
//! * **54 non attribuées** — le livrable. Des applications classiques
//!   qu'aucun gestionnaire ne suit, et c'est là que dorment les versions
//!   vulnérables depuis quatorze mois ;
//! * **86 empaquetées** (MSIX), rangées à part. Un paquet MSIX a toujours un
//!   canal de service, mais le registre ne dit pas lequel : le Store pour ceux
//!   qui en viennent, l'éditeur pour ceux déposés à la main. Vérifié sur trois
//!   paquets de signatures différentes, aucune valeur ne les distingue.
//!
//! La troisième catégorie existe parce que la verser dans la deuxième ferait
//! passer l'indicateur de 54 à 140 sans qu'un seul logiciel de plus soit à
//! l'abandon. Un faux positif sur un indicateur de sécurité coûte sa crédibilité
//! à l'outil entier — et le principe P6 interdit l'indicateur qu'on ne peut pas
//! déplier en ses composantes exactes.
//!
//! ## Lecture seule, sans exception
//!
//! Aucune écriture, aucun processus lancé, aucun effet de bord. On lit quatre
//! vues du registre, quelques répertoires de gestionnaires, et les bases de suivi
//! de winget en mode strictement lecture. Ni `winget list` ni `Get-AppxPackage`
//! ne sont invoqués : lancer un processus pour observer coûte des secondes, et un
//! collecteur ne doit pas peser sur la machine (NF-01).
//!
//! ## Ce qui n'est pas encore fait
//!
//! * **Canal de service des paquets MSIX** — distinguer Store et dépôt manuel
//!   exige une source hors registre.
//! * **Applications à mise à jour autonome** — un navigateur non attribué se met
//!   pourtant à jour seul. « Non attribuée » ne se lit donc pas « jamais mise à
//!   jour ». La distinction est une tâche de la Phase 1.
//! * **Dernier lancement (SRUM)** — base ESE, analyse lourde. Prévu en 0.3 bis.
//!
//! Ces manques sont *déclarés*, pas silencieux : `inventory.software.attribution`
//! vaut « partielle » dès qu'un gestionnaire détecté cesse d'être interrogeable,
//! `unqueryable_managers` le nomme, et le pourcentage se tait alors entièrement.

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
    /// Paquet MSIX dont le canal de service n'est **pas** identifiable.
    ///
    /// Un paquet empaqueté a toujours un canal : le Store le met à jour s'il en
    /// vient, son éditeur s'en charge s'il a été déposé à la main. Le registre ne
    /// dit pas lequel — vérifié sur trois paquets de signatures différentes,
    /// aucune valeur ne les distingue.
    ///
    /// D'où une catégorie à part, et non un rangement parmi les orphelines. Les y
    /// verser gonflerait de 86 unités, sur la machine de référence, un indicateur
    /// que l'utilisateur lirait « personne ne met à jour ces applications » —
    /// alors que le Store en met à jour la plupart. Un faux positif sur un
    /// indicateur de sécurité coûte sa crédibilité à l'outil entier.
    Msix,
    /// **Non attribué** — et surtout pas « aucun ».
    ///
    /// La nuance décide de la valeur du livrable. « Aucun gestionnaire ne la met à
    /// jour » est une affirmation sur le monde ; « je n'ai pas su identifier son
    /// gestionnaire » est une affirmation sur ce que ce collecteur sait faire.
    /// L'attribution winget ayant rejoint le code, la première est enfin vraie
    /// pour les applications classiques — tant que winget reste interrogeable.
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
            Self::Msix => "paquet MSIX, canal non identifié",
            Self::NonAttribue => "non attribué",
        }
    }

    /// L'application a-t-elle un gestionnaire identifié ?
    ///
    /// `Msix` répond non : savoir qu'une application est empaquetée ne dit pas
    /// qui la met à jour. Elle est simplement comptée à part des orphelines.
    #[must_use]
    pub const fn est_attribue(self) -> bool {
        !matches!(self, Self::NonAttribue | Self::Msix)
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
    /// Le nom de la clé de désinstallation qui a produit cette entrée.
    ///
    /// C'est la seule identité stable dont on dispose, et c'est **exactement** ce
    /// que winget range dans sa table `productcodes`. Le rapprochement par ce
    /// champ vaut mieux que par libellé, et un cas réel le prouve : le paquet
    /// `readyfor` s'affiche « Smart Connect » côté registre et « Ready For
    /// Assistant » côté winget. Deux noms sans rapport, qu'aucune comparaison de
    /// chaînes n'aurait reliés.
    pub clef_source: Option<String>,
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
    /// Gestionnaires présents que l'on n'a **pas** su interroger.
    ///
    /// Constaté à l'exécution, et non déduit d'une table de capacités figée : le
    /// premier jet portait un `sait_attribuer()` constant qui décrétait winget
    /// définitivement muet. Il est désormais interrogeable, mais peut cesser de
    /// l'être — base absente, verrouillée, ou schéma inconnu. La capacité dépend
    /// de la machine, pas du type.
    pub non_interrogeables: Vec<Gestionnaire>,
}

impl Inventaire {
    /// **Le livrable.** Les applications classiques qu'aucun gestionnaire ne suit.
    ///
    /// Les paquets MSIX en sont exclus délibérément : ils ont tous un canal de
    /// service, même quand on ne sait pas lequel. Les compter ici ferait passer
    /// l'indicateur de 54 à 140 sur la machine de référence, sans qu'un seul
    /// logiciel de plus soit réellement à l'abandon.
    #[must_use]
    pub fn non_attribuees(&self) -> Vec<&Application> {
        self.applications
            .iter()
            .filter(|a| a.gestionnaire == Gestionnaire::NonAttribue)
            .collect()
    }

    /// Les paquets MSIX dont le canal de service reste à identifier.
    #[must_use]
    pub fn empaquetees(&self) -> Vec<&Application> {
        self.applications
            .iter()
            .filter(|a| a.gestionnaire == Gestionnaire::Msix)
            .collect()
    }

    /// Un gestionnaire est-il présent sans qu'on sache l'interroger ?
    ///
    /// Tant que c'est vrai, « non attribué » ne peut pas se lire « orphelin » :
    /// une application gérée par ce gestionnaire-là serait comptée à tort.
    #[must_use]
    pub fn attribution_incomplete(&self) -> Vec<Gestionnaire> {
        self.non_interrogeables.clone()
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
            "Applications classiques qu'aucun gestionnaire de paquets ne suit. Deux \
             réserves : tant que l'attribution est incomplète, c'est un majorant ; \
             et certaines se mettent à jour seules (navigateurs, éditeurs), donc \
             « non attribuée » ne se lit pas « jamais mise à jour ». Distinguer les \
             deux est une tâche de la Phase 1.",
            "Aucun — cet item est un constat.",
        );

        item(
            "inventory.software.packaged".to_owned(),
            ItemValue::Int(i64::try_from(inv.empaquetees().len()).unwrap_or(-1)),
            "Paquets MSIX dont le canal de service n'est pas identifiable : le Store \
             met à jour ceux qui en viennent, l'éditeur ceux qui ont été déposés à la \
             main, et le registre ne les distingue pas. Comptés à part des non \
             attribuées, parce qu'ils ne sont pas à l'abandon pour autant.",
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
                    clef_source: Some(nom_clef.clone()),
                });
            }
        }

        trouvees
    }

    /// Le dépôt où Windows déclare ses paquets MSIX.
    ///
    /// Sans lui, l'inventaire n'était pas seulement mal attribué : il était
    /// **aveugle**. PowerShell 7 l'a révélé — installé par winget, présent dans
    /// son suivi, et pourtant introuvable dans les trois vues `Uninstall`, parce
    /// qu'un paquet MSIX n'en pose aucune. Mesuré sur la machine de référence :
    /// 63 applications au registre, 87 en MSIX.
    const DEPOT_MSIX: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    /// Découpe un identifiant de paquet `Nom_Version_Arch__Éditeur`.
    ///
    /// Renvoie le nom, la version, et le **nom de famille** `Nom_Éditeur` — la
    /// forme sous laquelle winget range ses paquets MSIX, donc la clé de
    /// rapprochement. Le séparateur est sûr : la spécification MSIX interdit le
    /// tiret bas dans un nom de paquet.
    fn decouper_identifiant(id: &str) -> Option<(String, String, String)> {
        let morceaux: Vec<&str> = id.split('_').collect();
        // Nom, version, architecture, chaîne vide, éditeur : cinq au minimum.
        let (nom, version, editeur) = (
            *morceaux.first()?,
            *morceaux.get(1)?,
            *morceaux.last().filter(|s| !s.is_empty())?,
        );
        if nom.is_empty() || morceaux.len() < 5 {
            return None;
        }
        Some((
            nom.to_owned(),
            version.to_owned(),
            format!("{nom}_{editeur}"),
        ))
    }

    /// Les applications empaquetées, hors composants livrés avec Windows.
    ///
    /// Trois exclusions, chacune mesurée et défendable — 412 clés en donnent 87 :
    ///
    /// * `Framework = 1` : une bibliothèque partagée n'est pas une application,
    ///   personne ne l'a installée pour elle-même ;
    /// * un identifiant contenant `_split.` : un paquet de langue ou de ressources,
    ///   pas un produit ;
    /// * une racine hors de `WindowsApps` : `C:\Windows\SystemApps` abrite les
    ///   274 composants livrés avec le système, que compter comme « installés »
    ///   noierait le signal sous le bruit.
    ///
    /// La dernière exclusion a un coût assumé : elle écarte aussi les rares
    /// paquets déployés hors de ce dossier. Mieux vaut une frontière explicable
    /// qu'une liste exhaustive et illisible (principe P6).
    fn applications_msix() -> Vec<Application> {
        let Ok(depot) = CURRENT_USER.open(DEPOT_MSIX) else {
            return Vec::new();
        };
        let Ok(paquets) = depot.keys() else {
            return Vec::new();
        };

        let mut trouvees = Vec::new();
        for identifiant in paquets {
            let Ok(clef) = depot.open(&identifiant) else {
                continue;
            };
            if clef.get_u32("Framework").unwrap_or(0) == 1 || identifiant.contains("_split.") {
                continue;
            }
            let racine = lire_texte(&clef, "PackageRootFolder").unwrap_or_default();
            if !racine.to_lowercase().contains(r"\windowsapps\") {
                continue;
            }
            let Some((nom_paquet, version, famille)) = decouper_identifiant(&identifiant) else {
                continue;
            };

            // Plus de la moitié des `DisplayName` sont des références
            // `ms-resource:` que seul le chargeur de ressources sait résoudre.
            // Les afficher telles quelles donnerait « ms-resource:/Resources/
            // AppName » à l'écran : un item que l'utilisateur ne peut pas
            // comprendre, donc inaffichable (P6). On retombe alors sur le nom du
            // paquet, moins joli mais lisible.
            let nom = lire_texte(&clef, "DisplayName")
                .filter(|n| !n.starts_with("ms-resource"))
                .unwrap_or(nom_paquet);

            trouvees.push(Application {
                nom,
                version: Some(version),
                editeur: None,
                gestionnaire: Gestionnaire::Msix,
                clef_source: Some(famille),
            });
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

    /// Attribue à winget les applications que ses bases de suivi revendiquent.
    ///
    /// Renvoie `true` si l'interrogation a été complète. Dans le cas contraire, le
    /// gestionnaire rejoint les non interrogeables et le pourcentage se tait —
    /// une attribution partielle ne se laisse pas lire comme une mesure.
    fn attribuer_winget(applications: &mut [Application]) -> bool {
        let suivi = crate::winget::lire_suivi();
        for app in applications.iter_mut() {
            if app.gestionnaire.est_attribue() {
                continue;
            }
            // Deux identités, un seul champ : un code produit pour une entrée du
            // registre, un nom de famille pour un paquet MSIX. winget range les
            // deux, dans deux tables distinctes.
            let revendique = app
                .clef_source
                .as_deref()
                .is_some_and(|c| suivi.revendique_code(c) || suivi.revendique_famille(c));
            if revendique {
                app.gestionnaire = Gestionnaire::Winget;
            }
        }
        suivi.est_complet()
    }

    pub(super) fn inventorier() -> Inventaire {
        let mut applications = applications_du_registre();
        applications.extend(applications_msix());

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

        for app in &mut applications {
            if let Some(g) = revendications.get(&Application::clef(&app.nom)) {
                app.gestionnaire = *g;
            }
        }

        // winget en dernier : les autres gestionnaires revendiquent par nom, lui
        // par code produit. Le code produit est la source la plus sûre, mais il ne
        // doit pas écraser une revendication déjà faite — deux gestionnaires sur
        // la même application est un conflit qui mérite d'être vu, pas arbitré en
        // silence. C'est une question ouverte pour la Phase 1.
        let mut non_interrogeables = Vec::new();
        if winget_present() {
            gestionnaires.push(Gestionnaire::Winget);
            if !attribuer_winget(&mut applications) {
                non_interrogeables.push(Gestionnaire::Winget);
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
            non_interrogeables,
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
            clef_source: None,
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
            non_interrogeables: Vec::new(),
        };
        assert_eq!(inv.non_attribuees().len(), 2);
        assert!(inv.attribution_incomplete().is_empty());
        assert_eq!(inv.part_non_attribuees(), Some(66));
    }

    #[test]
    fn un_gestionnaire_non_interrogeable_supprime_le_pourcentage() {
        // Le cas rencontré sur une machine réelle : winget présent mais muet, donc
        // toutes les applications remontent « non attribuées » — et « 100 % »
        // signifierait « on n'a pas su regarder ». Publier ce chiffre serait une
        // décoration. winget se lit désormais, mais peut redevenir muet : base
        // absente, verrouillée, ou schéma inconnu. Le cas reste donc à couvrir.
        let inv = Inventaire {
            applications: vec![
                app("Brave", Gestionnaire::NonAttribue),
                app("Obsidian", Gestionnaire::NonAttribue),
            ],
            gestionnaires: vec![Gestionnaire::Winget],
            non_interrogeables: vec![Gestionnaire::Winget],
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
    fn un_paquet_msix_ne_grossit_pas_le_compte_des_orphelines() {
        // L'invariant qui empêche l'indicateur de mentir. Un paquet MSIX a
        // toujours un canal de service — le Store, ou son éditeur — même quand le
        // registre ne dit pas lequel. Le verser parmi les orphelines ferait passer
        // le compte de 54 à 140 sur la machine de référence, sans qu'un seul
        // logiciel de plus soit réellement à l'abandon.
        let inv = Inventaire {
            applications: vec![
                app("Vieux Logiciel", Gestionnaire::NonAttribue),
                app("WhatsApp", Gestionnaire::Msix),
                app("Claude", Gestionnaire::Msix),
                app("Git", Gestionnaire::Winget),
            ],
            gestionnaires: vec![Gestionnaire::Winget],
            non_interrogeables: Vec::new(),
        };

        assert_eq!(inv.non_attribuees().len(), 1, "seul le logiciel classique");
        assert_eq!(inv.empaquetees().len(), 2);
        assert!(
            !Gestionnaire::Msix.est_attribue(),
            "empaqueté ne veut pas dire attribué : on ignore qui le met à jour"
        );
        // 1 orpheline sur 4 applications : le dénominateur reste le total, sinon
        // deux items du même écran se compareraient sur des bases différentes.
        assert_eq!(inv.part_non_attribuees(), Some(25));
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

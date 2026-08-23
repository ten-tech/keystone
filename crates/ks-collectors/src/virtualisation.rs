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
//! ## Et le volume qui la porte, parce qu'une taille seule ne se rapproche de rien
//!
//! Le `BasePath` dit aussi **où** vit le fichier. Sans cette moitié, Keystone
//! mesurait 101 Go de disques WSL et 627 Go d'occupation sur `C:` dans le même
//! scan sans jamais les rapprocher : un sixième de ce qui remplit le disque,
//! mesuré et tu (ADR-0022).
//!
//! Seule la **lettre** est publiée, jamais le chemin : un `BasePath` porte le
//! nom de l'utilisateur et celui du paquet, c'est-à-dire plus de surface que
//! n'en demande la question posée. La dérivation vit dans
//! [`crate::lettre_de_volume`], partagée avec la coque pour que les deux côtés
//! du rapprochement normalisent pareil.
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

use ks_core::{Domain, Item, ItemValue, Nature, Provenance};

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
    /// Lettre du volume qui porte ce disque — `C:`, et jamais le chemin.
    ///
    /// `None` quand elle n'est pas dérivable du `BasePath` : chemin UNC, forme
    /// inattendue. Une lettre supposée rattacherait des gibioctets au mauvais
    /// volume sans que rien ne le signale.
    pub volume: Option<String>,
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

    // Ce module produit trois natures sous un seul préfixe, ce qui est la
    // meilleure démonstration que `virtualization.` ne décide de rien :
    // `interop` est un réglage, `disk_bytes` une mesure, `version` un constat.
    let mut item = |chemin: String, nature: Nature, valeur: ItemValue, but: &str, risque: &str| {
        items.push(Item {
            path: chemin,
            domain: Domain::Virtualization,
            nature,
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
        // Un décompte : il suit les installations et les suppressions de
        // distributions. Ce qui se déclarera un jour, ce sont les réglages de
        // chaque distribution, pas leur nombre.
        Nature::Mesure,
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
                // Constat : la génération est celle sous laquelle la
                // distribution a été enregistrée. La convertir n'est pas un
                // réglage qu'on écrit, c'est une migration de disque, et aucun
                // verbe ne la portera sans une ADR à elle.
                Nature::Constat,
                d.version
                    .map_or(ItemValue::Absent, |v| ItemValue::Int(i64::from(v))),
                "Génération du sous-système. Une distribution restée en version 1 \
                 n'a ni noyau réel ni disque virtuel.",
                "Aucun — cet item est un constat.",
            );

            item(
                format!("virtualization.wsl[{clef}].disk_bytes"),
                // Il grossit tout seul, et ne redescend jamais : la mesure type.
                Nature::Mesure,
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
                format!("virtualization.wsl[{clef}].volume"),
                // Constat : le disque est là où WSL l'a créé. Le déplacer est
                // une opération de WSL, pas un réglage qu'on écrit — et aucun
                // verbe ne le portera sans une ADR à lui.
                Nature::Constat,
                d.volume.clone().map_or(ItemValue::Absent, ItemValue::Text),
                "Volume qui porte le disque virtuel — la lettre seule, jamais le \
                 chemin. C'est elle qui permet de retrancher cette taille de \
                 l'occupation du volume, et donc de nommer une part de ce qui le \
                 remplit. Une lettre qu'on n'a pas su dériver se dit « absent » : \
                 rattacher au hasard vaudrait moins que ne rien rattacher.",
                "Aucun — cet item est un constat.",
            );

            item(
                format!("virtualization.wsl[{clef}].interop"),
                // **Réglage, sous un chemin `virtualization.`** : le pendant de
                // `security.firmware.version`. Un drapeau du registre, un état
                // désirable, un verbe qui l'écrira.
                Nature::Reglage,
                d.interop.map_or(ItemValue::Absent, ItemValue::Bool),
                "Exécution de binaires Windows depuis la distribution.",
                "L'intégration élargit la surface entre les deux systèmes : un \
                 script Linux peut lancer un exécutable Windows.",
            );

            item(
                format!("virtualization.wsl[{clef}].drive_mounting"),
                Nature::Reglage,
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
            // `BasePath` est lu **une fois**, et il ne sort pas d'ici. Deux
            // réponses en sont tirées : combien pèse le disque, et sur quel
            // volume il pèse. Le chemin lui-même, qui porte le nom de
            // l'utilisateur et celui du paquet, n'est jamais publié.
            let base = clef.get_string("BasePath").ok();
            trouvees.push(Distribution {
                nom,
                version: clef.get_u32("Version").ok(),
                disque_octets: base.as_deref().and_then(taille_disque),
                volume: base.as_deref().and_then(crate::lettre_de_volume),
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
        sur_volume(nom, octets, Some("C:"))
    }

    /// La même, quand c'est le volume qui porte le disque qui est en jeu.
    fn sur_volume(nom: &str, octets: Option<u64>, volume: Option<&str>) -> Distribution {
        Distribution {
            nom: nom.to_owned(),
            version: Some(2),
            disque_octets: octets,
            volume: volume.map(str::to_owned),
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

        // Une distribution lue produit ses cinq items, et le décompte suit.
        let peuple = Lecture::Trouvee(vec![distro("Debian", Some(56_043_241_472))]);
        assert_eq!(decompte(&peuple).observed, ItemValue::Int(1));
        assert_eq!(
            items_depuis(&peuple).len(),
            6,
            "le décompte plus cinq items"
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
    fn un_item_illisible_garde_sa_nature_et_le_prefixe_ne_la_decide_pas() {
        // Deux règles de l'ADR-0009, éprouvées ensemble parce que ce module
        // les porte toutes les deux.
        //
        // **L'illisibilité est une propriété de la lecture, pas de l'item.**
        // Un décompte qu'on n'a pas su lire reste une mesure : le classer
        // autrement le ferait entrer ou sortir du fichier d'état désiré selon
        // qu'une clé était ouvrable ce jour-là.
        for lecture in [
            Lecture::Trouvee(Vec::new()),
            Lecture::Trouvee(vec![distro("Debian", Some(56_043_241_472))]),
            Lecture::Absente,
            Lecture::Refusee,
        ] {
            assert_eq!(
                decompte(&lecture).nature,
                Nature::Mesure,
                "le décompte change de vocation selon ce que la lecture a donné"
            );
        }

        // **Le préfixe du chemin ne décide de rien.** Trois natures sous le
        // même `virtualization.wsl[…]`, et la plus parlante est `interop` : un
        // réglage hors du domaine `security.`, exactement comme
        // `security.firmware.version` est un constat dedans.
        let items = items_depuis(&Lecture::Trouvee(vec![distro("Debian", Some(1))]));
        let nature = |suffixe: &str| {
            items
                .iter()
                .find(|i| i.path.ends_with(suffixe))
                .unwrap_or_else(|| panic!("« {suffixe} » n'est pas produit"))
                .nature
        };

        assert_eq!(nature(".interop"), Nature::Reglage);
        assert_eq!(nature(".drive_mounting"), Nature::Reglage);
        assert_eq!(nature(".disk_bytes"), Nature::Mesure);
        assert_eq!(nature(".version"), Nature::Constat);
        // Le volume est un constat, et **pas** une mesure : le disque ne change
        // pas de lettre tout seul. Le classer parmi les mesures ne changerait
        // rien à l'affichage, et beaucoup à ce que la Phase 1 en attend.
        assert_eq!(nature(".volume"), Nature::Constat);

        // Et seuls les deux réglages ont vocation à être déclarés : sur les
        // six items d'une distribution, quatre n'ont jamais prétendu être
        // stables.
        assert_eq!(
            items.iter().filter(|i| i.nature.est_declarable()).count(),
            2
        );
    }

    #[test]
    fn le_volume_publie_la_lettre_seule_et_jamais_le_chemin() {
        // **LA BARRIÈRE DE SURFACE.** Un `BasePath` vaut
        // `C:\Users\tenenan\AppData\Local\Packages\TheDebianProject.…\LocalState` :
        // il porte le nom de l'utilisateur ET celui du paquet. La question posée
        // est « sur quel volume ce fichier pèse-t-il ? », et la lettre y répond
        // en entier. Publier le chemin répondrait à des questions qu'on n'a pas
        // posées, dans un rapport qu'on transmet.
        let items = items_depuis(&Lecture::Trouvee(vec![sur_volume(
            "Debian",
            Some(56_043_241_472),
            crate::lettre_de_volume(
                r"C:\Users\tenenan\AppData\Local\Packages\TheDebianProject.Debian\LocalState",
            )
            .as_deref(),
        )]));

        let volume = items
            .iter()
            .find(|i| i.path.ends_with(".volume"))
            .expect("le volume se dit");
        assert_eq!(volume.observed, ItemValue::Text("C:".to_owned()));

        // Et rien du chemin ne fuit par une autre porte : ni la valeur, ni la
        // finalité, ni le risque d'aucun des six items.
        for item in &items {
            let expose = format!("{} {} {}", item.observed, item.purpose, item.risk);
            for fragment in ["tenenan", "AppData", "Packages", "LocalState", r"C:\"] {
                assert!(
                    !expose.contains(fragment),
                    "« {} » laisse passer « {fragment} » : {expose}",
                    item.path
                );
            }
        }
    }

    #[test]
    fn un_volume_non_derivable_se_dit_absent_et_jamais_devine() {
        // Un disque posé sur un partage réseau, ou sous une forme qu'on ne sait
        // pas lire. La tentation serait de rattacher au volume système « parce
        // que c'est presque toujours vrai » : ce presque compte des gibioctets
        // sur un volume qui ne les porte pas, et personne ne le verrait.
        let items = items_depuis(&Lecture::Trouvee(vec![sur_volume(
            "Debian",
            Some(56_043_241_472),
            crate::lettre_de_volume(r"\\serveur\partage\wsl").as_deref(),
        )]));

        let volume = items
            .iter()
            .find(|i| i.path.ends_with(".volume"))
            .expect("le volume se dit même quand on ne l'a pas trouvé");
        assert_eq!(volume.observed, ItemValue::Absent, "aucune lettre devinée");

        // L'absence ne se confond pas avec un refus : on a lu le chemin, on n'a
        // simplement pas su en tirer une lettre.
        assert!(volume.observed.est_constat());
        // Et la taille, elle, reste publiée : ne pas savoir où le disque pèse
        // n'empêche pas de dire combien il pèse.
        let taille = items
            .iter()
            .find(|i| i.path.ends_with(".disk_bytes"))
            .expect("la taille se dit");
        assert_eq!(taille.observed, ItemValue::Int(56_043_241_472));
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

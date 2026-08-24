//! Attribuer la source d'un **changement**, par liste blanche (ADR-0011).
//!
//! ## La règle, et le piège qu'elle évite
//!
//! Une source ne revendique un changement que si **les deux** conditions sont
//! réunies :
//!
//! * le chemin de l'item figure dans l'ensemble déclaré des chemins que cette
//!   source est connue pour toucher ;
//! * l'événement daté de cette source **contient** l'intervalle du changement.
//!
//! Sinon : [`Provenance::Unknown`]. C'est la règle qui garde le signal vivant.
//! Une corrélation temporelle large attribuerait, sur une machine qui installe
//! des correctifs chaque semaine, la quasi-totalité des changements à Windows
//! Update — et `Unknown` redeviendrait inatteignable, donc
//! `Provenance::is_security_signal` toujours faux, donc le mécanisme le plus
//! valorisé du modèle structurellement mort. C'est l'incident déjà consigné,
//! sous une autre forme.
//!
//! **Rien ne rend jamais [`Provenance::Observed`].** `Observed` qualifie une
//! lecture, et un changement n'en est pas une : le repli d'une attribution qui
//! échoue est l'aveu, pas le silence.
//!
//! ## Ce que la Phase 1 n'attribue pas, et pourquoi
//!
//! `Human` et `Application` sont **hors de portée sans élévation**. Seul
//! l'événement 4657, « valeur de registre modifiée », dit qui a écrit une
//! valeur ; il vit dans le journal `Security`, dont la lecture est refusée sans
//! élévation (mesuré, ADR-0011), et il exige en outre qu'une SACL d'audit soit
//! posée — donc une écriture système, interdite avant la Phase 2. Aucune
//! heuristique ne les devine : une attribution fausse est pire qu'une absence
//! d'attribution, parce que personne ne la met en doute.
//!
//! `Keystone` n'est pas attribué non plus, et c'est correct : il se lirait dans
//! le journal des applications de Keystone, et la Phase 1 n'en écrit aucune.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use ks_core::{Change, Provenance};

use crate::posture::Lecture;

/// Les chemins d'items que **Windows Update est connu pour toucher**.
///
/// # Pourquoi une liste, et pourquoi elle est courte
///
/// C'est la moitié qui empêche l'attribution de se diluer. Sur une machine à
/// jour, un correctif tombe chaque semaine ; sans liste, tout changement
/// survenu ce jour-là deviendrait « Windows Update », y compris la
/// désactivation d'une protection.
///
/// Elle sera incomplète au début, et **son incomplétude va dans le sens sûr** :
/// un chemin manquant produit du [`Provenance::Unknown`], jamais une fausse
/// attribution.
///
/// # Ce qui n'y figure pas, et pourquoi ce n'est pas un oubli
///
/// Les versions du moteur et des signatures de Defender sont de nature
/// `Mesure` : elles n'entrent pas dans la série d'observations du magasin
/// (ADR-0014), donc aucun changement n'est jamais construit pour elles. Les y
/// inscrire donnerait une liste qui a l'air plus complète et qui n'attribuerait
/// rien de plus.
pub const CHEMINS_DE_WINDOWS_UPDATE: &[&str] = &[
    // Le numéro de build du noyau : il change à chaque mise à jour cumulative,
    // et c'est le seul item du scan dont Windows Update soit l'auteur
    // structurel.
    "inventory.os.kernel",
    // Les révisions de microcode sont distribuées par Windows Update autant que
    // par le firmware du constructeur. L'attribution reste donc conditionnée à
    // la présence d'un correctif dans la journée : une mise à jour de BIOS ne
    // pose pas de KB, et sortira honnêtement en inconnu.
    "security.firmware.microcode_revision",
];

/// Un correctif appliqué, tel que `Win32_QuickFixEngineering` le rapporte.
///
/// Le champ de date est un **jour**, pas un instant, et le type le dit :
/// `InstalledOn` n'a pas d'heure. C'est toute la restriction assumée de cette
/// source — l'historique COM de `Microsoft.Update.Session`, horodaté à la
/// seconde, exigerait `unsafe` et fait l'objet d'une décision séparée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correctif {
    /// L'identifiant du correctif, `KB5123304` par exemple.
    pub id: String,
    /// Le jour d'installation, tel qu'il a pu être relu.
    pub jour: NaiveDate,
}

impl Correctif {
    /// La fenêtre du correctif : la journée entière, en UTC, borne haute exclue.
    ///
    /// # L'approximation, nommée
    ///
    /// `InstalledOn` ne porte **aucun fuseau**. On lit donc le jour comme un
    /// jour UTC, ce qui décale la fenêtre de quelques heures par rapport à
    /// l'heure locale du poste. Le garde-fou réel n'est pas cette fenêtre mais
    /// la liste blanche : un changement hors liste n'est jamais attribué, quelle
    /// que soit sa date.
    #[must_use]
    pub fn fenetre(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        let debut = Utc.from_utc_datetime(&self.jour.and_time(NaiveTime::MIN));
        (debut, debut + Duration::days(1))
    }
}

/// Ce que les sources d'attribution ont rendu, à un instant donné.
///
/// Chaque source porte une [`Lecture`], donc **trois** états : lue, absente,
/// refusée. Confondre « aucun correctif » et « je n'ai pas pu regarder »
/// reviendrait à conclure d'un silence, ce que ce projet refuse partout
/// ailleurs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sources {
    /// Les correctifs de `Win32_QuickFixEngineering`.
    pub correctifs: Lecture<Vec<Correctif>>,
}

impl Sources {
    /// **Aucune source disponible.** Tout changement sortira en inconnu.
    ///
    /// Nommée plutôt que laissée au `Default` : c'est un cas qu'on veut pouvoir
    /// écrire exprès dans un test, et lire comme tel dans la sortie.
    #[must_use]
    pub fn aucune() -> Self {
        Self {
            correctifs: Lecture::Absente,
        }
    }

    /// Interroge les sources disponibles sur cette plateforme.
    ///
    /// Ne renvoie jamais d'erreur : une source indisponible se range dans son
    /// [`Lecture`], et l'attribution rend alors [`Provenance::Unknown`] — ce
    /// qui est le résultat correct, pas une panne.
    #[must_use]
    pub fn lire() -> Self {
        #[cfg(windows)]
        {
            Self {
                correctifs: windows_impl::correctifs(),
            }
        }
        #[cfg(not(windows))]
        {
            // Hors Windows, la classe n'existe pas. C'est une absence de
            // source, pas un refus : il n'y a rien à lire ici.
            Self::aucune()
        }
    }
}

/// Attribue un changement à une source, ou avoue ne pas savoir.
///
/// L'ordre des contrôles est celui de l'ADR-0011, et il n'est pas
/// interchangeable : **la liste blanche d'abord**, la date ensuite. Un chemin
/// hors liste sort en inconnu sans même que les sources soient consultées, ce
/// qui rend impossible la dérive vers « il y avait un correctif ce jour-là,
/// donc c'était lui ».
#[must_use]
pub fn attribuer(changement: &Change, sources: &Sources) -> Provenance {
    if !CHEMINS_DE_WINDOWS_UPDATE.contains(&changement.path.as_str()) {
        return Provenance::Unknown;
    }
    // Une source absente ou refusée ne revendique rien. Elle ne dément rien non
    // plus : c'est exactement ce que `Unknown` dit.
    let Lecture::Trouvee(correctifs) = &sources.correctifs else {
        return Provenance::Unknown;
    };
    let revendique = correctifs.iter().any(|c| {
        let (debut, fin) = c.fenetre();
        changement.fits_within(debut, fin)
    });
    if revendique {
        Provenance::WindowsUpdate
    } else {
        Provenance::Unknown
    }
}

/// Attribue une série de changements, en place.
///
/// Les sources sont lues **une fois** et passées à chaque changement : les
/// relire par changement multiplierait les allers-retours WMI, et pire, ferait
/// dépendre l'attribution de l'instant où l'on regarde.
pub fn attribuer_tous(changements: &mut [Change], sources: &Sources) {
    for changement in changements.iter_mut() {
        changement.provenance = attribuer(changement, sources);
    }
}

/// Relit `InstalledOn` tel que `Win32_QuickFixEngineering` l'écrit.
///
/// # Une chaîne, pas une date
///
/// La propriété est déclarée `string` dans la classe CIM ; c'est l'installeur
/// qui l'écrit, et Microsoft documente deux formes : `MM/JJ/AAAA`, ou un
/// `FILETIME` en hexadécimal. Les deux sont relues ici, et rien d'autre.
///
/// # L'ordre jour/mois, mesuré et non supposé
///
/// Relevé sur la machine de référence le 2026-08-17, en session non élevée :
/// `8/14/2026`, `8/13/2026`, `10/8/2025`. Les deux premiers **prouvent** que le
/// premier nombre est le mois, aucun mois ne valant 14 ni 13 — et la valeur est
/// écrite en `M/J/AAAA` malgré une machine en fr-FR, donc indépendamment de la
/// locale.
///
/// C'est la différence avec [`crate::posture::date_firmware_certaine`], qui
/// refuse l'ambiguïté : là, aucune spécification ne tranche et deux firmwares
/// voisins écrivent dans des ordres différents ; ici, la forme est documentée
/// **et** mesurée. Le prix d'une erreur reste borné par la liste blanche.
///
/// Toute autre forme rend `None`, et le correctif ne fournit alors aucune
/// fenêtre. Un correctif sans date n'attribue rien ; il n'invente pas un jour.
#[must_use]
pub fn jour_dinstallation(brut: &str) -> Option<NaiveDate> {
    /// Une date hors de cet intervalle n'est pas une date, ce sont des octets.
    /// Mêmes bornes que [`crate::posture::filetime_vers_horodatage`], et pour
    /// la même raison : une date absurde dans un journal forensique se corrèle
    /// avec d'autres événements et fabrique une histoire.
    const ANNEES: std::ops::Range<i32> = 2000..2100;

    let brut = brut.trim();
    let jour = if let Some(hexa) = brut.strip_prefix("0x").or_else(|| brut.strip_prefix("0X")) {
        u64::from_str_radix(hexa, 16)
            .ok()
            .and_then(crate::posture::filetime_vers_horodatage)
            .map(|d| d.date_naive())?
    } else {
        NaiveDate::parse_from_str(brut, "%m/%d/%Y").ok()?
    };
    ANNEES
        .contains(&chrono::Datelike::year(&jour))
        .then_some(jour)
}

#[cfg(windows)]
mod windows_impl {
    use super::{jour_dinstallation, Correctif};
    use crate::posture::Lecture;
    use serde::Deserialize;

    /// Projection de `Win32_QuickFixEngineering`.
    ///
    /// **Le nom sérialisé est le nom de la classe WMI**, pas une coquetterie :
    /// la bibliothèque construit sa requête à partir de lui. Les deux champs
    /// sont optionnels — un correctif sans identifiant ou sans date ne doit pas
    /// faire échouer la lecture des autres.
    #[derive(Deserialize, Debug)]
    #[serde(rename = "Win32_QuickFixEngineering", rename_all = "PascalCase")]
    struct QuickFixEngineering {
        #[serde(rename = "HotFixID")]
        hot_fix_id: Option<String>,
        installed_on: Option<String>,
    }

    /// Les correctifs appliqués, lus dans `root\CIMV2`.
    ///
    /// Mesuré lisible en session non élevée sur la machine de référence — c'est
    /// pourquoi cette source est retenue quand le journal `Security`, lui, est
    /// refusé.
    pub(super) fn correctifs() -> Lecture<Vec<Correctif>> {
        crate::etat_effectif::sur_un_fil_dedie(crate::etat_effectif::DELAI_WMI, interroger, || {
            Lecture::Refusee
        })
    }

    fn interroger() -> Lecture<Vec<Correctif>> {
        use wmi::{COMLibrary, WMIConnection};

        let Ok(com) = COMLibrary::new() else {
            return Lecture::Refusee;
        };
        let Ok(connexion) = WMIConnection::new(com) else {
            return Lecture::Refusee;
        };
        let Ok(lignes) = connexion.query::<QuickFixEngineering>() else {
            return Lecture::Refusee;
        };

        // Un correctif dont la date est illisible est **écarté**, pas daté au
        // hasard : il ne fournit aucune fenêtre, donc il n'attribue rien. C'est
        // la même règle que pour un `FILETIME` invraisemblable.
        Lecture::Trouvee(
            lignes
                .into_iter()
                .filter_map(|l| {
                    Some(Correctif {
                        id: l.hot_fix_id?,
                        jour: jour_dinstallation(&l.installed_on?)?,
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_core::ItemValue;

    fn jour(annee: i32, mois: u32, jour: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(annee, mois, jour).expect("date de test valide")
    }

    fn instant(mois: u32, j: u32, heure: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, mois, j, heure, 0, 0)
            .single()
            .expect("date de test valide")
    }

    fn changement(chemin: &str, debut: DateTime<Utc>, fin: DateTime<Utc>) -> Change {
        Change::unattributed(
            chemin,
            (ItemValue::Text("26100".into()), debut),
            (ItemValue::Text("26200".into()), fin),
        )
    }

    /// Les correctifs réellement relevés sur la machine de référence.
    fn correctifs_de_reference() -> Sources {
        Sources {
            correctifs: Lecture::Trouvee(vec![
                Correctif {
                    id: "KB5123304".into(),
                    jour: jour(2026, 8, 13),
                },
                Correctif {
                    id: "KB5121003".into(),
                    jour: jour(2026, 8, 14),
                },
            ]),
        }
    }

    #[test]
    fn un_changement_contenu_dans_la_journee_dun_correctif_sattribue_a_windows_update() {
        let c = changement("inventory.os.kernel", instant(8, 13, 6), instant(8, 13, 18));
        assert_eq!(
            attribuer(&c, &correctifs_de_reference()),
            Provenance::WindowsUpdate
        );
    }

    #[test]
    fn un_changement_hors_liste_blanche_reste_sans_auteur() {
        // **La barrière du lot.** Le correctif couvre parfaitement l'intervalle,
        // et ça ne suffit pas : Windows Update ne touche pas la protection en
        // temps réel, donc il ne la revendique pas. Sans cette moitié, un
        // attaquant qui désactive Defender un mardi de correctif verrait son
        // geste signé « Windows Update ».
        for chemin in [
            "security.defender.realtime",
            "security.defender.asr_rules.local",
            "security.services.windefend.startup",
            "security.firewall.public.enabled",
            "inventory.os.name",
        ] {
            let c = changement(chemin, instant(8, 13, 6), instant(8, 13, 18));
            let p = attribuer(&c, &correctifs_de_reference());
            assert_eq!(
                p,
                Provenance::Unknown,
                "« {chemin} » n'est pas dans la liste blanche de Windows Update"
            );
            assert!(
                p.is_security_signal(),
                "« {chemin} » : un changement sans auteur DOIT rester un signal"
            );
        }
    }

    #[test]
    fn un_changement_a_cheval_sur_deux_jours_ne_sattribue_a_personne() {
        // La restriction assumée de `InstalledOn`, éprouvée : la source est
        // datée à la journée, donc elle ne prouve rien sur un intervalle plus
        // large qu'une journée. La dire sans la tenir serait pire que de ne
        // pas l'avoir.
        for (debut, fin) in [
            (instant(8, 12, 22), instant(8, 13, 18)),
            (instant(8, 13, 6), instant(8, 14, 3)),
            (instant(8, 13, 6), instant(8, 14, 18)),
        ] {
            assert_eq!(
                attribuer(
                    &changement("inventory.os.kernel", debut, fin),
                    &correctifs_de_reference()
                ),
                Provenance::Unknown,
                "intervalle {debut} → {fin} : il déborde la journée du correctif"
            );
        }
    }

    #[test]
    fn un_changement_sans_correctif_ce_jour_la_reste_sans_auteur() {
        // Chemin dans la liste blanche, mais aucune journée de correctif ne
        // contient l'intervalle. La liste blanche seule n'attribue rien.
        assert_eq!(
            attribuer(
                &changement("inventory.os.kernel", instant(8, 16, 6), instant(8, 16, 18)),
                &correctifs_de_reference()
            ),
            Provenance::Unknown
        );
    }

    #[test]
    fn toutes_sources_indisponibles_tout_reste_inconnu() {
        // Le critère qui compte : **rien** ne rend `Observed` par défaut. Un
        // changement dont on ne sait rien est un aveu, pas un relevé.
        for sources in [
            Sources::aucune(),
            Sources {
                correctifs: Lecture::Refusee,
            },
            Sources {
                correctifs: Lecture::Trouvee(Vec::new()),
            },
        ] {
            for chemin in CHEMINS_DE_WINDOWS_UPDATE.iter().copied().chain([
                "security.defender.realtime",
                "virtualization.wsl.distro_count",
            ]) {
                let c = changement(chemin, instant(8, 13, 6), instant(8, 13, 18));
                let p = attribuer(&c, &sources);
                assert_eq!(p, Provenance::Unknown, "« {chemin} » avec {sources:?}");
                assert_ne!(
                    p,
                    Provenance::Observed,
                    "« {chemin} » : un changement n'est jamais un relevé"
                );
                assert!(p.is_security_signal());
            }
        }
    }

    #[test]
    fn attribuer_tous_ne_laisse_aucun_changement_en_arriere() {
        let mut changements = vec![
            changement("inventory.os.kernel", instant(8, 13, 6), instant(8, 13, 18)),
            changement(
                "security.defender.realtime",
                instant(8, 13, 6),
                instant(8, 13, 18),
            ),
        ];
        attribuer_tous(&mut changements, &correctifs_de_reference());
        assert_eq!(changements[0].provenance, Provenance::WindowsUpdate);
        assert!(!changements[0].is_security_signal());
        assert_eq!(changements[1].provenance, Provenance::Unknown);
        assert!(changements[1].is_security_signal());
    }

    #[test]
    fn la_date_dinstallation_se_relit_dans_les_deux_formes_documentees() {
        // Les trois valeurs mesurées sur la machine de référence, telles quelles.
        assert_eq!(jour_dinstallation("8/14/2026"), Some(jour(2026, 8, 14)));
        assert_eq!(jour_dinstallation("8/13/2026"), Some(jour(2026, 8, 13)));
        // Celle qui prouve que le premier nombre est le mois : lue « 8 octobre »
        // par Windows, donc `10` est bien le mois.
        assert_eq!(jour_dinstallation("10/8/2025"), Some(jour(2025, 10, 8)));
        // Les formes rembourrées, que d'autres installeurs écrivent.
        assert_eq!(jour_dinstallation(" 08/14/2026 "), Some(jour(2026, 8, 14)));

        // Le `FILETIME` hexadécimal, seconde forme documentée. La valeur est
        // calculée par aller-retour, pas de tête : c'est ainsi qu'une constante
        // d'époque fausse se voit.
        let attendu = instant(8, 14, 12);
        let ticks = u64::try_from(attendu.timestamp() + 11_644_473_600).expect("époque positive")
            * 10_000_000;
        assert_eq!(
            jour_dinstallation(&format!("0x{ticks:x}")),
            Some(jour(2026, 8, 14))
        );
    }

    #[test]
    fn une_date_dinstallation_illisible_ne_fabrique_pas_de_fenetre() {
        // Un correctif sans date exploitable n'attribue rien. Il ne se voit pas
        // attribuer « aujourd'hui » faute de mieux — ce serait inventer une
        // fenêtre, donc une attribution.
        for brut in [
            "",
            "   ",
            "14/08/2026", // jour en premier : `%m` refuse 14
            "8/14",
            "hier",
            "0xzz",
            "0x0", // 1601 : la conversion est juste, le résultat est absurde
        ] {
            assert_eq!(
                jour_dinstallation(brut),
                None,
                "« {brut} » ne décrit pas une journée exploitable"
            );
        }
    }

    #[test]
    fn la_fenetre_dun_correctif_est_une_journee_entiere_borne_haute_exclue() {
        let c = Correctif {
            id: "KB5123304".into(),
            jour: jour(2026, 8, 13),
        };
        let (debut, fin) = c.fenetre();
        assert_eq!(debut, instant(8, 13, 0));
        assert_eq!(fin, instant(8, 14, 0));
        assert_eq!(fin - debut, Duration::days(1));
    }

    #[test]
    fn la_liste_blanche_ne_contient_que_des_chemins_dont_windows_update_est_lauteur() {
        // Le contrôle qui empêche l'élargissement discret. Une posture de
        // sécurité ne se met pas à jour par Windows Update : y inscrire un
        // `security.defender.*` ou un `security.services.*` rendrait
        // attribuable, donc invisible, exactement ce que le §6 du modèle de
        // menace place en tête.
        for chemin in CHEMINS_DE_WINDOWS_UPDATE {
            assert!(
                !chemin.starts_with("security.defender.")
                    && !chemin.starts_with("security.services.")
                    && !chemin.starts_with("security.firewall.")
                    && !chemin.starts_with("security.platform."),
                "« {chemin} » : une protection ne s'attribue pas à Windows Update"
            );
        }
        assert!(
            !CHEMINS_DE_WINDOWS_UPDATE.is_empty(),
            "sans chemin, l'attribution ne peut plus rien attribuer et le contrôle ne \
             contrôle plus rien"
        );
    }

    #[test]
    fn la_lecture_des_sources_ne_fait_jamais_tomber_lappelant() {
        // Sur toute plateforme, y compris sans WMI : la fonction rend la main,
        // et son résultat est l'un des trois états de `Lecture` — jamais une
        // panique, jamais une liste inventée.
        let sources = Sources::lire();
        assert!(matches!(
            sources.correctifs,
            Lecture::Trouvee(_) | Lecture::Absente | Lecture::Refusee
        ));
    }
}

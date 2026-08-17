//! La ruche de politique — une **origine**, pas une supposition (ADR-0018).
//!
//! ## Ce que ce module décide, et pourquoi ce n'est pas une heuristique
//!
//! Une valeur lue sous `HKLM\SOFTWARE\Policies\…` n'a pas d'autre auteur
//! possible : c'est ainsi que fonctionne la ruche. Le collecteur de posture le
//! sait déjà et le dit dans ses chemins —
//! `security.defender.asr_rules.policy` d'un côté, `.local` de l'autre — mais
//! il jetait l'information au moment précis où elle servait. Ce module la
//! retient et la porte jusqu'à [`ks_core::Provenance::Managed`].
//!
//! Le gain n'est pas cosmétique : sans lui, `Provenance::is_sovereign` n'est
//! vrai nulle part, donc [`ks_core::DriftStatus::Conflict`] n'est jamais
//! construit, donc le principe P10 — « la MDM est souveraine » — n'a aucun
//! support dans les données.
//!
//! ## Ce que ce module refuse de faire
//!
//! **Nommer une autorité qu'on n'a pas identifiée.** L'ADR-0011 a mesuré, sur
//! une machine inscrite à aucune MDM, **31 sous-clés** sous `Enrollments`,
//! toutes en `EnrollmentState = 1` : conclure « inscrit donc Intune » produirait
//! un faux positif sur chaque Windows 11. L'autorité publiée est donc la ruche
//! elle-même, [`AUTORITE`], et D12-01 reste ouverte.
//!
//! **Marquer une clé absente.** Une clé de politique qui n'existe pas n'impose
//! rien ; une clé refusée n'a rien dit. Dans les deux cas, aucune valeur n'a
//! été lue, donc aucune autorité n'est attestée. Marquer ces cas rendrait
//! `is_sovereign` vrai sur toute machine, donc tout non convergeable — le
//! symétrique exact de l'incident consigné, où marquer les relevés `Keystone`
//! rendait `Unknown` inatteignable.

use ks_core::{Item, ItemValue, Provenance};

/// L'autorité nommée par un relevé issu de la ruche de politique.
///
/// Pas `Intune`, pas `GPO de domaine` : la ruche ne dit pas **qui** a écrit la
/// politique, seulement qu'une autorité l'impose. Nommer l'un des deux serait
/// inventer, et une fausse attribution ne se corrige jamais parce que personne
/// ne la met en doute.
pub const AUTORITE: &str = "stratégie de groupe ou MDM";

/// Les chemins d'items dont la valeur est lue **sous une ruche de politique**.
///
/// # Source de vérité unique
///
/// Le collecteur consulte cette liste pour marquer, le test pour vérifier. Une
/// seconde liste écrite dans le test ferait du test la source de vérité, et
/// elle dériverait au premier item ajouté — c'est le défaut que l'ADR-0018
/// nomme explicitement.
///
/// # Pourquoi un seul chemin, et pourquoi ce n'est pas un oubli
///
/// C'est le seul que les collecteurs lisent aujourd'hui sous
/// `HKLM\SOFTWARE\Policies\…`. La liste grandira avec eux ; son incomplétude
/// se voit et va dans le sens sûr — elle produit des relevés `Observed`, jamais
/// une souveraineté inventée.
pub const CHEMINS: &[&str] = &["security.defender.asr_rules.policy"];

/// Marque un relevé comme imposé par une autorité, si et seulement s'il l'est.
///
/// Deux conditions, toutes deux nécessaires :
///
/// * le chemin figure dans [`CHEMINS`], c'est-à-dire que le collecteur a
///   réellement lu cette valeur sous la ruche de politique ;
/// * **une valeur a été lue** — voir `valeur_lue`, privée à ce module. Une clé absente ou
///   refusée n'atteste aucune autorité.
///
/// La fonction prend et rend l'[`Item`] plutôt que de le muter en place : la
/// décision se lit alors dans l'expression qui construit l'item, à côté du
/// chemin qui la justifie, et non trois lignes plus bas.
#[must_use]
pub fn marquer(mut item: Item) -> Item {
    if CHEMINS.contains(&item.path.as_str()) && valeur_lue(&item.observed) {
        item.provenance = Provenance::Managed(AUTORITE.to_owned());
    }
    item
}

/// Une valeur a-t-elle réellement été lue, ou n'a-t-on rien trouvé ?
///
/// Une liste **vide** compte comme lue : la clé existe sous la ruche, une
/// autorité l'a donc créée, et « cette autorité n'impose aucune règle » est un
/// fait. Une absence et un refus, eux, ne disent rien de l'autorité.
///
/// Le `match` est **exhaustif sans bras `_`** : ajouter une variante à
/// [`ItemValue`] casse ici la compilation, donc la CI, avant qu'un test
/// s'exécute. Le contributeur doit venir décider si sa nouvelle forme de valeur
/// atteste une autorité.
fn valeur_lue(valeur: &ItemValue) -> bool {
    match valeur {
        ItemValue::Absent | ItemValue::Illisible { .. } => false,
        ItemValue::Bool(_) | ItemValue::Int(_) | ItemValue::Text(_) | ItemValue::List(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_core::{Domain, Drift, DriftStatus, Nature, Severity};

    fn releve(chemin: &str, valeur: ItemValue) -> Item {
        Item {
            path: chemin.to_owned(),
            domain: Domain::Security,
            nature: Nature::Reglage,
            desired: None,
            observed: valeur,
            observed_at: chrono::Utc::now(),
            // La provenance de départ d'un relevé, celle que `marquer` a le
            // droit — et le seul droit — de remplacer.
            provenance: Provenance::Observed,
            purpose: "Règles ASR imposées par stratégie.".into(),
            risk: "Une règle en audit journalise sans bloquer.".into(),
            reference: None,
        }
    }

    #[test]
    fn une_valeur_lue_sous_la_ruche_rend_le_releve_souverain() {
        let item = marquer(releve(
            CHEMINS[0],
            ItemValue::List(vec!["r = bloque".into()]),
        ));
        assert_eq!(item.provenance, Provenance::Managed(AUTORITE.to_owned()));
        assert!(item.provenance.is_sovereign());
        // Et surtout pas un signal : un relevé n'est pas un changement.
        assert!(!item.provenance.is_security_signal());
    }

    #[test]
    fn une_cle_de_politique_absente_ou_refusee_ne_fabrique_aucune_autorite() {
        // Le piège d'`Enrollments`, transposé : marquer ce qui n'a pas été lu
        // rendrait `is_sovereign` vrai sur toute machine, donc tout non
        // convergeable. Mesuré sur la machine de référence, la branche de
        // stratégie ASR n'existe pas — c'est exactement ce cas-là.
        for valeur in [
            ItemValue::Absent,
            ItemValue::illisible("accès refusé sans élévation"),
        ] {
            let item = marquer(releve(CHEMINS[0], valeur.clone()));
            assert_eq!(
                item.provenance,
                Provenance::Observed,
                "« {valeur:?} » : rien n'a été lu, donc aucune autorité n'est attestée"
            );
            assert!(!item.provenance.is_sovereign());
        }
    }

    #[test]
    fn un_chemin_hors_ruche_ne_devient_jamais_managed() {
        // La moitié qui barre l'élargissement. `asr_rules.local` partage tout
        // avec `asr_rules.policy` sauf le dernier segment, et il vient de la
        // branche locale — celle qu'alimente `Add-MpPreference`.
        for chemin in [
            "security.defender.asr_rules.local",
            "security.defender.realtime",
            "inventory.os.kernel",
        ] {
            let item = marquer(releve(chemin, ItemValue::Bool(true)));
            assert_eq!(
                item.provenance,
                Provenance::Observed,
                "« {chemin} » n'est pas lu sous la ruche de politique"
            );
        }
    }

    #[test]
    fn lautorite_ne_se_donne_pas_un_nom_quon_na_pas_identifie() {
        // La ruche dit qu'une autorité impose la valeur ; elle ne dit pas
        // laquelle. Un `reg add` sous `HKLM\SOFTWARE\Policies` produit la même
        // lecture qu'une GPO de domaine — l'ADR-0018 l'assume et le documente.
        let minuscules = AUTORITE.to_lowercase();
        for invente in ["intune", "gpo", "configuration manager", "wsus"] {
            assert!(
                !minuscules.contains(invente),
                "l'autorité s'annonce « {AUTORITE} » : elle ne doit nommer personne"
            );
        }
    }

    #[test]
    fn un_releve_de_la_ruche_rend_le_conflit_de_politique_constructible() {
        // Ce que ce lot débloque, montré de bout en bout. `DriftStatus::Conflict`
        // existe dans le modèle et il est testé depuis la Phase 0, mais rien ne
        // pouvait le construire : `is_sovereign` n'était vrai nulle part.
        //
        // L'autorité du conflit vient du relevé, pas d'une constante recopiée —
        // sans quoi l'écran afficherait une autorité et le collecteur une autre.
        let item = marquer(releve(
            CHEMINS[0],
            ItemValue::List(vec!["r = bloque".into()]),
        ));
        let Provenance::Managed(autorite) = item.provenance.clone() else {
            panic!("le relevé devait être souverain : {:?}", item.provenance)
        };

        let now = chrono::Utc::now();
        let conflit = Drift {
            item,
            severity: Severity::Info,
            status: DriftStatus::Conflict {
                authority: autorite,
                // L'oscillation exige de voir une valeur repoussée à chaque
                // cycle, donc l'historique du magasin sur plusieurs jours
                // (D12-04). Rien ici ne la détecte, et le dire vaut mieux que
                // de publier un `true` qu'on n'a pas mesuré.
                oscillating: false,
            },
            first_seen: now,
            last_seen: now,
        };

        assert!(
            !conflit.is_convergeable(now.date_naive()),
            "principe P10 : une politique gérée n'est jamais convergeable"
        );
        assert!(
            !conflit.is_security_signal(),
            "un conflit de politique est un état connu, pas un signal de sécurité"
        );
    }
}

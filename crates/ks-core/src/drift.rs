//! L'écart entre l'état désiré et l'état réel — et sa qualification.

use serde::{Deserialize, Serialize};

use crate::{Item, Timestamp};

/// Gravité d'un écart.
///
/// Trois niveaux, pas cinq. Une échelle qu'on ne sait pas expliquer produit des
/// alertes qu'on ne sait pas trier. Et il n'y a **pas** de niveau « urgent » :
/// la gravité se dit par le mot juste, jamais par l'emphase (brief design §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// À savoir. N'interrompt jamais.
    Info,
    /// Mérite un regard. Entre dans le rapport matinal.
    Attention,
    /// État confirmé grave. Peut consommer une des deux interruptions du jour.
    Serious,
}

impl Severity {
    /// Libellé affiché. Toujours accompagné d'une icône — jamais la couleur seule
    /// (exigence NF-05 : aucun sens porté par la couleur).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "Information",
            Self::Attention => "Attention",
            Self::Serious => "Grave",
        }
    }
}

/// Statut d'un écart dans son cycle de vie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "status")]
pub enum DriftStatus {
    /// Écart mesuré, non traité.
    Active,

    /// Écart volontairement toléré.
    ///
    /// Exigence D2-06 : **la raison et la date d'expiration sont obligatoires.**
    /// C'est le seul mécanisme qui empêche un fichier d'état de pourrir sur trois
    /// ans sous une couche d'exceptions dont personne ne se rappelle le motif.
    Accepted {
        /// Pourquoi cet écart est toléré.
        reason: String,
        /// Le dernier jour où la tolérance vaut, **inclus**.
        ///
        /// Une date civile, et non un horodatage : c'est ce que l'humain écrit
        /// dans `workstation.yaml` (`expires: 2026-10-15`), et convertir cette
        /// date en instant obligerait à choisir une heure que personne n'a
        /// donnée. Le choix se ferait en silence, et l'échéance basculerait à
        /// 02:00 heure locale sur un poste en heure d'été.
        ///
        /// L'inclusivité est éprouvée des deux côtés par
        /// `une_acceptation_est_en_vigueur_le_jour_de_son_echeance` et
        /// `une_acceptation_expire_le_lendemain` : une convention qui n'est
        /// vérifiée que sur une borne se retourne au premier passage.
        expires: chrono::NaiveDate,
        /// Qui a décidé.
        decided_by: String,
        /// Quand la décision a été prise.
        decided_at: Timestamp,
    },

    /// Écart causé par une autorité souveraine (MDM, GPO).
    ///
    /// Exigence D12-03 : ce n'est **pas** une dérive, et ce n'est pas convergeable.
    /// Les confondre produit une guerre de politiques que Keystone perd toujours.
    Conflict {
        /// L'autorité qui impose la valeur.
        authority: String,
        /// L'écart a-t-il déjà été observé en oscillation (repoussé à chaque cycle) ?
        ///
        /// Exigence D12-04 : un item repoussé par la MDM à chaque cycle est identifié
        /// comme tel plutôt que reconverti en boucle.
        oscillating: bool,
    },
}

/// Un écart mesuré, qualifié, et attribué à son auteur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drift {
    /// L'item concerné, avec ses valeurs voulue et constatée.
    pub item: Item,
    /// Gravité.
    pub severity: Severity,
    /// Statut dans le cycle de vie.
    pub status: DriftStatus,
    /// Première fois que cet écart a été vu.
    pub first_seen: Timestamp,
    /// Dernière confirmation de l'écart.
    pub last_seen: Timestamp,
}

impl Drift {
    /// L'écart peut-il faire l'objet d'une convergence ?
    ///
    /// Un conflit MDM ne peut pas : l'autorité est souveraine (principe P10).
    /// Un écart accepté non expiré ne doit pas : c'est une décision humaine.
    ///
    /// `aujourd_hui` est **passée en paramètre**, jamais lue d'une horloge ici :
    /// une fonction de modèle qui appelle `Utc::now()` n'est pas testable sur ses
    /// bornes, et ce sont précisément les bornes qui comptent.
    #[must_use]
    pub fn is_convergeable(&self, aujourd_hui: chrono::NaiveDate) -> bool {
        match &self.status {
            DriftStatus::Active => true,
            DriftStatus::Conflict { .. } => false,
            DriftStatus::Accepted { expires, .. } => *expires < aujourd_hui,
        }
    }

    /// L'écart doit-il être remonté comme constat de sécurité ?
    ///
    /// Règle D2-05 : un changement dont l'auteur est indéterminable est un signal,
    /// quelle que soit la banalité de l'item touché.
    #[must_use]
    pub fn is_security_signal(&self) -> bool {
        matches!(self.status, DriftStatus::Active) && self.item.provenance.is_security_signal()
    }

    /// Une acceptation arrivée à échéance redevient un écart actif, sans intervention.
    ///
    /// « Arrivée à échéance » veut dire **le lendemain** du jour inscrit : la
    /// tolérance vaut encore tout le jour de son échéance. C'est la lecture
    /// qu'attend quiconque écrit `expires: 2026-10-15`, et l'autre convention
    /// retirerait une journée sans le dire.
    #[must_use]
    pub fn is_expired_acceptance(&self, aujourd_hui: chrono::NaiveDate) -> bool {
        matches!(&self.status, DriftStatus::Accepted { expires, .. } if *expires < aujourd_hui)
    }
}

/// Résumé d'un état de dérive, tel qu'affiché sur la tuile de la vue d'ensemble.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct DriftSummary {
    /// Items observés au total.
    pub observed: usize,
    /// Items conformes.
    pub compliant: usize,
    /// Écarts actifs.
    pub active: usize,
    /// Écarts acceptés, non expirés.
    pub accepted: usize,
    /// Conflits avec une autorité souveraine.
    pub conflicts: usize,
    /// Écarts actifs sans auteur identifié.
    pub unattributed: usize,
    /// **Items qu'on n'a pas pu lire.**
    ///
    /// Ni conformes, ni en écart. Sans ce compteur, `compliant` les absorbait :
    /// il se calculait par soustraction, donc tout ce qui n'était pas un écart
    /// déclaré comptait pour conforme — y compris une exclusion Defender posée
    /// sous une clé qu'on ne sait pas lire. L'outil affichait vert sur ce qu'il
    /// n'avait pas regardé, ce qui est le pire des trois états possibles.
    pub unreadable: usize,
}

impl DriftSummary {
    /// Construit le résumé depuis une liste d'écarts et un total d'items observés.
    ///
    /// `items_unreadable` est **obligatoire**, et non un `Option` avec une
    /// valeur par défaut : un appelant qui l'oublie doit être arrêté par le
    /// compilateur, parce que l'oublier revient à recompter les illisibles
    /// comme conformes — le défaut précis que ce paramètre corrige.
    #[must_use]
    pub fn build(
        items_observed: usize,
        items_unreadable: usize,
        drifts: &[Drift],
        aujourd_hui: chrono::NaiveDate,
    ) -> Self {
        let mut s = Self {
            observed: items_observed,
            unreadable: items_unreadable,
            ..Self::default()
        };
        for d in drifts {
            match &d.status {
                DriftStatus::Active => s.active += 1,
                DriftStatus::Conflict { .. } => s.conflicts += 1,
                DriftStatus::Accepted { expires, .. } => {
                    if *expires < aujourd_hui {
                        s.active += 1; // l'acceptation a expiré : l'écart est de retour
                    } else {
                        s.accepted += 1;
                    }
                }
            }
            if d.is_security_signal() {
                s.unattributed += 1;
            }
        }
        // Les illisibles sortent du décompte des conformes. Le calcul reste une
        // soustraction, mais il ne peut plus absorber ce qu'on n'a pas lu.
        s.compliant = items_observed
            .saturating_sub(s.active + s.accepted + s.conflicts)
            .saturating_sub(s.unreadable);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Domain, ItemValue, Nature, Provenance};
    use chrono::Utc;

    fn drift(status: DriftStatus, provenance: Provenance) -> Drift {
        let now = Utc::now();
        Drift {
            item: Item {
                path: "services.Fax.startupType".into(),
                domain: Domain::Configuration,
                // Un type de démarrage de service a un état désirable, et un
                // verbe l'écrira : c'est un réglage.
                nature: Nature::Reglage,
                desired: Some(ItemValue::Text("Disabled".into())),
                observed: ItemValue::Text("Manual".into()),
                observed_at: now,
                provenance,
                purpose: "Service de télécopie, inutilisé sur ce poste.".into(),
                risk: "Surface d'attaque supplémentaire, sans usage.".into(),
                reference: None,
            },
            severity: Severity::Info,
            status,
            first_seen: now,
            last_seen: now,
        }
    }

    #[test]
    fn un_conflit_mdm_nest_jamais_convergeable() {
        let d = drift(
            DriftStatus::Conflict {
                authority: "Intune".into(),
                oscillating: true,
            },
            Provenance::Managed("Intune".into()),
        );
        assert!(!d.is_convergeable(jour("2026-08-17")));
    }

    /// Le jour inscrit dans toutes les tolérances de ce module de test.
    const ECHEANCE: &str = "2026-10-15";

    /// Une date civile littérale, sans horloge.
    fn jour(litteral: &str) -> chrono::NaiveDate {
        litteral.parse().expect("date littérale valide")
    }

    /// Une tolérance dont l'échéance est [`ECHEANCE`].
    fn tolere() -> Drift {
        drift(
            DriftStatus::Accepted {
                reason: "pilote du scanner du labo".into(),
                expires: jour(ECHEANCE),
                decided_by: "tene".into(),
                decided_at: Utc::now(),
            },
            Provenance::Human("tene".into()),
        )
    }

    #[test]
    fn une_acceptation_est_en_vigueur_le_jour_de_son_echeance() {
        // La première des deux bornes. Qui écrit « expires: 2026-10-15 » attend
        // que la tolérance couvre ce jour-là ; l'autre convention lui retirerait
        // une journée sans le dire, et le poste afficherait un écart le matin de
        // l'échéance.
        let d = tolere();
        assert!(!d.is_expired_acceptance(jour(ECHEANCE)));
        assert!(!d.is_convergeable(jour(ECHEANCE)));
        assert!(!d.is_convergeable(jour("2026-10-14")));
    }

    #[test]
    fn une_acceptation_expire_le_lendemain() {
        // La seconde borne, et c'est elle qui rend la convention vérifiable :
        // une inclusivité éprouvée d'un seul côté se retourne au premier
        // passage, puisque « en vigueur la veille » est vrai des deux
        // conventions.
        let d = tolere();
        assert!(d.is_expired_acceptance(jour("2026-10-16")));
        assert!(d.is_convergeable(jour("2026-10-16")));
    }

    #[test]
    fn une_acceptation_valide_est_respectee() {
        let d = tolere();
        assert!(!d.is_convergeable(jour("2026-08-17")));
    }

    #[test]
    fn un_ecart_sans_auteur_est_un_signal_de_securite() {
        let d = drift(DriftStatus::Active, Provenance::Unknown);
        assert!(d.is_security_signal());

        let d = drift(DriftStatus::Active, Provenance::WindowsUpdate);
        assert!(!d.is_security_signal());
    }

    #[test]
    fn le_resume_compte_les_expirations_comme_actives() {
        let aujourd_hui = jour("2026-08-17");
        let drifts = vec![
            drift(DriftStatus::Active, Provenance::Unknown),
            drift(
                DriftStatus::Accepted {
                    reason: "r".into(),
                    expires: jour("2026-08-16"), // la veille : échue
                    decided_by: "tene".into(),
                    decided_at: Utc::now(),
                },
                Provenance::Human("tene".into()),
            ),
            drift(
                DriftStatus::Accepted {
                    reason: "r".into(),
                    expires: aujourd_hui, // le jour même : encore en vigueur
                    decided_by: "tene".into(),
                    decided_at: Utc::now(),
                },
                Provenance::Human("tene".into()),
            ),
        ];
        let s = DriftSummary::build(312, 0, &drifts, aujourd_hui);
        assert_eq!(s.active, 2, "l'acceptation expirée doit redevenir active");
        assert_eq!(s.accepted, 1);
        assert_eq!(s.unattributed, 1);
        assert_eq!(s.compliant, 309);
    }

    #[test]
    fn un_item_illisible_ne_compte_jamais_pour_conforme() {
        // `compliant` se calculait par soustraction, donc absorbait tout ce qui
        // n'était pas un écart déclaré — y compris une exclusion Defender posée
        // sous une clé qu'on ne sait pas lire. L'outil affichait vert sur ce
        // qu'il n'avait pas regardé, ce qui est le pire des trois états.
        let s = DriftSummary::build(115, 3, &[], jour("2026-08-17"));

        assert_eq!(s.unreadable, 3);
        assert_eq!(
            s.compliant, 112,
            "les trois illisibles sortent du décompte des conformes"
        );
        assert_eq!(
            s.compliant + s.unreadable + s.active + s.accepted + s.conflicts,
            s.observed,
            "les cinq catégories doivent couvrir exactement les items observés"
        );
    }
}

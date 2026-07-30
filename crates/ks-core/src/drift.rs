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
        /// Quand la tolérance expire et l'écart redevient actif.
        expires: Timestamp,
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
    #[must_use]
    pub fn is_convergeable(&self, now: Timestamp) -> bool {
        match &self.status {
            DriftStatus::Active => true,
            DriftStatus::Conflict { .. } => false,
            DriftStatus::Accepted { expires, .. } => *expires <= now,
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
    #[must_use]
    pub fn is_expired_acceptance(&self, now: Timestamp) -> bool {
        matches!(&self.status, DriftStatus::Accepted { expires, .. } if *expires <= now)
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
}

impl DriftSummary {
    /// Construit le résumé depuis une liste d'écarts et un total d'items observés.
    #[must_use]
    pub fn build(items_observed: usize, drifts: &[Drift], now: Timestamp) -> Self {
        let mut s = Self {
            observed: items_observed,
            ..Self::default()
        };
        for d in drifts {
            match &d.status {
                DriftStatus::Active => s.active += 1,
                DriftStatus::Conflict { .. } => s.conflicts += 1,
                DriftStatus::Accepted { expires, .. } => {
                    if *expires <= now {
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
        s.compliant = items_observed.saturating_sub(s.active + s.accepted + s.conflicts);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Domain, ItemValue, Provenance};
    use chrono::{Duration, Utc};

    fn drift(status: DriftStatus, provenance: Provenance) -> Drift {
        let now = Utc::now();
        Drift {
            item: Item {
                path: "services.Fax.startupType".into(),
                domain: Domain::Configuration,
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
        assert!(!d.is_convergeable(Utc::now()));
    }

    #[test]
    fn une_acceptation_expiree_redevient_convergeable() {
        let past = Utc::now() - Duration::days(1);
        let d = drift(
            DriftStatus::Accepted {
                reason: "driver du scanner du labo".into(),
                expires: past,
                decided_by: "tene".into(),
                decided_at: past,
            },
            Provenance::Human("tene".into()),
        );
        assert!(d.is_expired_acceptance(Utc::now()));
        assert!(d.is_convergeable(Utc::now()));
    }

    #[test]
    fn une_acceptation_valide_est_respectee() {
        let future = Utc::now() + Duration::days(30);
        let d = drift(
            DriftStatus::Accepted {
                reason: "driver du scanner du labo".into(),
                expires: future,
                decided_by: "tene".into(),
                decided_at: Utc::now(),
            },
            Provenance::Human("tene".into()),
        );
        assert!(!d.is_convergeable(Utc::now()));
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
        let now = Utc::now();
        let drifts = vec![
            drift(DriftStatus::Active, Provenance::Unknown),
            drift(
                DriftStatus::Accepted {
                    reason: "r".into(),
                    expires: now - Duration::days(1),
                    decided_by: "tene".into(),
                    decided_at: now,
                },
                Provenance::Human("tene".into()),
            ),
            drift(
                DriftStatus::Accepted {
                    reason: "r".into(),
                    expires: now + Duration::days(1),
                    decided_by: "tene".into(),
                    decided_at: now,
                },
                Provenance::Human("tene".into()),
            ),
        ];
        let s = DriftSummary::build(312, &drifts, now);
        assert_eq!(s.active, 2, "l'acceptation expirée doit redevenir active");
        assert_eq!(s.accepted, 1);
        assert_eq!(s.unattributed, 1);
        assert_eq!(s.compliant, 309);
    }
}

//! Le plan : ce que Keystone s'apprête à faire, montré avant d'être fait.
//!
//! C'est ici que vivent les deux invariants les plus importants du projet, et ils
//! sont portés par le **typage**, pas par la relecture :
//!
//! * P2 — une action qui ne sait pas se simuler ne peut pas être appliquée ;
//! * P3 — une action qui ne sait pas s'annuler ne peut pas être automatisée.

use serde::{Deserialize, Serialize};

use crate::{Error, Result, SnapshotKind, Timestamp};

/// Ce qu'un exécuteur déclare savoir faire.
///
/// Le broker vérifie ces capacités **au chargement du module**, pas à l'exécution.
/// Un module qui ne déclare pas `dry_run` est rejeté avant d'avoir pu agir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Le module sait calculer et montrer son diff sans rien écrire.
    pub dry_run: bool,
    /// Le module sait défaire ce qu'il a fait, ou s'appuie sur un instantané qui le sait.
    pub rollback: bool,
    /// Le module est idempotent : deux applications produisent le même état (D2-09).
    pub idempotent: bool,
}

impl Capabilities {
    /// Un module en lecture seule : c'est tout ce dont la Phase 0 a besoin.
    #[must_use]
    pub const fn read_only() -> Self {
        Self {
            dry_run: true,
            rollback: true,
            idempotent: true,
        }
    }
}

/// Une opération élémentaire, décrite avant d'être exécutée.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Verbe typé et énuméré. **Jamais une commande libre** (exigence SEC-02) :
    /// le broker n'expose aucune primitive d'exécution arbitraire.
    pub verb: String,
    /// Chemin de l'item visé.
    pub target: String,
    /// Ce que l'action va changer, en une phrase lisible.
    pub summary: String,
    /// Ce que le module déclare savoir faire.
    pub capabilities: Capabilities,
    /// L'action impose-t-elle un redémarrage ? Ils sont regroupés (D3-05).
    pub requires_reboot: bool,
    /// L'action exige-t-elle une confirmation Windows Hello (SEC-08) ?
    pub requires_presence: bool,
    /// Durée estimée, en secondes.
    pub estimated_seconds: u32,
}

impl Action {
    /// Contrôle d'admission avant application. Renvoie une erreur plutôt que
    /// d'appliquer quelque chose qu'on ne saurait pas montrer d'abord.
    pub fn ensure_appliable(&self) -> Result<()> {
        if !self.capabilities.dry_run {
            return Err(Error::DryRunUnsupported {
                verb: self.verb.clone(),
            });
        }
        Ok(())
    }

    /// Contrôle d'admission avant planification automatique.
    ///
    /// C'est la traduction en code du principe P3 : *si je ne sais pas l'annuler,
    /// je ne l'automatise pas*.
    pub fn ensure_schedulable(&self) -> Result<()> {
        self.ensure_appliable()?;
        if !self.capabilities.rollback {
            return Err(Error::RollbackUnsupported {
                verb: self.verb.clone(),
            });
        }
        Ok(())
    }
}

/// Une vague : un groupe d'actions appliquées ensemble, puis jugées.
///
/// Exigence D3-08 : deux composants couplés ne sont jamais dans la même vague.
/// Sans cette règle, quand ça casse on ne sait pas *qui* a cassé.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    /// Numéro de vague, à partir de 1.
    pub index: u8,
    /// Intitulé lisible, ex. « Sécurité — couloir prioritaire ».
    pub title: String,
    /// Actions de la vague.
    pub actions: Vec<Action>,
    /// Instantanés à prendre **avant** la vague (exigence D3-07).
    pub snapshots: Vec<SnapshotKind>,
    /// Identifiants des tests de fumée qui jugeront le résultat (exigence D3-09).
    pub smoke_tests: Vec<String>,
}

/// Un plan complet, tel que présenté à l'utilisateur avant toute écriture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Identifiant du plan, pour le journal et le rollback.
    pub id: String,
    /// Portée : `updates`, `converge`, `space`…
    pub scope: String,
    /// Vagues ordonnées.
    pub waves: Vec<Wave>,
    /// Fenêtre d'application prévue. `None` = à la demande.
    pub window: Option<(Timestamp, Timestamp)>,
    /// Le plan a-t-il déjà été simulé ? **`apply` est refusé tant que c'est faux.**
    pub simulated: bool,
}

impl Plan {
    /// Nombre total d'actions.
    #[must_use]
    pub fn action_count(&self) -> usize {
        self.waves.iter().map(|w| w.actions.len()).sum()
    }

    /// Durée estimée totale, en secondes.
    #[must_use]
    pub fn estimated_seconds(&self) -> u32 {
        self.waves
            .iter()
            .flat_map(|w| &w.actions)
            .map(|a| a.estimated_seconds)
            .sum()
    }

    /// Un seul redémarrage pour tout le plan, jamais un par action (exigence D3-05).
    #[must_use]
    pub fn reboot_count(&self) -> u8 {
        u8::from(
            self.waves
                .iter()
                .flat_map(|w| &w.actions)
                .any(|a| a.requires_reboot),
        )
    }

    /// Contrôle d'admission du plan entier avant application.
    ///
    /// Deux refus possibles, tous deux volontaires :
    /// une action non simulable, ou un plan qui n'a pas encore été simulé.
    pub fn ensure_appliable(&self) -> Result<()> {
        for wave in &self.waves {
            for action in &wave.actions {
                action.ensure_appliable()?;
            }
        }
        if !self.simulated {
            return Err(Error::System {
                message: "Ce plan n'a pas encore été simulé. Simulez-le d'abord.".into(),
                detail: Some("P2 — la simulation est le comportement par défaut".into()),
            });
        }
        Ok(())
    }

    /// Toute vague qui écrit doit être précédée d'au moins un instantané (P3).
    #[must_use]
    pub fn waves_without_safety_net(&self) -> Vec<u8> {
        self.waves
            .iter()
            .filter(|w| !w.actions.is_empty() && w.snapshots.is_empty())
            .map(|w| w.index)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(dry_run: bool, rollback: bool) -> Action {
        Action {
            verb: "set-service-startup".into(),
            target: "services.Fax.startupType".into(),
            summary: "Passer le service Fax en Disabled.".into(),
            capabilities: Capabilities {
                dry_run,
                rollback,
                idempotent: true,
            },
            requires_reboot: false,
            requires_presence: false,
            estimated_seconds: 2,
        }
    }

    fn plan(actions: Vec<Action>, simulated: bool, snapshots: Vec<SnapshotKind>) -> Plan {
        Plan {
            id: "plan-0001".into(),
            scope: "converge".into(),
            waves: vec![Wave {
                index: 1,
                title: "Configuration".into(),
                actions,
                snapshots,
                smoke_tests: vec!["build-ref".into()],
            }],
            window: None,
            simulated,
        }
    }

    #[test]
    fn une_action_non_simulable_est_refusee() {
        assert!(action(false, true).ensure_appliable().is_err());
    }

    #[test]
    fn une_action_sans_rollback_nest_pas_planifiable() {
        let a = action(true, false);
        assert!(
            a.ensure_appliable().is_ok(),
            "elle reste applicable à la main"
        );
        assert!(
            a.ensure_schedulable().is_err(),
            "mais jamais par l'ordonnanceur automatique"
        );
    }

    #[test]
    fn un_plan_non_simule_est_refuse() {
        let p = plan(
            vec![action(true, true)],
            false,
            vec![SnapshotKind::SystemRestorePoint],
        );
        assert!(p.ensure_appliable().is_err());
    }

    #[test]
    fn un_plan_simule_et_capable_est_accepte() {
        let p = plan(
            vec![action(true, true)],
            true,
            vec![SnapshotKind::SystemRestorePoint],
        );
        assert!(p.ensure_appliable().is_ok());
    }

    #[test]
    fn une_vague_sans_instantane_est_signalee() {
        let p = plan(vec![action(true, true)], true, vec![]);
        assert_eq!(p.waves_without_safety_net(), vec![1]);
    }

    #[test]
    fn les_redemarrages_sont_regroupes() {
        let mut a1 = action(true, true);
        let mut a2 = action(true, true);
        a1.requires_reboot = true;
        a2.requires_reboot = true;
        let p = plan(vec![a1, a2], true, vec![SnapshotKind::SystemRestorePoint]);
        assert_eq!(p.reboot_count(), 1, "deux actions, un seul redémarrage");
    }
}

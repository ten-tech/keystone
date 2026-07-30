//! Le journal — inaltérable, chaîné, expédié hors machine.
//!
//! C'est le composant qui permet de contredire une machine compromise.
//!
//! Un agent qui tourne sur la machine qu'il surveille ne peut pas être cru quand il
//! la déclare saine : un attaquant qui a SYSTEM peut réécrire le journal local. Il ne
//! peut pas réécrire ce qui est **déjà parti** vers une destination en écriture seule.
//!
//! D'où deux propriétés non négociables (SEC-03 à SEC-05) :
//!
//! * chaque entrée porte l'empreinte de la précédente — une suppression se voit ;
//! * un battement de cœur est attendu côté puits externe, et **son absence est une
//!   alerte** : le silence de l'agent est un signal, pas une absence de signal.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Qui a demandé l'action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "actor", content = "name")]
pub enum Actor {
    /// Un humain, via la CLI ou l'interface.
    Human(String),
    /// L'ordonnanceur de Keystone.
    Scheduler,
    /// Le copilote local — qui ne peut que *proposer* (exigence D15-03).
    Copilot,
    /// Le système, lors d'un démarrage ou d'un contrôle d'intégrité.
    System,
}

/// Résultat d'une action journalisée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Simulée seulement : aucune écriture n'a eu lieu.
    Simulated,
    /// Appliquée avec succès.
    Applied,
    /// Refusée par un contrôle d'admission (capacité manquante, conflit MDM…).
    Refused {
        /// Motif du refus, lisible par un humain.
        reason: String,
    },
    /// Échouée, puis annulée automatiquement.
    RolledBack {
        /// Identifiant du test de fumée qui a déclenché le retour arrière.
        failed_test: String,
    },
    /// Échouée sans possibilité d'annulation — cas qui doit rester impossible par
    /// construction (P3) ; s'il survient, c'est un défaut à traiter en priorité.
    Failed {
        /// Trace technique, à replier derrière « Détails » dans l'interface.
        detail: String,
    },
}

/// Une entrée de journal. Écrite une fois, jamais modifiée.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Numéro de séquence monotone.
    pub seq: u64,
    /// Horodatage UTC. La cohérence inter-systèmes est une exigence (D1-09) :
    /// sans elle, la corrélation forensique ne vaut rien.
    pub at: Timestamp,
    /// Demandeur.
    pub actor: Actor,
    /// Verbe typé exécuté.
    pub verb: String,
    /// Cible.
    pub target: String,
    /// Diff simulé, sérialisé. Conservé même en cas de refus : c'est la trace de
    /// ce qui *aurait* été fait.
    pub diff: Option<String>,
    /// Résultat.
    pub outcome: Outcome,
    /// Empreinte de l'entrée précédente — le chaînage qui rend la troncature visible.
    pub prev_digest: String,
}

impl JournalEntry {
    /// Empreinte de cette entrée, à reporter dans `prev_digest` de la suivante.
    ///
    /// # Note d'implémentation
    ///
    /// L'implémentation ci-dessous est un **bouchon volontaire** : elle n'est pas
    /// cryptographique. Le remplacement par BLAKE3 ou SHA-256 est la première tâche
    /// de la Phase 1 (voir `docs/07-FEUILLE-DE-ROUTE.md`). Elle est laissée en
    /// évidence plutôt que masquée : un faux chaînage qui *a l'air* solide serait
    /// plus dangereux qu'un bouchon annoncé.
    #[must_use]
    pub fn digest(&self) -> String {
        let material = format!(
            "{}|{}|{:?}|{}|{}|{}",
            self.seq,
            self.at.to_rfc3339(),
            self.actor,
            self.verb,
            self.target,
            self.prev_digest
        );
        let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a — NON cryptographique
        for b in material.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("fnv1a-stub:{h:016x}")
    }

    /// Le chaînage est-il intact sur toute la séquence ?
    #[must_use]
    pub fn verify_chain(entries: &[Self]) -> bool {
        entries.windows(2).all(|pair| {
            let (a, b) = (&pair[0], &pair[1]);
            b.seq == a.seq + 1 && b.prev_digest == a.digest()
        })
    }
}

/// Battement de cœur attendu par le puits externe.
///
/// L'absence de battement **est** l'alerte (SEC-05).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Dernier battement reçu.
    pub last: Timestamp,
    /// Période attendue, en secondes.
    pub period_seconds: u32,
}

impl Heartbeat {
    /// Le silence a-t-il dépassé le seuil tolérable (trois périodes) ?
    #[must_use]
    pub fn is_silent(&self, now: Timestamp) -> bool {
        let tolerance = i64::from(self.period_seconds) * 3;
        (now - self.last).num_seconds() > tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn entry(seq: u64, prev: &str) -> JournalEntry {
        JournalEntry {
            seq,
            at: Utc::now(),
            actor: Actor::Human("tene".into()),
            verb: "scan".into(),
            target: "inventory".into(),
            diff: None,
            outcome: Outcome::Simulated,
            prev_digest: prev.into(),
        }
    }

    #[test]
    fn le_chainage_detecte_une_entree_manquante() {
        let e1 = entry(1, "genesis");
        let e2 = entry(2, &e1.digest());
        assert!(JournalEntry::verify_chain(&[e1.clone(), e2.clone()]));

        // On saute une entrée : la chaîne doit casser.
        let e4 = entry(4, &e2.digest());
        assert!(!JournalEntry::verify_chain(&[e1, e2, e4]));
    }

    #[test]
    fn le_silence_de_lagent_est_une_alerte() {
        let hb = Heartbeat {
            last: Utc::now() - Duration::seconds(200),
            period_seconds: 60,
        };
        assert!(
            hb.is_silent(Utc::now()),
            "200 s sans battement sur une période de 60 s : l'agent est muet, \
             et c'est exactement ce qu'on veut voir"
        );

        let hb = Heartbeat {
            last: Utc::now() - Duration::seconds(90),
            period_seconds: 60,
        };
        assert!(
            !hb.is_silent(Utc::now()),
            "un retard d'une période reste toléré"
        );
    }
}

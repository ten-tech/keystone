//! # ks-core — le vocabulaire de Keystone
//!
//! Ce crate ne fait *rien*. Il nomme les choses.
//!
//! Tout le reste du projet dépend de lui, et lui ne dépend de rien de spécifique
//! à une plateforme : il compile sur Windows, Linux et macOS à l'identique. C'est
//! volontaire — le modèle de données doit être testable sans machine à administrer.
//!
//! Les types correspondent un pour un au §9.2 du cahier des charges.
//!
//! ## Les deux invariants portés par le typage
//!
//! Ils viennent des principes P2 et P3 du projet, et le compilateur doit les faire
//! respecter plutôt que la relecture humaine :
//!
//! * une [`Action`] qui ne sait pas se simuler ne peut pas être appliquée ;
//! * une [`Action`] qui ne sait pas s'annuler ne peut pas être planifiée automatiquement.

#![forbid(unsafe_code)]

pub mod drift;
pub mod item;
pub mod journal;
pub mod plan;
pub mod snapshot;

pub use drift::{Drift, DriftStatus, DriftSummary, Severity};
pub use item::{Domain, Item, ItemValue, Provenance};
pub use journal::Heartbeat;
pub use journal::{Actor, JournalEntry, Outcome};
pub use plan::{Action, Capabilities, Plan, Wave};
pub use snapshot::{BackupSet, Snapshot, SnapshotKind};

/// Erreur du domaine Keystone.
///
/// Volontairement courte : une erreur qui ne peut pas être expliquée à
/// l'utilisateur en une phrase n'a pas sa place ici. Le code technique
/// (`0x80070005` et compagnie) vit dans `detail`, jamais dans le message.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Une action a été demandée en mode application alors qu'elle ne sait pas se simuler.
    #[error("l'action « {verb} » ne sait pas se simuler : elle ne peut pas être appliquée")]
    DryRunUnsupported {
        /// Verbe concerné.
        verb: String,
    },

    /// Une action sans retour arrière a été soumise à l'ordonnanceur automatique.
    #[error("l'action « {verb} » n'a pas de retour arrière : elle ne peut pas être automatisée")]
    RollbackUnsupported {
        /// Verbe concerné.
        verb: String,
    },

    /// Un item du fichier d'état désiré contredit une politique gérée par la MDM.
    #[error("« {path} » est piloté par {authority} : conflit, non convergeable")]
    ManagedConflict {
        /// Chemin de l'item.
        path: String,
        /// Autorité souveraine (Intune, GPO, Configuration Manager…).
        authority: String,
    },

    /// Échec d'une opération système, avec son code technique replié.
    #[error("{message}")]
    System {
        /// Phrase lisible par un humain.
        message: String,
        /// Code ou trace technique, affiché seulement derrière « Détails ».
        detail: Option<String>,
    },

    /// Erreur de sérialisation.
    #[error("données illisibles")]
    Serde(#[from] serde_json::Error),
}

/// Résultat conventionnel du projet.
pub type Result<T> = std::result::Result<T, Error>;

/// Horodatage utilisé partout dans le projet.
///
/// **Toujours en UTC avec décalage explicite.** La valeur forensique du journal
/// dépend de la cohérence des horloges entre l'hôte, les distros WSL et les VM
/// (exigence D1-09) : un horodatage sans fuseau est un horodatage inutilisable.
pub type Timestamp = chrono::DateTime<chrono::Utc>;

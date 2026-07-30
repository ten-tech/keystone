//! Le filet de sécurité — et la distinction qui a failli manquer au projet.
//!
//! **Un instantané n'est pas une sauvegarde.**
//!
//! Un instantané protège la *machine* : il permet de revenir à l'état d'avant une
//! opération. Une sauvegarde protège le *travail* : elle permet de retrouver des
//! données perdues. Confondre les deux est l'erreur classique — et c'est exactement
//! l'angle mort qu'a comblé le domaine D6 du cahier des charges.
//!
//! Les deux types sont donc distincts dans le code, et l'interface ne les mélange
//! jamais dans la même liste.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Nature technique d'un point de retour arrière.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotKind {
    /// Point de restauration système Windows.
    SystemRestorePoint,
    /// Point de contrôle Hyper-V sur une VM nommée.
    HyperVCheckpoint(String),
    /// Export complet d'une distribution WSL2 (`wsl --export`).
    WslExport(String),
    /// Export d'une branche de registre.
    RegistryExport(String),
    /// Copie d'un fichier de configuration avant modification.
    FileCopy(String),
    /// Zone de quarantaine, pour ce qui aurait été supprimé (exigence D4-04).
    Quarantine {
        /// Durée de vie avant purge automatique, en jours (30 par défaut).
        ttl_days: u16,
    },
}

impl SnapshotKind {
    /// Durée de retour arrière typique, en secondes. Sert à annoncer le coût
    /// **avant** l'opération : « retour arrière estimé à 3 min 20 s ».
    #[must_use]
    pub const fn typical_rollback_seconds(&self) -> u32 {
        match self {
            Self::HyperVCheckpoint(_) => 20,
            Self::WslExport(_) => 200,
            Self::RegistryExport(_) | Self::FileCopy(_) => 5,
            Self::SystemRestorePoint => 600,
            Self::Quarantine { .. } => 10,
        }
    }

    /// Libellé lisible, pour le bandeau du filet de sécurité.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::SystemRestorePoint => "point de restauration système".into(),
            Self::HyperVCheckpoint(vm) => format!("point de contrôle Hyper-V sur {vm}"),
            Self::WslExport(d) => format!("export de la distro {d}"),
            Self::RegistryExport(k) => format!("export de la branche {k}"),
            Self::FileCopy(p) => format!("copie de {p}"),
            Self::Quarantine { ttl_days } => format!("quarantaine {ttl_days} jours"),
        }
    }
}

/// Un point de retour arrière effectivement pris.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Identifiant, référencé par `ks rollback <id>`.
    pub id: String,
    /// Nature du point.
    pub kind: SnapshotKind,
    /// Plan qui a motivé sa prise.
    pub plan_id: Option<String>,
    /// Quand il a été pris.
    pub taken_at: Timestamp,
    /// Taille sur disque, en octets.
    pub size_bytes: u64,
    /// Quand il sera purgé automatiquement.
    pub expires_at: Option<Timestamp>,
    /// Empreinte, pour vérifier qu'il n'a pas été altéré.
    pub digest: String,
}

/// Une sauvegarde de données de travail — **catégorie distincte** (domaine D6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSet {
    /// Identifiant.
    pub id: String,
    /// Chemin sauvegardé.
    pub source: String,
    /// Nombre de copies existantes (règle 3-2-1 : on en veut trois).
    pub copies: u8,
    /// Une copie est-elle hors site ?
    pub offsite: bool,
    /// Dernière sauvegarde réussie.
    pub last_backup: Timestamp,
    /// **Dernière restauration vérifiée.** `None` est une alerte, pas un détail :
    /// une sauvegarde jamais restaurée n'est pas une sauvegarde (exigence D6-03).
    pub last_verified_restore: Option<Timestamp>,
}

impl BackupSet {
    /// La règle 3-2-1 est-elle satisfaite ?
    #[must_use]
    pub const fn satisfies_321(&self) -> bool {
        self.copies >= 3 && self.offsite
    }

    /// La sauvegarde est-elle digne de confiance ?
    ///
    /// Non tant qu'aucune restauration n'a été vérifiée. C'est délibérément sévère.
    #[must_use]
    pub const fn is_trustworthy(&self) -> bool {
        self.last_verified_restore.is_some() && self.satisfies_321()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn une_sauvegarde_jamais_restauree_nest_pas_digne_de_confiance() {
        let b = BackupSet {
            id: "src".into(),
            source: r"D:\src".into(),
            copies: 3,
            offsite: true,
            last_backup: Utc::now(),
            last_verified_restore: None,
        };
        assert!(b.satisfies_321());
        assert!(
            !b.is_trustworthy(),
            "3-2-1 ne suffit pas : il faut avoir restauré au moins une fois"
        );
    }

    #[test]
    fn le_cout_du_retour_arriere_est_annonce_a_lavance() {
        assert_eq!(
            SnapshotKind::HyperVCheckpoint("ks-lab".into()).typical_rollback_seconds(),
            20
        );
        assert!(
            SnapshotKind::SystemRestorePoint.typical_rollback_seconds()
                > SnapshotKind::HyperVCheckpoint("x".into()).typical_rollback_seconds(),
            "revenir en arrière sur l'hôte coûte bien plus cher que sur une VM — \
             c'est pourquoi on automatise davantage dans les VM (D3-15)"
        );
    }
}

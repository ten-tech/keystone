//! L'unité de configuration : ce que Keystone observe et, plus tard, fait converger.

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Domaine fonctionnel d'un item, aligné sur le §7 du cahier des charges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Domain {
    /// D1 — matériel, firmware, plateforme, chiffrement.
    Inventory,
    /// D2 — services, tâches, registre, politiques locales.
    Configuration,
    /// D3 — applications, pilotes, paquets, chaînes d'outils.
    Updates,
    /// D4 — occupation et propreté du stockage.
    Space,
    /// D5 — posture de sécurité et surveillance de changement.
    Security,
    /// D6 — sauvegarde et restauration des données de travail.
    Backup,
    /// D7 — secrets, clés, certificats, jetons.
    Identity,
    /// D8 — profils, alimentation, thermique.
    Profiles,
    /// D9 — distributions WSL2 et machines virtuelles.
    Virtualization,
    /// D10 — réseau, docks, périphériques.
    Peripherals,
    /// D11 — environnement de développement.
    DevEnv,
}

/// D'où vient l'information — ou, pour un changement, **qui l'a fait**.
///
/// C'est le champ le plus important du modèle. Une dérive dont l'auteur est
/// connu est une information ; une dérive [`Provenance::Unknown`] est un signal
/// de sécurité, et elle est remontée comme telle au domaine [`Domain::Security`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "who")]
pub enum Provenance {
    /// Keystone lui-même, lors d'une convergence.
    Keystone,
    /// Une décision humaine explicite, horodatée.
    Human(String),
    /// Windows Update.
    WindowsUpdate,
    /// Une autorité de gestion souveraine : Intune, GPO, Configuration Manager.
    Managed(String),
    /// L'installeur ou l'auto-mise-à-jour d'une application.
    Application(String),
    /// **Personne d'identifiable.** Le vrai signal.
    Unknown,
}

impl Provenance {
    /// Un changement sans auteur identifiable doit remonter en constat de sécurité.
    #[must_use]
    pub const fn is_security_signal(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Une politique gérée est souveraine : Keystone ne l'écrase jamais (principe P10).
    #[must_use]
    pub const fn is_sovereign(&self) -> bool {
        matches!(self, Self::Managed(_))
    }
}

/// Valeur d'un item. Volontairement pauvre : on compare, on affiche, on ne calcule pas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemValue {
    /// Absence de valeur — l'item n'existe pas sur la machine.
    Absent,
    /// Valeur booléenne (activé / désactivé).
    Bool(bool),
    /// Valeur numérique entière.
    Int(i64),
    /// Chaîne (version, chemin, nom de mode…).
    Text(String),
    /// Liste de chaînes (exclusions, règles, membres d'un groupe…).
    List(Vec<String>),
}

impl std::fmt::Display for ItemValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => f.write_str("absent"),
            Self::Bool(true) => f.write_str("activé"),
            Self::Bool(false) => f.write_str("désactivé"),
            Self::Int(n) => write!(f, "{n}"),
            Self::Text(s) => f.write_str(s),
            Self::List(v) => write!(f, "{} élément(s)", v.len()),
        }
    }
}

/// Une unité de configuration observée, et éventuellement désirée.
///
/// Exigence D1-10 : **un item sans provenance ni date d'observation n'est jamais
/// affiché.** C'est pour cela que ces deux champs ne sont pas optionnels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    /// Chemin canonique et stable, ex. `security.defender.realtime`.
    pub path: String,
    /// Domaine fonctionnel.
    pub domain: Domain,
    /// Valeur déclarée dans `workstation.yaml`. `None` = non déclarée, donc non contrainte.
    pub desired: Option<ItemValue>,
    /// Valeur réellement constatée sur la machine.
    pub observed: ItemValue,
    /// Quand cette observation a été faite.
    pub observed_at: Timestamp,
    /// Qui a produit la valeur constatée, autant qu'on puisse le savoir.
    pub provenance: Provenance,
    /// À quoi sert cet item, en une phrase lisible (principe P6 — explicabilité).
    pub purpose: String,
    /// Ce qu'on risque en le changeant, en une phrase (principe P6).
    pub risk: String,
    /// Référence : baseline Microsoft, CIS, documentation constructeur…
    pub reference: Option<String>,
}

impl Item {
    /// L'item est-il en écart avec ce qui est désiré ?
    ///
    /// Un item non déclaré n'est jamais en écart : Keystone ne contraint que ce
    /// qui est écrit dans le fichier d'état désiré.
    #[must_use]
    pub fn is_drifted(&self) -> bool {
        match &self.desired {
            None => false,
            Some(want) => want != &self.observed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(desired: Option<ItemValue>, observed: ItemValue) -> Item {
        Item {
            path: "security.defender.realtime".into(),
            domain: Domain::Security,
            desired,
            observed,
            observed_at: chrono::Utc::now(),
            provenance: Provenance::Unknown,
            purpose: "Analyse en temps réel des fichiers ouverts.".into(),
            risk: "Désactivée, les menaces connues ne sont plus bloquées à l'exécution.".into(),
            reference: Some("Microsoft Security Baseline".into()),
        }
    }

    #[test]
    fn un_item_non_declare_nest_jamais_en_ecart() {
        assert!(!item(None, ItemValue::Bool(false)).is_drifted());
    }

    #[test]
    fn un_item_declare_et_different_est_en_ecart() {
        let i = item(Some(ItemValue::Bool(true)), ItemValue::Bool(false));
        assert!(i.is_drifted());
    }

    #[test]
    fn un_item_conforme_nest_pas_en_ecart() {
        let i = item(Some(ItemValue::Bool(true)), ItemValue::Bool(true));
        assert!(!i.is_drifted());
    }

    #[test]
    fn une_provenance_inconnue_est_un_signal_de_securite() {
        assert!(Provenance::Unknown.is_security_signal());
        assert!(!Provenance::WindowsUpdate.is_security_signal());
        assert!(Provenance::Managed("Intune".into()).is_sovereign());
    }
}

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
    /// Keystone lui-même, lors d'une convergence — il a **écrit** cette valeur.
    Keystone,
    /// Simplement **relevé** par un collecteur, sans prétention sur l'auteur.
    ///
    /// À distinguer soigneusement des deux voisines :
    ///
    /// * [`Provenance::Keystone`] affirme que Keystone a *produit* la valeur ;
    /// * [`Provenance::Unknown`] affirme qu'un *changement* a eu lieu sans auteur
    ///   identifiable — c'est un signal de sécurité.
    ///
    /// Un relevé n'est ni l'un ni l'autre : il ne dit rien de l'origine, et ne doit
    /// donc rien déclencher. Sans cette variante, les collecteurs marquaient tout en
    /// `Keystone`, ce qui rendait `Unknown` inatteignable en Phase 0 — donc
    /// `is_security_signal` toujours faux, et le mécanisme le plus valorisé du
    /// modèle structurellement mort.
    Observed,
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
    /// **La lecture a échoué.** Ni une valeur, ni une absence.
    ///
    /// La distinction n'est pas une subtilité de modélisation, c'est le cœur du
    /// sujet. Les exclusions de Defender se lisent dans une clé protégée par ACL :
    /// une CLI non élevée se voit refuser l'accès, et confondre ce refus avec une
    /// liste vide afficherait « 0 exclusion » sur une machine où un attaquant vient
    /// d'en poser une. L'outil dirait « il n'y a rien » là où il ne sait que
    /// « je n'ai pas pu regarder ».
    ///
    /// Variante de forme **objet** à dessein : la sérialisation est `untagged`,
    /// donc une variante unitaire se confondrait avec [`Self::Absent`] et une
    /// variante à chaîne avec [`Self::Text`]. Un objet ne ressemble à aucune autre.
    Illisible {
        /// Pourquoi la lecture a échoué, en clair pour l'utilisateur.
        raison: String,
    },
}

impl ItemValue {
    /// Raccourci de lisibilité pour le cas le plus fréquent.
    #[must_use]
    pub fn illisible(raison: &str) -> Self {
        Self::Illisible {
            raison: raison.to_owned(),
        }
    }

    /// La valeur est-elle un constat, ou un aveu d'échec ?
    ///
    /// Un item illisible ne doit jamais alimenter un décompte ni un pourcentage :
    /// c'est la même règle que pour l'attribution logicielle, où un gestionnaire
    /// non interrogeable supprime le pourcentage plutôt que de le fausser.
    #[must_use]
    pub const fn est_constat(&self) -> bool {
        !matches!(self, Self::Illisible { .. })
    }
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
            Self::Illisible { raison } => write!(f, "illisible — {raison}"),
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
    /// Ce que vaut la comparaison entre le désiré et le constaté.
    ///
    /// # Pourquoi ce n'est pas un booléen
    ///
    /// `is_drifted()` renvoyait une égalité stricte, et traitait donc un item
    /// **illisible** comme un écart. Ce n'est pas une nuance : sur la machine de
    /// référence, trois items sont illisibles en permanence — les exclusions
    /// Defender, dont la clé est protégée par ACL et que la CLI, non élevée
    /// (SEC-01), ne peut pas lire.
    ///
    /// Dès qu'une politique d'exclusions serait déclarée (D11-02), Keystone
    /// publierait **trois écarts par jour, indéfiniment**, sur une machine
    /// parfaitement saine. Et ces écarts ne diraient pas ce qui est faux : ils
    /// diraient qu'on n'a pas su regarder. Le critère de sortie de la Phase 1 —
    /// « la dérive suivie pendant sept jours sans faux positif inexpliqué » —
    /// en devenait **inatteignable**, pas difficile.
    ///
    /// La règle est donc simple et vaut dans les deux sens : **un côté qui n'est
    /// pas un constat rend la comparaison impossible**, jamais un écart, et
    /// jamais une conformité.
    ///
    /// [`ItemValue::Absent`] reste un constat : « cette clé n'existe pas » est
    /// une mesure, et son écart avec un état désiré est légitime.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let Some(voulu) = &self.desired else {
            return Verdict::NonContraint;
        };
        if !self.observed.est_constat() {
            return Verdict::Incomparable {
                raison: self.observed.to_string(),
            };
        }
        if voulu == &self.observed {
            Verdict::Conforme
        } else {
            Verdict::Ecart
        }
    }
}

/// Ce que donne la confrontation d'un état désiré et d'un état constaté.
///
/// Quatre valeurs, et le `match` sur ce type est **exhaustif sans bras `_`** :
/// ajouter un cas obligera à décider ce qu'on en fait partout, plutôt que de le
/// laisser tomber silencieusement dans un fourre-tout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "verdict")]
pub enum Verdict {
    /// Aucun état désiré : Keystone ne contraint que ce qui est écrit.
    NonContraint,
    /// Le constaté correspond au désiré.
    Conforme,
    /// Le constaté diffère du désiré.
    Ecart,
    /// **La comparaison n'a pas de sens** : la lecture a échoué.
    ///
    /// Ni un écart — on ne sait pas si la machine est conforme — ni une
    /// conformité — on ne sait pas non plus qu'elle l'est. C'est un troisième
    /// état, et le taire serait le mensonge que ce type existe pour empêcher.
    Incomparable {
        /// Ce que la lecture a répondu, en clair.
        raison: String,
    },
}

impl Verdict {
    /// Cet item demande-t-il une action de convergence ?
    ///
    /// `Incomparable` répond **non**, et c'est le cœur de la décision : on ne
    /// converge pas vers un état qu'on n'a pas su lire. L'item réclame une
    /// élévation ou un diagnostic, pas une écriture.
    #[must_use]
    pub const fn demande_convergence(&self) -> bool {
        matches!(self, Self::Ecart)
    }

    /// Peut-on conclure quoi que ce soit sur cet item ?
    #[must_use]
    pub const fn est_concluant(&self) -> bool {
        !matches!(self, Self::Incomparable { .. })
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
        let v = item(None, ItemValue::Bool(false)).verdict();
        assert_eq!(v, Verdict::NonContraint);
        assert!(!v.demande_convergence());
    }

    #[test]
    fn un_item_declare_et_different_est_en_ecart() {
        let v = item(Some(ItemValue::Bool(true)), ItemValue::Bool(false)).verdict();
        assert_eq!(v, Verdict::Ecart);
        assert!(v.demande_convergence());
    }

    #[test]
    fn un_item_conforme_nest_pas_en_ecart() {
        let v = item(Some(ItemValue::Bool(true)), ItemValue::Bool(true)).verdict();
        assert_eq!(v, Verdict::Conforme);
        assert!(!v.demande_convergence());
    }

    #[test]
    fn un_item_illisible_nest_ni_conforme_ni_en_ecart() {
        // **Le défaut qui rendait le critère de sortie de la Phase 1
        // inatteignable.** Trois items sont illisibles en permanence sur la
        // machine de référence : les exclusions Defender, dont la clé est
        // protégée par ACL et que la CLI, non élevée, ne peut pas lire.
        //
        // L'égalité stricte les comptait en écart. Déclarer une politique
        // d'exclusions aurait donc produit trois écarts par jour, indéfiniment,
        // sur une machine saine — et ces écarts n'auraient pas dit ce qui est
        // faux, seulement qu'on n'avait pas su regarder.
        let i = item(
            Some(ItemValue::Bool(true)),
            ItemValue::illisible("accès refusé sans élévation"),
        );
        let v = i.verdict();

        assert!(matches!(v, Verdict::Incomparable { .. }), "{v:?}");
        assert_ne!(v, Verdict::Ecart, "un aveu n'est pas un écart");
        assert_ne!(v, Verdict::Conforme, "et surtout pas une conformité");
        assert!(
            !v.demande_convergence(),
            "on ne converge pas vers un état qu'on n'a pas su lire"
        );
        assert!(!v.est_concluant());

        // La raison remonte jusqu'à l'utilisateur : un item incomparable sans
        // explication serait exactement l'indicateur que le principe P6 refuse.
        let Verdict::Incomparable { raison } = v else {
            unreachable!("le verdict vient d'être vérifié")
        };
        assert!(raison.contains("accès refusé"), "raison perdue : {raison}");
    }

    #[test]
    fn une_absence_reste_un_constat_donc_comparable() {
        // La nuance qui empêche la correction d'aller trop loin. « Cette clé
        // n'existe pas » est une MESURE : son écart avec un état désiré est
        // légitime, et le ranger en incomparable désarmerait la moitié du
        // produit.
        let v = item(Some(ItemValue::Bool(true)), ItemValue::Absent).verdict();
        assert_eq!(v, Verdict::Ecart);
        assert!(v.demande_convergence());
    }

    #[test]
    fn une_provenance_inconnue_est_un_signal_de_securite() {
        assert!(Provenance::Unknown.is_security_signal());
        assert!(!Provenance::WindowsUpdate.is_security_signal());
        assert!(Provenance::Managed("Intune".into()).is_sovereign());
    }
}

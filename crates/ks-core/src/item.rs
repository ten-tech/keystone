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

/// Ce qu'un item a **vocation** à devenir dans le fichier d'état désiré (ADR-0009).
///
/// # Pourquoi une propriété de l'item, et pas une table dans l'importateur
///
/// Les 115 items d'un scan n'ont pas la même vocation, et les traiter à égalité
/// coûte deux fois. `ks import` verserait 115 lignes dans `workstation.yaml`,
/// c'est-à-dire un fichier que personne ne relit ; et le suivi de dérive
/// publierait un écart par scan sur tout ce qui bouge de lui-même — le temps de
/// fonctionnement, le taux d'occupation d'un volume, la date des dernières
/// signatures de Defender.
///
/// Une table de chemins déclarables tenue à part serait une seconde source de
/// vérité : un collecteur qui ajoute un item le verrait **silencieusement**
/// classé non déclarable. La nature vit donc avec l'item, décidée dans le
/// collecteur qui le fabrique, et nulle part ailleurs.
///
/// # Ce que le préfixe du chemin ne dit pas
///
/// La déduire du préfixe est faux dès la première ligne :
/// `security.firmware.version` est un [`Nature::Constat`] — on subit la version
/// du firmware — tandis que `virtualization.wsl[*].interop` est un
/// [`Nature::Reglage`]. Il n'existe pas de raccourci ; il n'y a qu'une décision
/// par item.
///
/// # Volontairement pas `#[non_exhaustive]`
///
/// L'énumération est fermée pour que le `match` **exhaustif sans bras `_`**
/// reste possible chez ses consommateurs, y compris hors de ce crate. Ajouter
/// une variante doit casser la compilation partout où l'on décide quelque chose
/// d'une nature, plutôt que de tomber dans un fourre-tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Nature {
    /// Un réglage : il a un état désirable, et un verbe du broker l'écrira.
    Reglage,
    /// Un état qu'on peut vouloir vrai, qu'aucun verbe n'écrit directement.
    ///
    /// Déclarable, suivi, mais **jamais convergeable**. C'est la position
    /// honnête sur Secure Boot, la protection DMA et l'exécution effective de
    /// VBS : on peut les vouloir, les mesurer et les signaler sans savoir agir.
    Objectif,
    /// Un nombre qui évolue de lui-même. Ni déclaré ni suivi en Phase 1.
    ///
    /// Faute d'un vocabulaire de contrainte (« au plus 85 », « plus récent que
    /// sept jours »), une mesure déclarée le serait par égalité — donc en écart
    /// à chaque scan. Ce vocabulaire s'écrira le jour où un second cas d'usage
    /// apparaîtra, et pas avant (ADR-0009).
    Mesure,
    /// Un fait sur la machine. Ne se déclare jamais.
    Constat,
}

impl Nature {
    /// L'item a-t-il vocation à figurer dans le fichier d'état désiré ?
    ///
    /// C'est ce que `ks import` retiendra du scan, et rien d'autre.
    #[must_use]
    pub const fn est_declarable(self) -> bool {
        match self {
            Self::Reglage | Self::Objectif => true,
            Self::Mesure | Self::Constat => false,
        }
    }

    /// Keystone saura-t-il un jour **écrire** cet item ?
    ///
    /// À terme, la Phase 2 refusera de faire converger un [`Nature::Objectif`] :
    /// on ne prétend pas écrire ce qu'aucun verbe n'écrit. Aujourd'hui rien ne
    /// converge — les phases 0 et 1 sont en lecture seule — et cette méthode ne
    /// sert qu'à nommer la frontière avant qu'un exécuteur existe.
    #[must_use]
    pub const fn est_convergeable(self) -> bool {
        match self {
            Self::Reglage => true,
            Self::Objectif | Self::Mesure | Self::Constat => false,
        }
    }
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
///
/// # La forme sérialisée d'`Absent`, et pourquoi elle n'est pas `null`
///
/// La sérialisation est `untagged`, et une variante **unité** y sérialise en
/// `null` — exactement comme `Option::None`. Mesuré : `desired: Some(Absent)`,
/// c'est-à-dire « cette clé ne doit pas exister », revenait en `None`, c'est-à-dire
/// « je ne contrains pas cet item », après un simple aller-retour. Deux intentions
/// opposées rendues indiscernables, et un [`Verdict::Ecart`] silencieusement
/// dégradé en [`Verdict::NonContraint`].
///
/// [`Self::Absent`] porte donc une **forme objet**, `{"absent": true}`, comme
/// [`Self::Illisible`] porte `{"raison": …}`. Le passage par
/// `#[serde(with = …)]` plutôt que par une variante struct est délibéré : la
/// variante reste **unité côté Rust**, donc `ItemValue::Absent` demeure
/// constructible et filtrable tel quel dans tout le workspace, et le changement
/// ne touche que le format d'échange, qui est le seul endroit où le défaut vivait.
///
/// # Le piège d'ordre, désamorcé plutôt que contourné
///
/// En `untagged`, serde essaie les variantes **dans l'ordre de déclaration**, et
/// une variante struct dérivée **ignore les champs inconnus**. Une forme
/// `Absent {}` déclarée avant `Illisible` avalerait donc `{"raison": "…"}`, et un
/// aveu d'illisibilité deviendrait une absence — le défaut réparé, remis à
/// l'envers. Ici les deux formes sont **mutuellement exclusives par leur
/// contenu** : `absent_objet` exige la clé `absent` et refuse toute autre clé
/// (`deny_unknown_fields`), tandis qu'`Illisible` exige `raison`. L'ordre de
/// déclaration ne peut donc plus décider du résultat, et le test
/// `aucune_valeur_ne_se_confond_avec_une_autre_apres_un_aller_retour` l'exige
/// variante par variante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemValue {
    /// Absence de valeur — l'item n'existe pas sur la machine.
    ///
    /// Sérialisée `{"absent": true}`, et **jamais** `null` : voir la note de
    /// forme sur [`ItemValue`].
    #[serde(with = "absent_objet")]
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
    /// donc une variante à chaîne se confondrait avec [`Self::Text`]. Un objet
    /// ne ressemble à aucune autre — et [`Self::Absent`] en porte un lui aussi
    /// depuis qu'on a mesuré que sa forme `null` se confondait avec `None`.
    Illisible {
        /// Pourquoi la lecture a échoué, en clair pour l'utilisateur.
        raison: String,
    },
}

/// La forme sérialisée d'[`ItemValue::Absent`] : `{"absent": true}`.
///
/// Un module plutôt qu'une variante struct, pour que la variante reste unité
/// côté Rust — voir la note de forme sur [`ItemValue`]. `deny_unknown_fields`
/// n'est pas décoratif : c'est lui qui empêche cette forme d'avaler un
/// `{"raison": "…"}` quel que soit l'ordre de déclaration des variantes.
mod absent_objet {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// L'objet effectivement écrit et relu. Un seul champ, et aucun autre toléré.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case", deny_unknown_fields)]
    struct Forme {
        /// Toujours `true`. Le champ porte le sens ; sa valeur ne fait que le confirmer.
        absent: bool,
    }

    /// Écrit `{"absent": true}`.
    pub(super) fn serialize<S: Serializer>(serialiseur: S) -> Result<S::Ok, S::Error> {
        Forme { absent: true }.serialize(serialiseur)
    }

    /// Relit `{"absent": true}`, et refuse tout le reste.
    ///
    /// `absent: false` n'a pas de sens : ce serait une absence qui n'en est pas
    /// une. On la refuse plutôt que de la traduire, faute de savoir en quoi.
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserialiseur: D) -> Result<(), D::Error> {
        let forme = Forme::deserialize(deserialiseur)?;
        if forme.absent {
            Ok(())
        } else {
            Err(serde::de::Error::custom(
                "« absent: false » ne décrit rien : une absence ne se nie pas",
            ))
        }
    }
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
    /// Ce que cet item a vocation à devenir dans le fichier d'état désiré.
    ///
    /// Obligatoire, comme [`Item::purpose`] et [`Item::risk`] : une valeur par
    /// défaut serait une décision qu'on n'a pas prise, et elle se prendrait
    /// alors sur quarante items d'un coup.
    pub nature: Nature,
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
        // Les DEUX côtés doivent être des constats, pas seulement le côté
        // observé. L'ADR-0008 l'écrit ainsi, et le code n'en posait qu'un.
        // Un désir `Illisible` n'a certes aucun sens — c'est même pourquoi
        // l'ADR-0010 décide qu'à terme le côté désiré ne réutilisera pas
        // `ItemValue` — mais tant qu'il est représentable, il est constructible,
        // et la comparaison par égalité le traiterait comme une valeur voulue.
        for (cote, valeur) in [("constatée", &self.observed), ("voulue", voulu)] {
            if !valeur.est_constat() {
                return Verdict::Incomparable {
                    raison: format!("valeur {cote} : {valeur}"),
                };
            }
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
            // La protection en temps réel a un état désirable, et un verbe
            // l'écrira : c'est le réglage archétypal.
            nature: Nature::Reglage,
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

    /// L'ADR-0008 pose la règle sur les DEUX côtés ; le code n'en gardait qu'un.
    ///
    /// Un désir illisible n'a aucun sens — c'est même la raison pour laquelle
    /// l'ADR-0010 décide qu'à terme le côté désiré ne réutilisera pas
    /// `ItemValue`. Mais tant qu'il est représentable, il est constructible :
    /// sans ce garde, la comparaison par égalité traitait un aveu écrit du côté
    /// voulu comme une valeur voulue, et pouvait déclarer « conforme » deux
    /// aveux de même raison.
    #[test]
    fn un_desir_qui_nest_pas_un_constat_rend_la_comparaison_impossible() {
        let raison = "accès refusé sans élévation";

        let i = item(Some(ItemValue::illisible(raison)), ItemValue::Bool(true));
        let v = i.verdict();
        assert!(matches!(v, Verdict::Incomparable { .. }), "{v:?}");
        assert!(!v.demande_convergence());

        // Le cas qui mordait le plus fort : deux aveux identiques des deux
        // côtés se seraient déclarés conformes par simple égalité.
        let deux_aveux = item(
            Some(ItemValue::illisible(raison)),
            ItemValue::illisible(raison),
        );
        assert_ne!(
            deux_aveux.verdict(),
            Verdict::Conforme,
            "deux « je n'ai pas su lire » ne font pas une conformité"
        );
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

    /// Toutes les valeurs du type, une par variante.
    ///
    /// Écrite à la main faute de mieux, mais **pas laissée à la vigilance** : le
    /// `match` exhaustif sans bras `_` ci-dessous casse la compilation le jour où
    /// une variante s'ajoute sans rejoindre cette liste. Une liste d'échantillons
    /// ne détecte jamais ce qu'on a oublié d'y mettre ; le compilateur, si.
    fn toutes_les_valeurs() -> Vec<ItemValue> {
        let echantillons = vec![
            ItemValue::Absent,
            ItemValue::Bool(true),
            ItemValue::Bool(false),
            ItemValue::Int(0),
            ItemValue::Int(-42),
            ItemValue::Text(String::new()),
            ItemValue::Text("absent".into()),
            ItemValue::List(Vec::new()),
            ItemValue::List(vec!["a".into(), "b".into()]),
            ItemValue::illisible("accès refusé sans élévation"),
        ];
        for v in &echantillons {
            match v {
                ItemValue::Absent
                | ItemValue::Bool(_)
                | ItemValue::Int(_)
                | ItemValue::Text(_)
                | ItemValue::List(_)
                | ItemValue::Illisible { .. } => {}
            }
        }
        echantillons
    }

    #[test]
    fn aucune_valeur_ne_se_confond_avec_une_autre_apres_un_aller_retour() {
        // Le défaut mesuré : `Absent` était une variante unité, donc sérialisée
        // `null` en `untagged` — indiscernable de `Option::None`. Ce test
        // l'exige variante par variante plutôt que sur le seul cas fautif : c'est
        // la famille du piège qu'on verrouille, pas son occurrence.
        for valeur in toutes_les_valeurs() {
            let json = serde_json::to_string(&valeur).expect("sérialisation");
            let relue: ItemValue = serde_json::from_str(&json).expect(&json);
            assert_eq!(relue, valeur, "aller-retour cassé : {json}");
        }

        // Et deux variantes distinctes ne produisent jamais la même écriture,
        // sans quoi l'aller-retour ci-dessus tiendrait par accident d'ordre.
        let mut vues: Vec<String> = toutes_les_valeurs()
            .iter()
            .map(|v| serde_json::to_string(v).expect("sérialisation"))
            .collect();
        let avant = vues.len();
        vues.sort();
        vues.dedup();
        assert_eq!(
            avant,
            vues.len(),
            "deux variantes s'écrivent pareil : {vues:?}"
        );

        // La forme d'`Absent` est un objet, nommément. Un `null` ici, et le
        // défaut est revenu.
        assert_eq!(
            serde_json::to_string(&ItemValue::Absent).expect("sérialisation"),
            r#"{"absent":true}"#
        );

        // Un aveu d'illisibilité ne se relit jamais en absence : c'est le piège
        // d'ordre d'`untagged`, celui qu'on remettrait à l'envers en donnant à
        // `Absent` une forme struct tolérante aux champs inconnus.
        let aveu: ItemValue =
            serde_json::from_str(r#"{"raison":"accès refusé"}"#).expect("forme connue");
        assert!(
            !aveu.est_constat(),
            "{aveu:?} : un aveu est devenu un constat"
        );
    }

    #[test]
    fn un_desir_dabsence_ne_se_relit_pas_en_absence_de_desir() {
        // Les deux intentions que la forme `null` confondait, et le verdict que
        // cette confusion faisait basculer : « cette clé ne doit pas exister »
        // (un écart, donc à converger) devenait « je ne contrains pas cet item »
        // (rien à faire). Le silence est du bon côté de l'erreur, ce qui est
        // exactement ce qui le rendait indétectable.
        for desire in [None, Some(ItemValue::Absent)] {
            let json = serde_json::to_string(&desire).expect("sérialisation");
            let relu: Option<ItemValue> = serde_json::from_str(&json).expect(&json);
            assert_eq!(relu, desire, "aller-retour cassé : {json}");
        }

        assert_ne!(
            serde_json::to_string(&Some(ItemValue::Absent)).expect("sérialisation"),
            serde_json::to_string(&Option::<ItemValue>::None).expect("sérialisation"),
            "« ne doit pas exister » et « non déclaré » se réécrivent pareil"
        );

        // Le verdict, qui est ce que tout cela protège : l'item survit à un
        // aller-retour complet sans changer de sens.
        let i = item(Some(ItemValue::Absent), ItemValue::Bool(true));
        let json = serde_json::to_string(&i).expect("sérialisation");
        let relu: Item = serde_json::from_str(&json).expect(&json);
        assert_eq!(relu.verdict(), Verdict::Ecart);
        assert_eq!(i.verdict(), relu.verdict());
    }

    /// Toutes les natures, une par variante.
    ///
    /// Même mécanique que [`toutes_les_valeurs`] : le `match` exhaustif sans
    /// bras `_` casse la **compilation** le jour où une variante s'ajoute sans
    /// rejoindre cette liste. Une liste d'échantillons écrite à la main ne
    /// détecte jamais ce qu'on a oublié d'y mettre ; le compilateur, si.
    fn toutes_les_natures() -> Vec<Nature> {
        let echantillons = vec![
            Nature::Reglage,
            Nature::Objectif,
            Nature::Mesure,
            Nature::Constat,
        ];
        for n in &echantillons {
            match n {
                Nature::Reglage | Nature::Objectif | Nature::Mesure | Nature::Constat => {}
            }
        }
        echantillons
    }

    #[test]
    fn une_nature_dit_ce_qui_se_declare_et_ce_qui_ne_se_converge_jamais() {
        // La table complète, parce que c'est elle qui décide de deux mécanismes
        // de la Phase 1 : ce que `ks import` écrit, et ce que le suivi de
        // dérive regarde.
        for (nature, declarable, convergeable) in [
            (Nature::Reglage, true, true),
            (Nature::Objectif, true, false),
            (Nature::Mesure, false, false),
            (Nature::Constat, false, false),
        ] {
            assert_eq!(
                nature.est_declarable(),
                declarable,
                "{nature:?} : vocation à être déclaré"
            );
            assert_eq!(
                nature.est_convergeable(),
                convergeable,
                "{nature:?} : vocation à être écrit"
            );
        }

        // Le cas qui justifie la quatrième variante plutôt que deux. Sans
        // `Objectif`, `vbs_running` serait soit non déclarable — donc jamais
        // suivi —, soit un réglage — donc la Phase 2 tenterait de le faire
        // converger, sans qu'aucun verbe sache l'écrire.
        assert!(Nature::Objectif.est_declarable());
        assert!(
            !Nature::Objectif.est_convergeable(),
            "on ne prétend pas écrire ce qu'aucun verbe n'écrit"
        );

        // Et rien de convergeable n'échappe à la déclaration : un item que
        // Keystone saurait écrire sans qu'on ait pu le vouloir serait une
        // écriture sans mandat.
        for nature in toutes_les_natures() {
            assert!(
                !nature.est_convergeable() || nature.est_declarable(),
                "{nature:?} : convergeable sans être déclarable"
            );
        }
    }

    #[test]
    fn aucune_nature_ne_se_confond_avec_une_autre_apres_un_aller_retour() {
        // La nature voyage dans `ks scan --json` et, demain, dans le fichier
        // d'état désiré : deux natures qui s'écriraient pareil feraient d'un
        // constat un réglage au premier aller-retour.
        let mut vues = Vec::new();
        for nature in toutes_les_natures() {
            let json = serde_json::to_string(&nature).expect("sérialisation");
            let relue: Nature = serde_json::from_str(&json).expect(&json);
            assert_eq!(relue, nature, "aller-retour cassé : {json}");
            vues.push(json);
        }

        let avant = vues.len();
        vues.sort();
        vues.dedup();
        assert_eq!(
            avant,
            vues.len(),
            "deux natures s'écrivent pareil : {vues:?}"
        );

        // La forme est nommée, pas positionnelle : un entier d'index se
        // décalerait au premier réordonnancement des variantes.
        assert_eq!(
            serde_json::to_string(&Nature::Reglage).expect("sérialisation"),
            r#""reglage""#
        );

        // Et l'item entier survit à l'aller-retour sans changer de vocation.
        let i = item(Some(ItemValue::Bool(true)), ItemValue::Bool(true));
        let json = serde_json::to_string(&i).expect("sérialisation");
        let relu: Item = serde_json::from_str(&json).expect(&json);
        assert_eq!(relu.nature, i.nature);
    }

    #[test]
    fn une_provenance_inconnue_est_un_signal_de_securite() {
        assert!(Provenance::Unknown.is_security_signal());
        assert!(!Provenance::WindowsUpdate.is_security_signal());
        assert!(Provenance::Managed("Intune".into()).is_sovereign());
    }
}

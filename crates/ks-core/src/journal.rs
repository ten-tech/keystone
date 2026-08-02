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

impl Actor {
    /// Étiquette **stable** pour le matériau d'empreinte.
    ///
    /// Surtout pas `{:?}` : la documentation de `Debug` prévient que le format
    /// dérivé n'est pas stable et peut changer d'une version de Rust à l'autre.
    /// Une empreinte adossée à `Debug` rend invérifiable, après une simple montée
    /// de compilateur, un journal déjà expédié hors machine — et « invérifiable »
    /// est indiscernable de « falsifié ».
    ///
    /// Le `match` est exhaustif : ajouter un acteur oblige à décider son étiquette.
    /// Ces valeurs **ne se renomment jamais** ; elles font partie du format.
    #[must_use]
    pub const fn etiquette_stable(&self) -> &'static str {
        match self {
            Self::Human(_) => "human",
            Self::Scheduler => "scheduler",
            Self::Copilot => "copilot",
            Self::System => "system",
        }
    }
}

impl Outcome {
    /// Étiquette stable du résultat. Même raison que [`Actor::etiquette_stable`].
    #[must_use]
    pub const fn etiquette_stable(&self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::Applied => "applied",
            Self::Refused { .. } => "refused",
            Self::RolledBack { .. } => "rolled-back",
            Self::Failed { .. } => "failed",
        }
    }

    /// La charge utile du résultat, vide pour les variantes qui n'en portent pas.
    ///
    /// Elle entre dans l'empreinte : sans elle, on pourrait changer le motif d'un
    /// refus ou le test de fumée fautif sans casser la chaîne.
    #[must_use]
    pub fn detail_stable(&self) -> &str {
        match self {
            Self::Simulated | Self::Applied => "",
            Self::Refused { reason } => reason,
            Self::RolledBack { failed_test } => failed_test,
            Self::Failed { detail } => detail,
        }
    }
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

/// Valeur de `prev_digest` attendue sur la toute première entrée d'un journal.
///
/// Sans ancre de départ, une troncature qui emporte le début de la chaîne laisse
/// un journal parfaitement cohérent avec lui-même. Le premier maillon doit donc
/// être reconnaissable.
pub const GENESIS_DIGEST: &str = "genesis";

/// Numéro de séquence de la toute première entrée.
///
/// L'ancre textuelle ne suffit pas : `GENESIS_DIGEST` est une constante publique
/// et non secrète, donc n'importe quelle entrée peut se prétendre premier maillon.
/// Sans contrainte sur le numéro, une entrée `seq: 42` portant cette ancre passait
/// la vérification — c'est-à-dire exactement la troncature du début que l'ancrage
/// prétend exclure.
pub const GENESIS_SEQ: u64 = 1;

impl JournalEntry {
    /// Empreinte de cette entrée, à reporter dans `prev_digest` de la suivante.
    ///
    /// # Ce qui entre dans l'empreinte
    ///
    /// **Tous** les champs de l'entrée, `diff` et `outcome` compris. Ils en étaient
    /// absents à l'origine, ce qui laissait réécrire un `Outcome::Failed` en
    /// `Applied`, ou vider un `diff`, sans casser la chaîne — c'est-à-dire défaire
    /// précisément ce que SEC-03 et SEC-04 prétendent garantir.
    ///
    /// Chaque champ est préfixé de sa longueur plutôt que séparé par `|` : sinon
    /// deux entrées différentes peuvent produire le même matériau dès qu'une valeur
    /// contient le séparateur, et une empreinte qu'on peut faire collisionner à la
    /// main ne vaut rien, même avec BLAKE3 derrière.
    ///
    /// # Ce que cette empreinte garantit, et ce qu'elle ne garantit pas
    ///
    /// BLAKE3, depuis la Phase 0.5. Le bouchon FNV-1a qui la précédait se
    /// collisionnait à la demande : la chaîne ne détectait rien du tout.
    ///
    /// Elle détecte désormais la corruption accidentelle, la troncature, et la
    /// réécriture par un attaquant **non privilégié**.
    ///
    /// Elle ne détecte **pas** un attaquant SYSTEM. L'ancrage est public,
    /// l'algorithme est public : qui obtient ce niveau réécrit ce qu'il veut,
    /// recalcule toutes les empreintes, et [`Self::verify_chain`] répond
    /// « intacte ». La parade est une clé scellée dans le TPM, donc le broker,
    /// donc la Phase 2. Voir `docs/adr/0004-chainage-du-journal.md`, dont la
    /// section « ce que ça ne garantit pas » doit être lue avant de citer ce
    /// mécanisme comme une protection.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut material = String::new();
        let mut champ = |s: &str| {
            // Préfixe de longueur : rend le découpage non ambigu.
            material.push_str(&format!("{}:{s}", s.len()));
        };
        // Étiquette de domaine et de version. Sans elle, une autre structure hachée
        // un jour par le même schéma pourrait produire le matériau d'une entrée de
        // journal. Elle change si le format change, ce qui rend la rupture visible
        // au lieu d'être silencieuse.
        champ("ks-journal-v1");
        champ(&self.seq.to_string());
        champ(&self.at.to_rfc3339());
        champ(self.actor.etiquette_stable());
        if let Actor::Human(nom) = &self.actor {
            champ(nom);
        }
        champ(&self.verb);
        champ(&self.target);
        champ(self.diff.as_deref().unwrap_or(""));
        // Distingue `Some("")` de `None` : sans ce marqueur, un diff vide et un
        // diff absent produiraient la même empreinte.
        champ(if self.diff.is_some() { "1" } else { "0" });
        champ(self.outcome.etiquette_stable());
        champ(self.outcome.detail_stable());
        champ(&self.prev_digest);

        // Le préfixe nomme l'algorithme. Il n'est pas décoratif : le jour où une
        // empreinte clée arrivera, les deux régimes doivent être distinguables
        // dans un journal existant, sinon la migration se fait à l'aveugle.
        format!("blake3:{}", blake3::hash(material.as_bytes()).to_hex())
    }

    /// Le chaînage est-il intact sur toute la séquence ?
    ///
    /// Vérifie trois choses, et pas seulement le maillon à maillon :
    ///
    /// * la séquence n'est pas vide — un journal absent n'est pas un journal valide ;
    /// * la première entrée s'ancre sur [`GENESIS_DIGEST`] **et** porte [`GENESIS_SEQ`] ;
    /// * chaque entrée suit la précédente en numéro **et** en empreinte.
    ///
    /// Les deux conditions d'ancrage comptent, et la seconde a été ajoutée après
    /// coup : `windows(2)` renvoie `true` sur une séquence d'un élément, et
    /// `GENESIS_DIGEST` est une constante publique que n'importe qui peut recopier.
    /// Une entrée isolée `seq: 42` portant l'ancre passait donc la vérification.
    #[must_use]
    pub fn verify_chain(entries: &[Self]) -> bool {
        let Some(first) = entries.first() else {
            return false;
        };
        if first.prev_digest != GENESIS_DIGEST || first.seq != GENESIS_SEQ {
            return false;
        }
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
        let e1 = entry(1, GENESIS_DIGEST);
        let e2 = entry(2, &e1.digest());
        assert!(JournalEntry::verify_chain(&[e1.clone(), e2.clone()]));

        // On saute une entrée : la chaîne doit casser.
        let e4 = entry(4, &e2.digest());
        assert!(!JournalEntry::verify_chain(&[e1, e2, e4]));
    }

    #[test]
    fn reecrire_le_resultat_casse_lempreinte() {
        // Le scénario que le journal existe pour rendre impossible : un attaquant
        // qui a agi, a échoué, et voudrait que la trace dise « appliqué ».
        let honnete = JournalEntry {
            outcome: Outcome::RolledBack {
                failed_test: "smoke:db-migrate".into(),
            },
            ..entry(1, GENESIS_DIGEST)
        };
        let maquille = JournalEntry {
            outcome: Outcome::Applied,
            ..honnete.clone()
        };

        assert_ne!(
            honnete.digest(),
            maquille.digest(),
            "le résultat doit entrer dans l'empreinte, sinon on peut transformer \
             un échec en succès sans casser la chaîne"
        );
    }

    #[test]
    fn vider_le_diff_casse_lempreinte() {
        let avec = JournalEntry {
            diff: Some("services.Fax.startupType: Automatic → Disabled".into()),
            ..entry(1, GENESIS_DIGEST)
        };
        let sans = JournalEntry {
            diff: None,
            ..avec.clone()
        };
        let vide = JournalEntry {
            diff: Some(String::new()),
            ..avec.clone()
        };

        assert_ne!(avec.digest(), sans.digest(), "le diff doit être couvert");
        assert_ne!(
            sans.digest(),
            vide.digest(),
            "« pas de diff » et « diff vide » ne disent pas la même chose"
        );
    }

    #[test]
    fn deux_entrees_differentes_ne_partagent_pas_une_empreinte() {
        // Sans préfixe de longueur, un séparateur présent dans une valeur permet de
        // déplacer la frontière entre deux champs et de fabriquer une collision.
        let a = JournalEntry {
            verb: "set-service".into(),
            target: "Fax|extra".into(),
            ..entry(1, GENESIS_DIGEST)
        };
        let b = JournalEntry {
            verb: "set-service|Fax".into(),
            target: "extra".into(),
            ..entry(1, GENESIS_DIGEST)
        };
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn lempreinte_nomme_son_algorithme_et_reagit_au_moindre_octet() {
        // Le préfixe n'est pas cosmétique : le jour où une empreinte clée
        // arrivera (Phase 2, ADR-0004), les deux régimes devront se distinguer
        // dans un journal existant, sinon la migration se fait à l'aveugle.
        let a = entry(1, GENESIS_DIGEST);
        let empreinte = a.digest();
        assert!(
            empreinte.starts_with("blake3:"),
            "l'algorithme doit se nommer : {empreinte}"
        );
        assert_eq!(
            empreinte.len(),
            "blake3:".len() + 64,
            "BLAKE3 rend 32 octets, donc 64 caractères hexadécimaux"
        );

        // Effet d'avalanche : un seul caractère de différence sur un seul champ
        // doit changer l'empreinte de fond en comble. Le bouchon FNV-1a passait
        // ce test-ci, et échouait sur le seul qui comptait — il se collisionnait
        // à la demande. C'est pourquoi cette assertion ne prouve pas la
        // résistance : elle vérifie qu'on n'a pas cassé le chaînage, la
        // résistance venant de l'algorithme, pas de nos tests.
        let mut b = entry(1, GENESIS_DIGEST);
        b.target.push('X');
        assert_ne!(empreinte, b.digest());

        let communs = empreinte
            .chars()
            .zip(b.digest().chars())
            .filter(|(x, y)| x == y)
            .count();
        assert!(
            communs < empreinte.len() / 2,
            "deux empreintes voisines ne doivent pas se ressembler"
        );
    }

    #[test]
    fn un_journal_tronque_a_son_debut_est_refuse() {
        // `windows(2)` seul est vrai sur une séquence d'un élément : un journal
        // réduit à sa dernière entrée passait la vérification sans rien prouver.
        let orpheline = entry(42, "blake3:0000000000000000");
        assert!(
            !JournalEntry::verify_chain(&[orpheline]),
            "une entrée dont le prédécesseur a disparu n'ancre rien"
        );

        // Le cas réellement dangereux : l'ancre textuelle est publique, donc
        // recopiable. Sans contrainte sur le numéro de séquence, cette entrée
        // passait — et c'est précisément la troncature du début.
        assert!(
            !JournalEntry::verify_chain(&[entry(42, GENESIS_DIGEST)]),
            "porter l'ancre de genèse ne suffit pas : il faut aussi en avoir le numéro"
        );
        assert!(
            !JournalEntry::verify_chain(&[]),
            "un journal vide n'est pas un journal valide"
        );
        assert!(JournalEntry::verify_chain(&[entry(1, GENESIS_DIGEST)]));
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

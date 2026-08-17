//! Le journal — chaîné, et expédié hors machine parce que le chaînage seul ne
//! suffit pas.
//!
//! Le mot « inaltérable » ouvrait ce fichier. Il était faux dans les deux sens
//! qui comptent : la troncature par la fin est indétectable localement, et un
//! attaquant SYSTEM refabrique la chaîne entière. C'est écrit dans l'ADR-0004,
//! et c'est la raison d'être de SEC-04.
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
    /// **Rien n'a été tenté** : l'entrée consigne une observation.
    ///
    /// Ajoutée en Phase 0.5, quand `ks scan --record` a eu besoin de se
    /// journaliser. Toutes les autres variantes décrivent le sort d'une
    /// *action* ; un scan n'en est pas une. Écrire `Applied` aurait laissé
    /// croire à une écriture, `Simulated` à une action envisagée puis retenue.
    /// Aucune des deux n'est vraie, et un journal qui se trompe sur la nature de
    /// ce qu'il consigne perd sa raison d'être.
    Observed,

    /// **Un humain a tranché** : il tolère un écart, jusqu'à une date.
    ///
    /// Ajoutée en Phase 1 pour l'exigence D2-07, et pour la même raison
    /// qu'`Observed` l'avait été : les six variantes précédentes décrivent
    /// toutes le sort d'une *action* tentée sur la machine, et une décision n'en
    /// est pas une. `Refused` s'en approche et ment tout de même : elle dit qu'un
    /// contrôle d'admission a écarté une action, quand ici personne n'a rien
    /// tenté et que c'est un choix qui est consigné.
    ///
    /// Le journal reste la mémoire du **pourquoi**. Le fichier d'état désiré, lui,
    /// porte les tolérances en vigueur et reste seul consulté pour calculer un
    /// verdict : les deux répondent à des questions différentes, et l'ADR-0020
    /// explique pourquoi aucune des deux ne remplace l'autre.
    Decided {
        /// Pourquoi cet écart est toléré. Jamais vide (exigence D2-06).
        reason: String,
        /// Le jour où la tolérance cesse, inclus.
        expires: chrono::NaiveDate,
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
            Self::Observed => "observed",
            Self::Decided { .. } => "decided",
        }
    }

    /// La charge utile du résultat, champ par champ.
    ///
    /// Elle entre dans l'empreinte : sans elle, on pourrait changer le motif d'un
    /// refus, le test de fumée fautif, ou **l'échéance d'une tolérance** sans
    /// casser la chaîne.
    ///
    /// # Pourquoi une liste, et pourquoi l'arité variable ne crée pas d'ambiguïté
    ///
    /// `Decided` porte deux champs, les autres un seul. Un matériau d'arité
    /// variable serait ambigu si rien ne disait combien de champs suivent ; ici
    /// [`Self::etiquette_stable`] les précède, elle vient d'un ensemble fermé, et
    /// elle détermine donc l'arité. Chaque champ reste par ailleurs préfixé de sa
    /// longueur, de sorte qu'aucune valeur ne peut déplacer une frontière.
    ///
    /// # Les variantes sans charge utile rendent **un champ vide**, pas zéro
    ///
    /// Ce n'est pas une coquetterie : elles émettaient déjà `""` avant que cette
    /// méthode remplace un `detail_stable` unique, et rendre zéro champ au lieu
    /// d'un champ vide changerait leur empreinte, donc rendrait invérifiable tout
    /// journal déjà écrit. C'est exactement ce que surveille
    /// `le_materiau_dune_entree_existante_na_pas_bouge`, qui porte les cinq
    /// valeurs mesurées avant ce changement.
    #[must_use]
    pub fn champs_stables(&self) -> Vec<String> {
        match self {
            Self::Simulated | Self::Applied | Self::Observed => vec![String::new()],
            Self::Refused { reason } => vec![reason.clone()],
            Self::RolledBack { failed_test } => vec![failed_test.clone()],
            Self::Failed { detail } => vec![detail.clone()],
            Self::Decided { reason, expires } => vec![reason.clone(), expires.to_string()],
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
        // L'étiquette vient d'être écrite, et elle détermine combien de champs
        // suivent : le découpage reste non ambigu malgré l'arité variable.
        for valeur in self.outcome.champs_stables() {
            champ(&valeur);
        }
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

    /// Une entrée **entièrement déterministe**, pour figer l'empreinte.
    ///
    /// Aucune horloge : `Utc::now()` rendrait la valeur irreproductible, donc le
    /// test impossible à écrire. C'est la même règle que partout ailleurs dans ce
    /// dépôt — pas d'`Instant::now()` ni de `rand` nu dans un chemin asserté.
    fn entree_figee(outcome: Outcome) -> JournalEntry {
        JournalEntry {
            seq: 1,
            at: chrono::DateTime::parse_from_rfc3339("2026-08-17T09:12:00+02:00")
                .expect("date littérale valide")
                .with_timezone(&Utc),
            actor: Actor::Human("exemple".into()),
            verb: "scan".into(),
            target: "inventory".into(),
            diff: None,
            outcome,
            prev_digest: GENESIS_DIGEST.into(),
        }
    }

    #[test]
    fn le_materiau_dune_entree_existante_na_pas_bouge() {
        // **Ce test protège les journaux déjà écrits, pas le code.**
        //
        // `digest()` couvre l'étiquette du résultat et sa charge utile. Le jour
        // où une variante s'ajoute — et il est arrivé le 2026-08-17 avec
        // `Decided` —, la tentation est de refondre le matériau au passage. Une
        // refonte, même correcte, rend invérifiable tout journal écrit avant
        // elle : `verify_chain` répond « rompue » sur une chaîne intacte, et
        // « invérifiable » est indiscernable de « falsifié ».
        //
        // Les valeurs ci-dessous ont été **mesurées sur le code d'alors**, puis
        // recopiées. Elles ne se recalculent pas : un attendu qu'on régénère
        // depuis le code qu'il surveille ne surveille rien.
        //
        // Les cinq variantes d'origine sont là, dont les trois sans charge utile,
        // parce que c'est précisément leur champ vide qui disparaît si l'on passe
        // d'un détail unique à une liste de champs sans y prendre garde.
        for (outcome, attendu) in [
            (
                Outcome::Simulated,
                "blake3:cb960d40d68b5c2a3d5858ee3990657d6ee9c2dd78175fe229ed990b6a5a032e",
            ),
            (
                Outcome::Applied,
                "blake3:6800b44caaa93adb3779f397b75c7dd1bed4a09da3fa126e59a300c9e8df4785",
            ),
            (
                Outcome::Observed,
                "blake3:a15f053833d1398ec94b835cf709a04962a84727c252b7a533bd651a6b56cadc",
            ),
            (
                Outcome::Refused {
                    reason: "conflit de politique gérée".into(),
                },
                "blake3:5cf10de22c115129ccd96c7fca026780d0ad1fb0fcca4af2edf8fd7448cdcebe",
            ),
            (
                Outcome::Failed {
                    detail: "E_ACCESSDENIED".into(),
                },
                "blake3:8625f10f04a4f599dc3becda281c4a7c365e7df6ae2352945e3150f76087dbaa",
            ),
        ] {
            let etiquette = outcome.etiquette_stable();
            assert_eq!(
                entree_figee(outcome).digest(),
                attendu,
                "le matériau de « {etiquette} » a changé : tout journal déjà écrit \
                 devient invérifiable. Si le changement est voulu, il faut une \
                 étiquette de version (« ks-journal-v2 ») et une migration, \
                 jamais une réécriture silencieuse."
            );
        }
    }

    /// Une décision figée, dont on fera varier un champ à la fois.
    fn decision(raison: &str, echeance: &str) -> JournalEntry {
        entree_figee(Outcome::Decided {
            reason: raison.to_owned(),
            expires: echeance.parse().expect("date littérale valide"),
        })
    }

    #[test]
    fn modifier_lecheance_dune_decision_casse_lempreinte() {
        // Le cœur de D2-07 : sans échéance dans le matériau, une tolérance de
        // trois mois se réécrit en tolérance de dix ans sans qu'aucune chaîne ne
        // s'en aperçoive. L'exception permanente silencieuse que l'exigence
        // interdit reviendrait alors par la porte du journal.
        let court = decision("pilote du scanner du labo", "2026-10-15");
        let long = decision("pilote du scanner du labo", "2036-10-15");
        assert_ne!(court.digest(), long.digest());
    }

    #[test]
    fn modifier_la_raison_dune_decision_casse_lempreinte() {
        // L'oubli du pourquoi est la principale cause de pourrissement des
        // configurations ; une raison réécrite après coup est pire que pas de
        // raison, parce qu'elle se croit.
        let vraie = decision("pilote du scanner du labo", "2026-10-15");
        let refaite = decision("on verra plus tard", "2026-10-15");
        assert_ne!(vraie.digest(), refaite.digest());
    }

    #[test]
    fn deux_champs_ne_se_confondent_pas_avec_un_seul() {
        // Le piège de l'arité variable, éprouvé plutôt qu'affirmé : une raison
        // qui contiendrait la date collée derrière elle ne doit pas produire le
        // matériau d'une décision à deux champs. C'est le préfixe de longueur
        // qui l'empêche, et c'est ici qu'on le vérifie.
        let deux_champs = decision("raison", "2026-10-15");
        let un_seul = entree_figee(Outcome::Refused {
            reason: "raison2026-10-15".into(),
        });
        assert_ne!(deux_champs.digest(), un_seul.digest());
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

//! Lecture de `workstation.yaml` — le fichier d'état désiré (ADR-0016).
//!
//! ## Lecture seule, et pas par timidité
//!
//! Ce module **lit**. Il n'écrit pas, et le crate YAML est configuré pour ne pas
//! le pouvoir : `default-features = false, features = ["deserialize"]`. Un
//! sérialiseur serde a été mesuré et écarté — il émet
//! `security.services.windefend.startup: automatique` sans guillemets, donc un
//! fichier qui se relit correctement **par chance**, et il n'émet aucun
//! commentaire, ce qui annulerait la contrepartie promise par l'ADR-0010 à la
//! forme plate. L'écriture passera par un émetteur maison, dans un lot
//! ultérieur.
//!
//! ## Ce que le document promet, et ce qu'il ne promet pas
//!
//! Le premier niveau est **fermé** : `deny_unknown_fields`. Une clé inattendue à
//! la racine est une faute de frappe ou un format plus récent ; dans les deux
//! cas, l'ignorer en silence est le mauvais choix. Les structures fermées des
//! niveaux suivants — `metadata`, une entrée d'`acceptedDrift` — le sont pour la
//! même raison, écrite une fois ici : ce sont des formes connues, un champ
//! inconnu y est une faute.
//!
//! `desired`, lui, est **délibérément ouvert** : n'importe quelle chaîne y est
//! une clé valide, parce qu'il n'existe pas de catalogue statique des chemins
//! d'items (ADR-0010). Le seul référentiel est ce qu'un scan a observé.
//!
//! ## La table `desired` sort d'ici non typée, et c'est voulu
//!
//! Elle est lue en [`ScalaireBrut`], c'est-à-dire « ce que YAML a donné ». Le
//! typage se fait ensuite, item par item, par [`Desire::contraindre`], contre la
//! **forme de la valeur constatée**. C'est le seul moyen de neutraliser le
//! typage implicite de YAML dans un fichier édité à la main, où c'est l'humain
//! qui écrira `off`.
//!
//! Conséquence assumée, et qui n'est pas un défaut de ce module : `ks diff` a
//! besoin d'un scan pour typer le fichier. Un état désiré n'a de sens que
//! confronté à une machine.
//!
//! ## Ce que ce module ne fait pas
//!
//! Il ne lit pas le disque : `ks import` et `ks diff` apporteront la source et
//! le chemin à afficher. Il ne signale pas non plus les chemins « déclarés, non
//! observés » — cela demande l'inventaire, donc la commande, pas le lecteur.

use std::collections::BTreeMap;

use serde::Deserialize;

use ks_core::ScalaireBrut;

/// La seule version de format que ce lecteur sait relire.
///
/// Un document qui en annonce une autre est refusé plutôt que lu au mieux : un
/// champ dont la sémantique a changé se relit sans erreur et ment ensuite.
pub const VERSION_DE_FORMAT: &str = "keystone/v1";

/// Le seul genre de document attendu, aujourd'hui.
pub const GENRE_ATTENDU: &str = "Workstation";

/// Le fichier d'état désiré, tel qu'il est écrit et relu.
///
/// La forme est celle décidée par l'ADR-0010 : un en-tête, une table plate
/// indexée par chemin d'item, une liste d'écarts acceptés.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EtatDesire {
    /// Version du format, `keystone/v1`.
    pub api_version: String,
    /// Genre du document, `Workstation`.
    pub kind: String,
    /// De quelle machine il s'agit, et de quoi elle hérite.
    pub metadata: Metadata,
    /// Ce qui est voulu, indexé par chemin d'item — **avant tout typage**.
    ///
    /// `BTreeMap` et non `HashMap` : l'ordre des clés est stable d'une lecture à
    /// l'autre, donc un diff affiché deux fois s'affiche deux fois pareil.
    #[serde(default)]
    pub desired: BTreeMap<String, ScalaireBrut>,
    /// Les écarts volontairement tolérés, avec leur échéance.
    #[serde(default)]
    pub accepted_drift: Vec<EcartAccepte>,
}

/// De quelle machine parle le document.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    /// Nom de la machine.
    pub name: String,
    /// Surcouche de flotte dont ce fichier hérite (D2-10, D13-01).
    #[serde(default)]
    pub inherits: Option<String>,
    /// Qui tient ce poste.
    #[serde(default)]
    pub owner: Option<String>,
    /// À quoi il sert, en une phrase.
    #[serde(default)]
    pub description: Option<String>,
}

/// Un écart toléré volontairement, et jusqu'à quand.
///
/// Exigence D2-06 : la raison **et** la date d'expiration sont obligatoires, et
/// c'est le typage qui le tient — aucun des deux champs n'est optionnel. C'est
/// le seul mécanisme qui empêche un fichier d'état de pourrir sous une couche
/// d'exceptions dont personne ne se rappelle le motif.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcartAccepte {
    /// Le chemin de l'item toléré.
    pub item: String,
    /// Pourquoi cet écart est toléré.
    pub reason: String,
    /// Quand la tolérance expire, et l'écart redevient actif tout seul.
    pub expires: chrono::NaiveDate,
    /// Qui a décidé.
    pub decided_by: String,
    /// Quand la décision a été prise.
    pub decided_at: ks_core::Timestamp,
}

/// Ce qui peut empêcher de relire un fichier d'état désiré.
///
/// Le message destiné à l'utilisateur et le détail technique sont **deux champs
/// distincts**, jamais une chaîne concaténée : c'est ce qui permet d'afficher
/// l'un et de journaliser l'autre.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurDeLecture {
    /// Le document n'a pas pu être relu en entier.
    ///
    /// Trois causes se rejoignent ici, et le détail seul les distingue : une clé
    /// inconnue à la racine, une clé écrite deux fois, ou un YAML invalide
    /// (tabulation, indentation irrégulière, guillemet non fermé).
    ///
    /// Le détail vient du crate de lecture, **en anglais**. Il ne suit donc pas
    /// la règle de voix du projet, et c'est une limite consignée par
    /// l'ADR-0016 : on préfère un message étranger qui nomme la ligne fautive à
    /// un message français qui ne la nomme pas.
    #[error(
        "Le fichier d'état désiré n'a pas pu être relu en entier : une clé y est \
         inconnue, écrite deux fois, ou le document n'est pas un YAML valide. \
         Tant que la lecture échoue, rien n'est comparé. Le détail technique \
         nomme la ligne en cause."
    )]
    Document {
        /// Ce que le lecteur YAML a répondu, mot pour mot.
        detail: String,
    },

    /// L'en-tête annonce un format que ce lecteur ne sait pas relire.
    #[error(
        "Le fichier d'état désiré annonce « {trouve} », et ce lecteur relit \
         « {attendu} ». Rien n'a été comparé. Vérifiez l'en-tête du fichier."
    )]
    Format {
        /// Ce que le fichier annonce.
        trouve: String,
        /// Ce que ce lecteur sait relire.
        attendu: &'static str,
    },
}

impl EtatDesire {
    /// Relit un fichier d'état désiré depuis sa source.
    ///
    /// La source arrive en mémoire plutôt qu'en chemin : le lecteur reste pur,
    /// donc testable sans disque, et c'est la commande qui décide d'où vient le
    /// texte et quel chemin afficher.
    ///
    /// # Erreurs
    ///
    /// * [`ErreurDeLecture::Document`] — clé inconnue à la racine, clé en
    ///   double, ou YAML invalide.
    /// * [`ErreurDeLecture::Format`] — `apiVersion` ou `kind` inattendu.
    pub fn lire(source: &str) -> Result<Self, ErreurDeLecture> {
        let document: Self =
            serde_saphyr::from_str(source).map_err(|e| ErreurDeLecture::Document {
                detail: e.to_string(),
            })?;

        // Validation au boundary : ce qui entre depuis un fichier est aussi peu
        // fiable qu'un input réseau, et `workstation.yaml` est inscriptible par
        // l'utilisateur — donc par l'adversaire A1.
        for (trouve, attendu) in [
            (&document.api_version, VERSION_DE_FORMAT),
            (&document.kind, GENRE_ATTENDU),
        ] {
            if trouve != attendu {
                return Err(ErreurDeLecture::Format {
                    trouve: trouve.clone(),
                    attendu,
                });
            }
        }

        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use ks_core::{Desire, FormeAttendue, ItemValue};

    use super::*;

    /// Un document minimal, auquel le corps du test ajoute ce qu'il éprouve.
    const EN_TETE: &str = "\
apiVersion: keystone/v1
kind: Workstation
metadata:
  name: WKS-EXEMPLE-01
";

    /// Relit un document fait d'un seul scalaire déclaré.
    fn lit_un_scalaire(ecrit: &str) -> Result<ScalaireBrut, ErreurDeLecture> {
        let source = format!("{EN_TETE}desired:\n  un.chemin: {ecrit}\n");
        let document = EtatDesire::lire(&source)?;
        Ok(document
            .desired
            .get("un.chemin")
            .cloned()
            .expect("la clé vient d'être écrite"))
    }

    // ── La batterie des vingt et un scalaires ──────────────────────────────
    //
    // Vingt et une valeurs réellement relevées sur la machine de référence ou
    // plausibles dans un fichier écrit à la main, telles que l'ADR-0016 les a
    // mesurées. La table ci-dessous est la mesure, pas une intention : chaque
    // ligne dit ce que YAML **donne**, y compris quand c'est absurde.

    /// Ce que le lecteur rend d'un scalaire écrit tel quel.
    #[derive(Debug, PartialEq, Eq)]
    enum CeQueYamlDonne {
        /// Un booléen, que l'humain ait écrit `false` ou `off`.
        Booleen(bool),
        /// Un entier, que l'humain ait écrit `9` ou `0x9`.
        Entier(i64),
        /// Un texte, seul cas où le fichier rend ce qu'on y a écrit.
        Texte,
        /// Le document entier est refusé : `08`, `~`, `null`.
        Refus,
    }

    /// Les vingt et un scalaires, et ce que YAML en fait.
    ///
    /// L'ordre suit le tableau de l'ADR-0016. `false` et `False` complètent la
    /// famille de sa première ligne, qui n'en citait que les quatre membres
    /// surprenants — ce sont eux qui portent le compte à vingt et un.
    const BATTERIE: &[(&str, CeQueYamlDonne)] = &[
        // Ce que YAML lit comme faux.
        ("no", CeQueYamlDonne::Booleen(false)),
        ("n", CeQueYamlDonne::Booleen(false)),
        ("off", CeQueYamlDonne::Booleen(false)),
        ("Off", CeQueYamlDonne::Booleen(false)),
        ("false", CeQueYamlDonne::Booleen(false)),
        ("False", CeQueYamlDonne::Booleen(false)),
        // Ce que YAML lit comme vrai.
        ("yes", CeQueYamlDonne::Booleen(true)),
        ("y", CeQueYamlDonne::Booleen(true)),
        ("on", CeQueYamlDonne::Booleen(true)),
        ("true", CeQueYamlDonne::Booleen(true)),
        ("True", CeQueYamlDonne::Booleen(true)),
        // Les deux items réellement exposés sur la machine de référence :
        // `inventory.os.kernel` vaut Text("26200") et
        // `security.firmware.microcode_revision` vaut Text("23410000").
        // Déclarés sans guillemets, ils reviendraient en entier et produiraient
        // un écart PERMANENT sur une machine qui n'a jamais changé.
        ("23410000", CeQueYamlDonne::Entier(23_410_000)),
        ("26200", CeQueYamlDonne::Entier(26_200)),
        // L'hexadécimal, que personne n'a demandé.
        ("0x9", CeQueYamlDonne::Entier(9)),
        // Le zéro en tête ne donne pas 8 : il fait échouer la lecture.
        ("08", CeQueYamlDonne::Refus),
        // Les deux écritures du vide. Un désir d'absence s'écrit
        // `{ absent: true }`, jamais `~` ni `null`.
        ("~", CeQueYamlDonne::Refus),
        ("null", CeQueYamlDonne::Refus),
        // Et les quatre qui se comportent : un jeton (ADR-0015), un numéro de
        // série, une version, une date.
        ("automatique", CeQueYamlDonne::Texte),
        ("P0CN20WW", CeQueYamlDonne::Texte),
        ("25.3.0", CeQueYamlDonne::Texte),
        ("2024-10-21", CeQueYamlDonne::Texte),
    ];

    #[test]
    fn les_vingt_et_un_scalaires_donnent_ce_que_yaml_a_decide() {
        assert_eq!(BATTERIE.len(), 21, "la batterie a changé de taille");

        for (ecrit, attendu) in BATTERIE {
            let obtenu = lit_un_scalaire(ecrit);
            match (attendu, &obtenu) {
                (CeQueYamlDonne::Booleen(b), Ok(ScalaireBrut::Bool(lu))) => {
                    assert_eq!(lu, b, "« {ecrit} »");
                }
                (CeQueYamlDonne::Entier(n), Ok(ScalaireBrut::Int(lu))) => {
                    assert_eq!(lu, n, "« {ecrit} »");
                }
                (CeQueYamlDonne::Texte, Ok(ScalaireBrut::Text(lu))) => {
                    assert_eq!(lu, ecrit, "« {ecrit} » : le texte a été transformé");
                }
                (CeQueYamlDonne::Refus, Err(ErreurDeLecture::Document { .. })) => {}
                _ => panic!("« {ecrit} » : attendu {attendu:?}, obtenu {obtenu:?}"),
            }
        }
    }

    #[test]
    fn huit_des_vingt_et_un_scalaires_sont_retypes_en_silence() {
        // **Le décompte est calculé, jamais recopié.** Est « piégeux » un
        // scalaire que YAML retype SANS BRUIT vers une valeur dont l'écriture
        // littérale n'est pas celle que l'humain a tapée :
        //
        //   `off` devient false, et « off » ne s'écrit pas « false » → piégeux ;
        //   `false` devient false, et « false » s'écrit « false »    → non ;
        //   `0x9` devient 9, et « 0x9 » ne s'écrit pas « 9 »         → piégeux ;
        //   `26200` devient 26200, écrit à l'identique               → non ;
        //   `08` et `~` sont REFUSÉS, donc bruyants                  → non.
        //
        // Ce critère est mécanique : il ne dépend d'aucune liste écrite à la
        // main, donc il ne peut pas dériver quand la batterie s'allonge. Il
        // isole exactement ce qui est dangereux — la conversion muette — et
        // laisse de côté le refus, qui se voit.
        let piegeux: Vec<&str> = BATTERIE
            .iter()
            .filter(|(ecrit, donne)| match donne {
                CeQueYamlDonne::Booleen(b) => ecrit.to_lowercase() != b.to_string(),
                CeQueYamlDonne::Entier(n) => *ecrit != n.to_string(),
                CeQueYamlDonne::Texte | CeQueYamlDonne::Refus => false,
            })
            .map(|(ecrit, _)| *ecrit)
            .collect();

        assert_eq!(
            piegeux,
            ["no", "n", "off", "Off", "yes", "y", "on", "0x9"],
            "la famille des conversions muettes a changé"
        );
        assert_eq!(
            piegeux.len(),
            8,
            "l'ADR-0016 en compte huit sur vingt et un"
        );
    }

    #[test]
    fn face_a_un_item_texte_seuls_les_textes_passent() {
        // Le contrôle de l'ADR-0016 appliqué à toute la batterie : sur un item
        // dont la valeur constatée est un texte — le cas de
        // `security.services.windefend.startup`, qui vaut le jeton
        // « automatique » —, tout ce que YAML n'a pas rendu en texte est
        // refusé. Les huit conversions muettes deviennent ainsi huit refus
        // nommés, ce qui est précisément le but de la décision.
        let observe = ItemValue::Text("automatique".into());
        let mut acceptes = 0;
        let mut refuses = 0;

        for (ecrit, attendu) in BATTERIE {
            let Ok(brut) = lit_un_scalaire(ecrit) else {
                // `08`, `~` et `null` n'arrivent jamais au contrôle : le
                // document entier est refusé avant.
                assert_eq!(*attendu, CeQueYamlDonne::Refus, "« {ecrit} »");
                refuses += 1;
                continue;
            };

            match Desire::contraindre(brut, "security.services.windefend.startup", &observe) {
                Ok(desire) => {
                    assert_eq!(
                        *attendu,
                        CeQueYamlDonne::Texte,
                        "« {ecrit} » accepté à tort"
                    );
                    assert_eq!(desire, Desire::Text((*ecrit).to_owned()));
                    acceptes += 1;
                }
                Err(e) => {
                    assert_ne!(*attendu, CeQueYamlDonne::Texte, "« {ecrit} » refusé à tort");
                    assert_eq!(e.attendu, FormeAttendue::Texte);
                    assert!(
                        e.to_string().contains("Entourez"),
                        "« {ecrit} » : le message ne dit pas quoi faire — {e}"
                    );
                    refuses += 1;
                }
            }
        }

        assert_eq!(acceptes, 4, "les quatre textes de la batterie");
        assert_eq!(refuses, 17, "tout le reste");
    }

    #[test]
    fn le_guillemetage_neutralise_le_typage_a_lecriture() {
        // Ce que l'ADR a mesuré, et pourquoi ça ne suffit pas : le guillemetage
        // règle le cas du fichier que Keystone ÉCRIT. Il ne règle pas celui que
        // l'humain édite pendant sept jours, et c'est l'humain qui écrira `off`.
        for ecrit in ["true", "off", "23410000", "08", "0x9", "2024-10-21"] {
            let cite = format!("\"{ecrit}\"");
            let brut = lit_un_scalaire(&cite).expect("un scalaire cité reste lisible");
            assert_eq!(
                brut,
                ScalaireBrut::Text((*ecrit).to_owned()),
                "« {cite} » n'est pas revenu en texte"
            );
        }
    }

    // ── La politique de champs, et les deux refus qu'elle porte ────────────

    #[test]
    fn une_cle_racine_inconnue_est_refusee() {
        // Une clé inattendue à la racine est une faute de frappe ou un format
        // plus récent. Dans les deux cas, l'ignorer en silence laisserait
        // croire que la déclaration a été prise en compte.
        let source = format!("{EN_TETE}desiredd:\n  un.chemin: true\n");
        let e = EtatDesire::lire(&source).expect_err("clé racine inconnue acceptée");

        let ErreurDeLecture::Document { detail } = &e else {
            panic!("mauvaise erreur : {e:?}");
        };
        assert!(
            detail.contains("desiredd"),
            "le détail ne nomme pas la clé : {detail}"
        );
        // La phrase destinée à l'utilisateur ne porte pas le détail technique :
        // ce sont deux champs, et c'est ce qui permet d'afficher l'un sans
        // l'autre.
        assert!(!e.to_string().contains("desiredd"), "{e}");
    }

    #[test]
    fn une_cle_en_double_est_refusee() {
        // C'est le mode de pourrissement classique d'un fichier de
        // configuration : une exception ajoutée en haut, oubliée, écrasée en
        // bas. La dernière gagnerait en silence.
        let source = format!(
            "{EN_TETE}desired:\n  \
             security.defender.realtime: true\n  \
             security.defender.realtime: false\n"
        );
        let e = EtatDesire::lire(&source).expect_err("clé en double acceptée");

        let ErreurDeLecture::Document { detail } = &e else {
            panic!("mauvaise erreur : {e:?}");
        };
        assert!(
            detail.contains("security.defender.realtime"),
            "le détail ne nomme pas la clé en double : {detail}"
        );
    }

    #[test]
    fn un_entete_inattendu_est_refuse_avant_toute_comparaison() {
        // Un champ dont la sémantique a changé se relit sans erreur et ment
        // ensuite. On refuse la version qu'on ne sait pas relire.
        for (entete, trouve) in [
            (
                "apiVersion: keystone/v2\nkind: Workstation\n",
                "keystone/v2",
            ),
            ("apiVersion: keystone/v1\nkind: Serveur\n", "Serveur"),
        ] {
            let source = format!("{entete}metadata:\n  name: WKS-EXEMPLE-01\n");
            let e = EtatDesire::lire(&source).expect_err("en-tête inattendu accepté");
            assert_eq!(
                e,
                ErreurDeLecture::Format {
                    trouve: trouve.to_owned(),
                    attendu: if trouve.starts_with("keystone") {
                        VERSION_DE_FORMAT
                    } else {
                        GENRE_ATTENDU
                    },
                }
            );
        }
    }

    // ── Le document dans son entier ────────────────────────────────────────

    #[test]
    fn un_document_complet_se_relit_dans_ses_trois_formes_de_desir() {
        let source = "\
apiVersion: keystone/v1
kind: Workstation

metadata:
  name: WKS-EXEMPLE-01
  inherits: ./base.yaml

desired:
  # security — posture de la plateforme
  security.defender.realtime: true
  security.services.windefend.startup: \"automatique\"
  security.platform.hvci_policy: true
  inventory.os.kernel: \"26200\"
  security.defender.asr_rules:
    - BlockOfficeChildProcess
    - BlockCredentialStealing
  security.defender.exclusions.paths:
    absent: true

acceptedDrift:
  - item: security.services.fax.startup
    reason: \"requis temporairement par le pilote du scanner du labo\"
    expires: 2026-10-15
    decidedBy: exemple
    decidedAt: \"2026-07-18T09:12:00+02:00\"
";
        let d = EtatDesire::lire(source).expect("document valide refusé");

        assert_eq!(d.metadata.name, "WKS-EXEMPLE-01");
        assert_eq!(d.metadata.inherits.as_deref(), Some("./base.yaml"));
        assert_eq!(d.desired.len(), 6);

        // Un booléen reste un booléen.
        assert_eq!(
            d.desired["security.defender.realtime"],
            ScalaireBrut::Bool(true)
        );
        // Un jeton cité reste un texte — et `26200` aussi, grâce aux guillemets.
        assert_eq!(
            d.desired["security.services.windefend.startup"],
            ScalaireBrut::Text("automatique".into())
        );
        assert_eq!(
            d.desired["inventory.os.kernel"],
            ScalaireBrut::Text("26200".into())
        );
        // Une liste reste une liste.
        assert_eq!(
            d.desired["security.defender.asr_rules"],
            ScalaireBrut::List(vec![
                "BlockOfficeChildProcess".into(),
                "BlockCredentialStealing".into(),
            ])
        );
        // Et le désir d'absence porte la forme objet décidée par l'ADR-0016,
        // pas l'étiquette `!absent` que l'ADR-0010 proposait : mesuré,
        // celle-ci n'est pas atteignable par un enum `untagged`.
        assert_eq!(
            d.desired["security.defender.exclusions.paths"],
            ScalaireBrut::Absent
        );

        assert_eq!(d.accepted_drift.len(), 1);
        assert_eq!(d.accepted_drift[0].expires.to_string(), "2026-10-15");
    }

    #[test]
    fn letiquette_absent_de_ladr_0010_nest_pas_atteignable() {
        // La mesure qui a fait corriger la décision n° 3 de l'ADR-0010 : une
        // étiquette YAML personnalisée ne traverse pas un enum `untagged`. Ce
        // test la garde, pour qu'on ne réintroduise pas la syntaxe en croyant
        // qu'elle marche.
        let source = format!("{EN_TETE}desired:\n  un.chemin: !absent\n");
        assert!(
            EtatDesire::lire(&source).is_err(),
            "« !absent » a été accepté : la syntaxe de l'ADR-0010 est revenue"
        );
    }

    #[test]
    fn un_ecart_accepte_exige_sa_raison_et_son_echeance() {
        // Exigence D2-06, tenue par le typage : ni `reason` ni `expires` n'est
        // optionnel. Sans elles, un fichier d'état pourrit en trois ans sous
        // une couche d'exceptions dont personne ne se rappelle le motif.
        for manquant in ["reason", "expires"] {
            let mut lignes = vec![
                "  - item: security.services.fax.startup",
                "    reason: \"pilote du scanner du labo\"",
                "    expires: 2026-10-15",
                "    decidedBy: exemple",
                "    decidedAt: \"2026-07-18T09:12:00+02:00\"",
            ];
            lignes.retain(|l| !l.trim_start().starts_with(manquant));
            let source = format!("{EN_TETE}acceptedDrift:\n{}\n", lignes.join("\n"));

            assert!(
                EtatDesire::lire(&source).is_err(),
                "un écart accepté sans « {manquant} » a été accepté"
            );
        }
    }
}

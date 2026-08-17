//! Confronter le fichier d'état désiré à ce qu'un scan a relevé — le cœur de `ks diff`.
//!
//! ## Pourquoi ce module vit dans la bibliothèque
//!
//! La coque de bureau a besoin **exactement** de ce chargement-là : sans lui,
//! tous les items lui arrivent sans désir, `Item::verdict()` répond
//! `NonContraint` partout, et sa vue Dérive affiche « sans objet » pour
//! toujours. Deux chargements écrits séparément dériveraient — et le jour où ils
//! dérivent, la CLI et l'écran ne disent plus la même chose du même fichier.
//!
//! ## Un scan est nécessaire pour typer le fichier
//!
//! Ce n'est pas une commodité d'implémentation : c'est la décision n° 5 de
//! l'ADR-0016. YAML 1.1 lit `off`, `no`, `n`, `on`, `yes`, `y` comme des
//! booléens et `0x9` comme le nombre 9 ; le scalaire est donc typé par la
//! **forme de la valeur constatée** de l'item du même chemin, dans les deux
//! sens. Un état désiré n'a de sens que confronté à une machine.
//!
//! ## Rien n'est appliqué tant que tout n'est pas typé
//!
//! Une déclaration mal typée refuse **le fichier**, pas seulement sa ligne. Un
//! chargement partiel laisserait croire à une comparaison complète, et
//! l'utilisateur lirait « aucun écart » sur un fichier dont la moitié n'a pas
//! été prise en compte. C'est la même règle que pour la lecture : tant qu'elle
//! échoue, rien n'est comparé.

use std::path::Path;

use ks_core::item::Verdict;
use ks_core::{Desire, ErreurDeTypage, Item};

use crate::etat_desire::{ErreurDeLecture, EtatDesire};

/// Ce qu'une tolérance déclarée vaut réellement, une fois confrontée à la machine.
///
/// **Une tolérance n'est jamais appliquée en silence.** Les trois qualifications
/// autres qu'[`Self::EnVigueur`] décrivent chacune une manière dont un fichier
/// d'état pourrit : une échéance dépassée que personne ne relit, une tolérance
/// qui ne couvre plus rien parce que l'écart a été résorbé, un chemin qui n'existe
/// plus sur la machine. Sans elles, les trois se taisent, et c'est le
/// pourrissement que D2-07 nomme comme la principale cause de configurations
/// devenues incompréhensibles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Qualification {
    /// L'item est en écart et la tolérance court encore.
    EnVigueur {
        /// Jours restants, l'échéance comprise. Vaut `0` le jour même.
        jours_restants: i64,
    },
    /// L'échéance est passée : l'écart est redevenu actif, **tout seul**.
    Echue {
        /// Depuis combien de jours.
        depuis_jours: i64,
    },
    /// L'item n'est pas en écart : la tolérance ne couvre rien.
    ///
    /// Ce n'est pas une faute — l'écart a pu être résorbé entre-temps —, mais
    /// une ligne inutile dans un fichier qu'on relira dans six mois. Elle se
    /// signale pour être retirée.
    SansObjet,
    /// Aucun item observé ne porte ce chemin.
    ///
    /// Même ambiguïté que pour une déclaration non observée, et pour la même
    /// raison : faute de frappe, ou item légitimement disparu. Les deux causes
    /// sont nommées, sans en choisir une.
    NonObservee,
}

/// Une tolérance déclarée, avec ce qu'elle vaut aujourd'hui.
#[derive(Debug, Clone)]
pub struct Tolerance {
    /// Le chemin visé.
    pub item: String,
    /// Pourquoi, tel qu'écrit dans le fichier.
    pub raison: String,
    /// Le dernier jour où elle vaut, inclus.
    pub echeance: chrono::NaiveDate,
    /// Ce qu'elle vaut réellement.
    pub qualification: Qualification,
}

/// Ce que la confrontation d'un fichier et d'un scan a produit.
#[derive(Debug, Clone, Default)]
pub struct Confrontation {
    /// Nombre de déclarations effectivement posées sur un item observé.
    pub declarees: usize,
    /// Les tolérances déclarées, chacune qualifiée.
    ///
    /// Dans l'ordre du fichier : c'est celui dans lequel l'utilisateur les
    /// relira, et le réordonner l'obligerait à chercher.
    pub tolerances: Vec<Tolerance>,
    /// Les chemins déclarés qu'aucun item observé ne porte.
    ///
    /// **Deux causes, indiscernables ici** : une faute de frappe dans le chemin,
    /// ou un item légitimement disparu — une distribution WSL supprimée, par
    /// exemple. Il n'existe pas de catalogue statique des chemins valides
    /// (ADR-0010) ; le seul référentiel est ce qu'un scan a observé. Les deux
    /// causes sont donc nommées à l'utilisateur, sans en choisir une.
    pub non_observes: Vec<String>,
}

/// Ce qui peut empêcher de charger un état désiré.
///
/// Le message destiné à l'utilisateur et le détail technique restent **deux
/// champs distincts** : c'est ce qui permet d'afficher l'un et de journaliser
/// l'autre.
#[derive(Debug, thiserror::Error)]
pub enum ErreurDeChargement {
    /// Le fichier n'existe pas.
    ///
    /// Variante à part, et non un cas d'erreur d'entrée-sortie : c'est l'état
    /// normal d'un poste qui n'a pas encore adopté son état, et l'appelant doit
    /// pouvoir le distinguer d'une panne de lecture pour proposer `ks import`.
    #[error(
        "Aucun fichier d'état désiré à « {chemin} », donc rien à confronter. \
         `ks import` écrit ce fichier depuis ce que Keystone lit de la machine, \
         sans qu'aucun formulaire soit à remplir."
    )]
    Absent {
        /// Le chemin cherché.
        chemin: String,
    },

    /// Le fichier existe mais n'a pas pu être lu.
    #[error(
        "Le fichier d'état désiré « {chemin} » n'a pas pu être ouvert, donc rien \
         n'a été comparé. Le détail technique nomme la cause."
    )]
    Fichier {
        /// Le chemin cherché.
        chemin: String,
        /// Ce que le système a répondu, mot pour mot.
        detail: String,
    },

    /// Le document n'est pas relisible — voir [`ErreurDeLecture`].
    #[error(transparent)]
    Lecture(#[from] ErreurDeLecture),

    /// Au moins une déclaration ne correspond pas à la forme de l'item.
    ///
    /// Le cas nommé par l'ADR-0016 : `off` écrit sans guillemets sur un item qui
    /// porte un texte arrive ici en booléen, parce que YAML l'a décidé avant que
    /// Keystone le voie.
    #[error(
        "{count} déclaration(s) ne correspondent pas à la forme de l'item observé, \
         donc rien n'a été comparé. Chaque ligne nomme l'item et la correction :\n{detail}",
        count = .erreurs.len(),
        detail = .erreurs.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n")
    )]
    Typage {
        /// Le détail, déclaration par déclaration.
        erreurs: Vec<ErreurDeTypage>,
    },
}

/// Lit un fichier d'état désiré depuis le disque.
///
/// # Erreurs
///
/// * [`ErreurDeChargement::Absent`] — le fichier n'existe pas ;
/// * [`ErreurDeChargement::Fichier`] — il existe et n'a pas pu être ouvert ;
/// * [`ErreurDeChargement::Lecture`] — clé racine inconnue, clé en double,
///   en-tête inattendu, ou YAML invalide.
pub fn charger(chemin: &Path) -> Result<EtatDesire, ErreurDeChargement> {
    let source = std::fs::read_to_string(chemin).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ErreurDeChargement::Absent {
                chemin: chemin.display().to_string(),
            }
        } else {
            ErreurDeChargement::Fichier {
                chemin: chemin.display().to_string(),
                detail: e.to_string(),
            }
        }
    })?;
    Ok(EtatDesire::lire(&source)?)
}

/// Pose sur chaque item observé le désir que le document déclare pour lui.
///
/// Les déclarations sont **toutes** typées avant qu'une seule soit posée : un
/// refus laisse l'inventaire exactement comme il était.
///
/// # Les tolérances ne changent aucun verdict
///
/// Elles sont qualifiées, jamais appliquées : un écart toléré **reste publié en
/// écart**. Le verdict dit le fait constaté, la tolérance dit la politique
/// décidée, et confondre les deux ferait disparaître de l'écran ce qu'on a
/// justement choisi de garder sous les yeux jusqu'à une date. C'est aussi ce qui
/// garantit qu'à l'échéance l'écart réapparaît sans qu'aucune commande soit
/// lancée : il n'avait jamais cessé d'être là.
///
/// `aujourd_hui` est un paramètre et non une lecture d'horloge, pour que les
/// bornes d'échéance soient éprouvables.
///
/// # Erreurs
///
/// [`ErreurDeChargement::Typage`] si une déclaration ne correspond pas à la
/// forme de la valeur constatée de l'item du même chemin.
pub fn confronter(
    document: &EtatDesire,
    items: &mut [Item],
    aujourd_hui: chrono::NaiveDate,
) -> Result<Confrontation, ErreurDeChargement> {
    let mut acceptees: Vec<(usize, Desire)> = Vec::new();
    let mut erreurs: Vec<ErreurDeTypage> = Vec::new();
    let mut non_observes: Vec<String> = Vec::new();

    for (chemin, brut) in &document.desired {
        let Some(rang) = items.iter().position(|i| &i.path == chemin) else {
            non_observes.push(chemin.clone());
            continue;
        };
        match Desire::contraindre(brut.clone(), chemin, &items[rang].observed) {
            Ok(desire) => acceptees.push((rang, desire)),
            Err(e) => erreurs.push(e),
        }
    }

    if !erreurs.is_empty() {
        return Err(ErreurDeChargement::Typage { erreurs });
    }

    let declarees = acceptees.len();
    for (rang, desire) in acceptees {
        items[rang].desired = Some(desire.into());
    }

    // Les tolérances se qualifient **après** que les désirs sont posés : il faut
    // le verdict pour savoir si une tolérance couvre quelque chose, et le verdict
    // n'existe qu'une fois l'item contraint.
    let tolerances = document
        .accepted_drift
        .iter()
        .map(|accepte| {
            let ecart = accepte.expires - aujourd_hui;
            let qualification = match items.iter().find(|i| i.path == accepte.item) {
                None => Qualification::NonObservee,
                Some(item) if verdict_publie(item) != Verdict::Ecart => Qualification::SansObjet,
                // L'échéance est inclusive : elle vaut encore tout son jour.
                Some(_) if ecart.num_days() >= 0 => Qualification::EnVigueur {
                    jours_restants: ecart.num_days(),
                },
                Some(_) => Qualification::Echue {
                    depuis_jours: -ecart.num_days(),
                },
            };
            Tolerance {
                item: accepte.item.clone(),
                raison: accepte.reason.texte().to_owned(),
                echeance: accepte.expires,
                qualification,
            }
        })
        .collect();

    Ok(Confrontation {
        declarees,
        tolerances,
        non_observes,
    })
}

/// Le verdict tel que `ks diff` le publie.
///
/// C'est [`Item::verdict`], à une précision près : **un item déclarable dont la
/// lecture a échoué est publié incomparable, même si rien ne le déclare.**
///
/// Les deux lectures sont vraies — personne ne contraint cet item, et rien ne
/// permet de le comparer — et c'est justement pourquoi il faut choisir. Le ranger
/// parmi les non contraints le noierait dans les quatre-vingts constats et
/// mesures que Keystone ne déclare jamais, alors que `ks import` vient de dire
/// qu'il n'a pas su le lire. Les deux commandes se contrediraient sur la même
/// machine, et l'échec de lecture disparaîtrait entre elles.
///
/// C'est le pendant exact de la correction de l'ADR-0008 sur `compliant` : ce
/// qu'on n'a pas su lire ne se range pas dans une catégorie rassurante.
///
/// Le `match` est **exhaustif sans bras `_`** : ajouter une valeur à [`Verdict`]
/// casse la compilation ici, donc oblige à décider ce que `ks diff` en publie.
#[must_use]
pub fn verdict_publie(item: &Item) -> Verdict {
    match item.verdict() {
        Verdict::NonContraint => {
            if item.nature.est_declarable() && !item.observed.est_constat() {
                Verdict::Incomparable {
                    raison: format!("valeur constatée : {}", item.observed),
                }
            } else {
                Verdict::NonContraint
            }
        }
        Verdict::Conforme => Verdict::Conforme,
        Verdict::Ecart => Verdict::Ecart,
        incomparable @ Verdict::Incomparable { .. } => incomparable,
    }
}

#[cfg(test)]
mod tests {
    use ks_core::{ItemValue, Nature};

    use super::*;
    use crate::emetteur;
    use crate::machine_de_reference;

    /// Une date civile littérale. Aucune horloge dans un chemin asserté.
    fn jour(litteral: &str) -> chrono::NaiveDate {
        litteral.parse().expect("date littérale valide")
    }

    /// Les quatre verdicts publiés, comptés.
    fn verdicts(items: &[Item]) -> (usize, usize, usize, usize) {
        let (mut non_contraints, mut conformes, mut ecarts, mut incomparables) = (0, 0, 0, 0);
        for item in items {
            match verdict_publie(item) {
                Verdict::NonContraint => non_contraints += 1,
                Verdict::Conforme => conformes += 1,
                Verdict::Ecart => ecarts += 1,
                Verdict::Incomparable { .. } => incomparables += 1,
            }
        }
        (non_contraints, conformes, ecarts, incomparables)
    }

    /// Le critère d'acceptation du lot, joué de bout en bout.
    ///
    /// `ks import` puis `ks diff` sur la même machine, sans rien toucher entre
    /// les deux : aucun écart, et les trois exclusions Defender publiées
    /// incomparables. Un import qui déformerait une valeur au passage
    /// produirait un écart ici, sur une machine qui n'a pas bougé.
    #[test]
    fn importer_puis_confronter_ne_produit_aucun_ecart() {
        let mut items = machine_de_reference::items();
        let emission = emetteur::emettre(&items, "WKS-EXEMPLE-01");

        let document = EtatDesire::lire(&emission.yaml).expect("Keystone doit relire son écriture");
        let bilan = confronter(&document, &mut items, jour("2026-08-17"))
            .expect("le typage refuse notre propre fichier");

        assert_eq!(bilan.declarees, emission.declarations);
        assert!(
            bilan.non_observes.is_empty(),
            "un chemin écrit depuis un scan doit exister dans ce scan : {:?}",
            bilan.non_observes
        );

        let (non_contraints, conformes, ecarts, incomparables) = verdicts(&items);
        assert_eq!(ecarts, 0, "un import qui déforme produirait un écart ici");
        assert_eq!(conformes, emission.declarations);
        assert_eq!(
            incomparables,
            emission.illisibles.len(),
            "les items que l'import n'a pas su lire doivent rester nommés"
        );
        assert_eq!(
            non_contraints + conformes + ecarts + incomparables,
            items.len(),
            "les quatre verdicts doivent couvrir exactement les items observés"
        );
    }

    /// Une seule valeur changée donne exactement un écart.
    ///
    /// La falsification du diff : ni zéro — le comparateur verrait tout pareil —
    /// ni deux — il verrait des écarts là où rien n'a bougé. La valeur est
    /// modifiée dans le **fichier**, à la ligne près, comme un humain le ferait.
    #[test]
    fn une_seule_valeur_modifiee_donne_exactement_un_ecart() {
        let mut items = machine_de_reference::items();
        let yaml = emetteur::emettre(&items, "WKS-EXEMPLE-01").yaml;

        let avant = "  security.defender.realtime: true";
        assert!(
            yaml.contains(avant),
            "la ligne visée n'existe plus :\n{yaml}"
        );
        let modifie = yaml.replace(avant, "  security.defender.realtime: false");
        assert_ne!(modifie, yaml, "la substitution n'a rien remplacé");

        let document = EtatDesire::lire(&modifie).expect("document valide");
        confronter(&document, &mut items, jour("2026-08-17")).expect("typage");

        let (_, _, ecarts, _) = verdicts(&items);
        assert_eq!(ecarts, 1, "une valeur changée, un écart — ni zéro, ni deux");

        let fautif = items
            .iter()
            .find(|i| verdict_publie(i) == Verdict::Ecart)
            .expect("l'écart vient d'être compté");
        assert_eq!(fautif.path, "security.defender.realtime");
    }

    /// Un item déclarable illisible reste nommé, même sans déclaration.
    #[test]
    fn un_item_declarable_illisible_est_publie_incomparable_sans_etre_declare() {
        let illisible = machine_de_reference::item(
            "security.defender.exclusions.paths",
            ks_core::Domain::Security,
            Nature::Reglage,
            ItemValue::illisible("accès refusé sans élévation"),
        );
        let verdict = verdict_publie(&illisible);
        let Verdict::Incomparable { raison } = &verdict else {
            panic!("un aveu de lecture rangé parmi les non contraints : {verdict:?}");
        };
        assert!(raison.contains("accès refusé"), "raison perdue : {raison}");

        // Et un constat non déclaré reste non contraint : Keystone ne contraint
        // que ce qui est écrit, et l'exception ci-dessus ne déborde pas.
        let constat = machine_de_reference::item(
            "inventory.os.kernel",
            ks_core::Domain::Inventory,
            Nature::Constat,
            ItemValue::Text("26200".into()),
        );
        assert_eq!(verdict_publie(&constat), Verdict::NonContraint);
    }

    /// Une déclaration mal typée refuse le fichier, pas seulement sa ligne.
    #[test]
    fn une_declaration_mal_typee_nen_laisse_appliquer_aucune_autre() {
        let mut items = machine_de_reference::items();
        let yaml = emetteur::emettre(&items, "WKS-EXEMPLE-01").yaml;

        // Le cas nommé par l'ADR-0016 : les guillemets retirés à la main sur un
        // item qui porte un jeton. YAML lit « off » comme un booléen avant que
        // Keystone le voie.
        let avant = "  security.services.windefend.startup: \"automatique\"";
        assert!(
            yaml.contains(avant),
            "la ligne visée n'existe plus :\n{yaml}"
        );
        let modifie = yaml.replace(avant, "  security.services.windefend.startup: off");
        assert_ne!(modifie, yaml, "la substitution n'a rien remplacé");

        let document = EtatDesire::lire(&modifie).expect("document valide");
        let e = confronter(&document, &mut items, jour("2026-08-17"))
            .expect_err("un booléen accepté sur un item texte");

        let ErreurDeChargement::Typage { erreurs } = &e else {
            panic!("mauvaise erreur : {e:?}");
        };
        assert_eq!(erreurs.len(), 1);
        assert_eq!(erreurs[0].chemin, "security.services.windefend.startup");

        // La phrase dit quoi faire, et rien n'a été posé sur l'inventaire.
        assert!(e.to_string().contains("Entourez"), "{e}");
        assert!(
            items.iter().all(|i| i.desired.is_none()),
            "des déclarations ont été appliquées malgré le refus"
        );
    }

    /// Un chemin déclaré qu'aucun item ne porte se dit, avec ses deux causes.
    #[test]
    fn un_chemin_declare_mais_non_observe_est_nomme() {
        let mut items = machine_de_reference::items();
        let yaml = emetteur::emettre(&items, "WKS-EXEMPLE-01").yaml;
        let modifie = yaml.replace(
            "\nacceptedDrift: []",
            "\n  security.defender.realtim: true\n\nacceptedDrift: []",
        );
        assert_ne!(modifie, yaml, "la substitution n'a rien remplacé");

        let document = EtatDesire::lire(&modifie).expect("document valide");
        let bilan = confronter(&document, &mut items, jour("2026-08-17")).expect("typage");

        assert_eq!(
            bilan.non_observes,
            vec!["security.defender.realtim".to_owned()]
        );
        // Une faute de frappe n'échappe pas seulement au typage : elle ne
        // produit aucun écart non plus, faute d'item à comparer.
        let (_, _, ecarts, _) = verdicts(&items);
        assert_eq!(ecarts, 0);
    }

    // ── Les tolérances : qualifiées, jamais appliquées ─────────────────────

    /// Le chemin sur lequel les essais de tolérance portent tous.
    const TOLERE: &str = "security.services.windefend.startup";

    /// Un document fabriqué depuis la machine de référence, où l'item [`TOLERE`]
    /// est déclaré à une valeur qui **ne correspond pas**, et donc en écart, plus
    /// une tolérance sur le chemin `vise` échéant à `echeance`.
    fn avec_tolerance(vise: &str, echeance: &str) -> (Vec<Item>, EtatDesire) {
        let items = machine_de_reference::items();
        let yaml = emetteur::emettre(&items, "WKS-EXEMPLE-01").yaml;
        let en_ecart = yaml.replace(
            &format!("{TOLERE}: \"automatique\""),
            &format!("{TOLERE}: \"desactive\""),
        );
        assert_ne!(en_ecart, yaml, "la substitution n'a pas créé d'écart");

        let avec = en_ecart.replace(
            "\nacceptedDrift: []",
            &format!(
                "\nacceptedDrift:\n  - item: {vise}\n    \
                 reason: \"pilote du scanner du labo\"\n    \
                 expires: {echeance}\n    \
                 decidedBy: exemple\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"",
            ),
        );
        assert_ne!(avec, en_ecart, "la tolérance n'a pas été insérée");

        (items, EtatDesire::lire(&avec).expect("document valide"))
    }

    #[test]
    fn un_ecart_tolere_reste_publie_en_ecart() {
        // **Le cœur de la décision.** Le verdict dit le fait, la tolérance dit la
        // politique. Faire disparaître l'item de la liste des écarts ferait
        // sortir de l'écran ce qu'on a justement choisi de garder sous les yeux
        // jusqu'à une date — et à l'échéance, il faudrait le faire réapparaître,
        // alors qu'ici il n'aura jamais cessé d'être là.
        let (mut items, document) = avec_tolerance(TOLERE, "2026-10-15");
        let bilan = confronter(&document, &mut items, jour("2026-08-17")).expect("typage");

        let (_, _, ecarts, _) = verdicts(&items);
        assert_eq!(ecarts, 1, "l'écart toléré a disparu des écarts");
        assert_eq!(
            bilan.tolerances[0].qualification,
            Qualification::EnVigueur { jours_restants: 59 }
        );
    }

    #[test]
    fn une_tolerance_ne_deplace_jamais_un_item_vers_les_conformes() {
        // La contre-épreuve du test précédent, et elle dit autre chose : non
        // seulement l'écart reste un écart, mais AUCUN compte ne bouge. Un
        // décompte identique avec et sans la tolérance est la seule façon de
        // prouver qu'elle n'a rien déplacé, plutôt que déplacé deux fois.
        let (mut sans, doc_sans) = avec_tolerance(TOLERE, "2026-10-15");
        // Un document identique, tolérance retirée.
        let vide = EtatDesire::lire(
            &emetteur::emettre(&machine_de_reference::items(), "WKS-EXEMPLE-01")
                .yaml
                .replace(
                    &format!("{TOLERE}: \"automatique\""),
                    &format!("{TOLERE}: \"desactive\""),
                ),
        )
        .expect("document valide");

        let mut avec = machine_de_reference::items();
        confronter(&doc_sans, &mut avec, jour("2026-08-17")).expect("typage");
        confronter(&vide, &mut sans, jour("2026-08-17")).expect("typage");

        assert_eq!(
            verdicts(&avec),
            verdicts(&sans),
            "la tolérance a déplacé un item d'une catégorie à une autre"
        );
    }

    #[test]
    fn une_tolerance_echue_se_signale_plutot_que_de_se_taire() {
        // À l'échéance, l'écart redevient actif SANS commande : il n'a jamais
        // cessé de l'être. Ce qui change, c'est que la tolérance se dénonce
        // elle-même comme périmée, faute de quoi elle resterait dans le fichier
        // pour trois ans sans que personne ne la relise.
        let (mut items, document) = avec_tolerance(TOLERE, "2026-08-16");
        let bilan = confronter(&document, &mut items, jour("2026-08-17")).expect("typage");
        assert_eq!(
            bilan.tolerances[0].qualification,
            Qualification::Echue { depuis_jours: 1 }
        );

        // Et la veille de ce jour-là, elle valait encore : les deux bornes, sans
        // quoi la convention d'inclusivité ne serait vérifiée qu'à moitié.
        let (mut items, document) = avec_tolerance(TOLERE, "2026-08-16");
        let bilan = confronter(&document, &mut items, jour("2026-08-16")).expect("typage");
        assert_eq!(
            bilan.tolerances[0].qualification,
            Qualification::EnVigueur { jours_restants: 0 }
        );
    }

    #[test]
    fn une_tolerance_sur_un_item_conforme_ne_refuse_pas_le_fichier() {
        // Elle ne couvre rien — l'écart a pu être résorbé entre-temps —, et ce
        // n'est pas une faute : c'est une ligne à retirer. La refuser
        // empêcherait de démarrer sur un fichier parfaitement sain.
        let items = machine_de_reference::items();
        let yaml = emetteur::emettre(&items, "WKS-EXEMPLE-01").yaml;
        let avec = yaml.replace(
            "\nacceptedDrift: []",
            &format!(
                "\nacceptedDrift:\n  - item: {TOLERE}\n    \
                 reason: \"pilote du scanner du labo\"\n    \
                 expires: 2026-10-15\n    \
                 decidedBy: exemple\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"",
            ),
        );
        let document = EtatDesire::lire(&avec).expect("document valide");
        let mut items = items;
        let bilan = confronter(&document, &mut items, jour("2026-08-17")).expect("typage");

        assert_eq!(bilan.tolerances[0].qualification, Qualification::SansObjet);
        let (_, _, ecarts, _) = verdicts(&items);
        assert_eq!(ecarts, 0);
    }

    #[test]
    fn une_tolerance_sur_un_chemin_disparu_se_signale() {
        // Le mode de pourrissement le plus discret : le chemin ne correspond
        // plus à rien, la ligne reste, et rien ne la dénonce.
        let (mut items, document) =
            avec_tolerance("security.services.fantome.startup", "2026-10-15");
        let bilan = confronter(&document, &mut items, jour("2026-08-17")).expect("typage");
        assert_eq!(
            bilan.tolerances[0].qualification,
            Qualification::NonObservee
        );
    }

    /// Un fichier absent se distingue d'un fichier illisible.
    #[test]
    fn un_fichier_absent_oriente_vers_import() {
        let nulle_part = std::env::temp_dir().join("ks-aucun-etat-desire-nexiste-ici.yaml");
        let _ = std::fs::remove_file(&nulle_part);

        let e = charger(&nulle_part).expect_err("un fichier absent a été chargé");
        assert!(matches!(e, ErreurDeChargement::Absent { .. }), "{e:?}");
        assert!(e.to_string().contains("ks import"), "{e}");
    }
}

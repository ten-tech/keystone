//! Tolérer un écart, avec sa raison et son échéance — ce que `ks accept` calcule.
//!
//! Exigences D2-06 (raison et expiration obligatoires) et D2-07 (journal des
//! décisions), tranchées par l'ADR-0020 et son amendement.
//!
//! ## Deux endroits, deux questions
//!
//! Une acceptation vit dans **deux** endroits, et ce n'est pas une redondance :
//!
//! * `workstation.yaml` répond à « quelles tolérances sont en vigueur ». Il est
//!   versionné en git, relu par un humain, et seul consulté pour calculer un
//!   verdict. C'est lui qui survit à un clone du dépôt sur une machine neuve.
//! * le journal chaîné répond à « qui a décidé quoi, quand, pourquoi ». Il n'est
//!   jamais lu pour décider d'un verdict, et il ne survit pas à une
//!   réinstallation — d'où l'autre.
//!
//! ## Pourquoi ce module ne lit ni n'écrit de fichier
//!
//! Il calcule et fabrique du texte ; c'est la commande qui ouvre les fichiers.
//! La différence n'est pas cosmétique : elle rend éprouvable, sans disque et
//! sans magasin, la seule chose difficile de ce lot — que le texte inséré se
//! relise, et que rien d'autre n'ait bougé dans le document.
//!
//! ## Ce que l'insertion promet
//!
//! **Tout ce qui n'est pas le bloc ajouté reste octet pour octet identique.**
//! Réémettre le document entier aurait été plus simple, et aurait perdu du
//! *contenu*, pas seulement de la mise en forme : l'émetteur ne sait produire ni
//! `inherits`, ni `owner`, ni `description`, ni les tolérances déjà présentes.
//! La promesse est vérifiée par `linsertion_ne_touche_que_le_bloc_accepted_drift`,
//! qui retire le bloc du résultat et compare les octets restants à l'original.

use std::fmt::Write as _;

use crate::emetteur::citer;

/// Une décision de tolérance, telle qu'on la demande à Keystone.
#[derive(Debug, Clone)]
pub struct Decision {
    /// Le chemin de l'item toléré.
    pub item: String,
    /// Pourquoi. Ni vide ni blanche — c'est [`Decision::nouvelle`] qui le tient.
    pub raison: String,
    /// Le dernier jour où la tolérance vaut, inclus.
    pub echeance: chrono::NaiveDate,
    /// Qui décide.
    pub decideur: String,
    /// Quand la décision est prise.
    pub decide_le: ks_core::Timestamp,
}

/// Ce qui peut empêcher de tolérer un écart.
///
/// Chaque variante dit ce qui s'est passé, ce que ça implique, et ce qu'on peut
/// faire — dans cet ordre, et sans reproche.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refus {
    /// La raison est vide ou ne contient que des blancs.
    #[error(
        "Une tolérance sans raison est une exception permanente : dans six mois, \
         personne ne saura s'il faut la reconduire. Écrivez pourquoi cet écart \
         est accepté, en une phrase."
    )]
    RaisonVide,

    /// L'échéance est déjà passée.
    #[error(
        "L'échéance du {echeance} est antérieure au {aujourd_hui} : la tolérance \
         serait échue avant d'exister, donc sans effet. Choisissez une date à \
         venir, ou aujourd'hui."
    )]
    EcheanceDepassee {
        /// Ce qui a été demandé.
        echeance: chrono::NaiveDate,
        /// Le jour de la demande.
        aujourd_hui: chrono::NaiveDate,
    },

    /// Aucun item observé ne porte ce chemin.
    #[error(
        "Aucun item observé ne porte le chemin « {item} ». Rien n'a été écrit : \
         soit le chemin comporte une faute de frappe, soit l'item a disparu de \
         cette machine. `ks scan` liste les chemins tels que Keystone les nomme."
    )]
    ItemNonObserve {
        /// Le chemin demandé.
        item: String,
    },

    /// L'item n'est pas en écart : il n'y a rien à tolérer.
    #[error(
        "L'item « {item} » n'est pas en écart aujourd'hui ({verdict}). Une \
         tolérance ne couvrirait rien, et resterait dans le fichier sans que \
         personne sache pourquoi. `ks diff` liste les écarts en cours."
    )]
    RienATolerer {
        /// Le chemin demandé.
        item: String,
        /// Le verdict constaté, en toutes lettres.
        verdict: String,
    },

    /// Une tolérance en vigueur existe déjà sur ce chemin.
    #[error(
        "« {item} » est déjà toléré jusqu'au {echeance}, au motif suivant : \
         {raison}. Rien n'a été écrit : deux tolérances sur un même chemin \
         rendent le fichier illisible. Modifiez la ligne existante, ou retirez-la \
         d'abord."
    )]
    DejaTolere {
        /// Le chemin demandé.
        item: String,
        /// L'échéance en vigueur.
        echeance: chrono::NaiveDate,
        /// La raison en vigueur.
        raison: String,
    },
}

impl Decision {
    /// Construit une décision, en refusant ce qui n'en est pas une.
    ///
    /// Les deux refus portés ici sont ceux qui ne dépendent **que** de la
    /// demande, pas de la machine : ils se prononcent sans scan, donc avant tout
    /// travail coûteux.
    ///
    /// # Erreurs
    ///
    /// * [`Refus::RaisonVide`] — la raison est vide ou blanche ;
    /// * [`Refus::EcheanceDepassee`] — l'échéance est antérieure à aujourd'hui.
    pub fn nouvelle(
        item: &str,
        raison: &str,
        echeance: chrono::NaiveDate,
        decideur: &str,
        aujourd_hui: chrono::NaiveDate,
        decide_le: ks_core::Timestamp,
    ) -> Result<Self, Refus> {
        if raison.trim().is_empty() {
            return Err(Refus::RaisonVide);
        }
        if echeance < aujourd_hui {
            return Err(Refus::EcheanceDepassee {
                echeance,
                aujourd_hui,
            });
        }
        Ok(Self {
            item: item.to_owned(),
            raison: raison.to_owned(),
            echeance,
            decideur: decideur.to_owned(),
            decide_le,
        })
    }

    /// Le bloc YAML de cette tolérance, tel qu'il s'insère sous `acceptedDrift:`.
    ///
    /// Tout texte est cité, sans exception ni heuristique, par le même
    /// [`citer`] que l'émetteur : une raison contenant `off`, un deux-points ou
    /// un guillemet doit se relire telle qu'elle a été écrite, et une liste de
    /// valeurs dangereuses écrite à la main ne détecte jamais ce qu'on a oublié
    /// d'y mettre.
    ///
    /// La date et l'horodatage sont cités eux aussi : `2026-10-15` non cité se
    /// relit en date YAML, ce qui tombe juste ici par chance, et l'horodatage non
    /// cité se relit en horodatage YAML, ce qui ne tombe pas juste partout. On ne
    /// s'appuie pas sur la chance.
    #[must_use]
    pub fn bloc(&self) -> String {
        let mut bloc = String::new();
        let _ = writeln!(bloc, "  - item: {}", citer(&self.item));
        let _ = writeln!(bloc, "    reason: {}", citer(&self.raison));
        let _ = writeln!(bloc, "    expires: {}", self.echeance);
        let _ = writeln!(bloc, "    decidedBy: {}", citer(&self.decideur));
        let _ = writeln!(
            bloc,
            "    decidedAt: {}",
            citer(&self.decide_le.to_rfc3339())
        );
        bloc
    }
}

/// La clé de la liste des tolérances, telle qu'elle s'écrit dans le document.
const CLE: &str = "acceptedDrift:";

/// La forme que prend cette clé quand la liste est vide, telle que l'émetteur l'écrit.
const CLE_VIDE: &str = "acceptedDrift: []";

/// Insère un bloc de tolérance dans un document, sans toucher au reste.
///
/// Trois formes de document sont traitées, et ce sont les trois qui existent :
/// la liste vide écrite par `ks import`, une liste déjà garnie, et un document
/// où la clé manque tout court.
///
/// # Ce que la fonction garantit
///
/// Le document rendu est l'original avec le bloc inséré, et **rien d'autre** :
/// commentaires, ordre des clés, indentation et fins de ligne sont préservés.
/// Les fins de ligne sont détectées plutôt que supposées, faute de quoi une
/// insertion en `\n` dans un fichier en CRLF produirait un document mixte.
#[must_use]
pub fn inserer(source: &str, bloc: &str) -> String {
    // La fin de ligne se mesure sur le document, elle ne se suppose pas : le
    // dépôt impose LF, mais `workstation.yaml` vit chez l'utilisateur et peut
    // avoir été enregistré par un éditeur Windows.
    let crlf = source.contains("\r\n");
    let bloc = if crlf {
        bloc.replace('\n', "\r\n")
    } else {
        bloc.to_owned()
    };
    let saut = if crlf { "\r\n" } else { "\n" };

    // Cas 1 — la liste vide. C'est la forme que `ks import` écrit, donc le cas
    // courant. On remplace la ligne entière, pas la sous-chaîne : `acceptedDrift:
    // []` pourrait apparaître dans un commentaire, et le remplacer là produirait
    // un document que plus rien ne relit.
    if let Some(rang) = position_de_ligne(source, CLE_VIDE) {
        let mut lignes: Vec<String> = source.split(saut).map(str::to_owned).collect();
        lignes[rang] = format!("{CLE}{saut}{}", bloc.trim_end_matches(saut));
        return lignes.join(saut);
    }

    // Cas 2 — la liste est déjà garnie. Le bloc s'ajoute à la fin de la liste,
    // et non au début : l'ordre du fichier est celui dans lequel les décisions
    // ont été prises, et le conserver rend la relecture chronologique.
    if let Some(rang) = position_de_ligne(source, CLE) {
        let lignes: Vec<&str> = source.split(saut).collect();
        // La liste s'arrête à la première ligne qui n'est ni vide ni indentée.
        let fin = lignes
            .iter()
            .enumerate()
            .skip(rang + 1)
            .find(|(_, l)| !l.trim().is_empty() && !l.starts_with([' ', '\t']))
            .map_or(lignes.len(), |(i, _)| i);
        let mut sortie: Vec<String> = lignes[..fin].iter().map(|l| (*l).to_owned()).collect();
        // Les lignes vides en queue de liste appartiennent visuellement à ce qui
        // suit : le bloc se pose avant elles.
        let mut pose = sortie.len();
        while pose > rang + 1 && sortie[pose - 1].trim().is_empty() {
            pose -= 1;
        }
        for (decalage, ligne) in bloc.trim_end_matches(saut).split(saut).enumerate() {
            sortie.insert(pose + decalage, ligne.to_owned());
        }
        sortie.extend(lignes[fin..].iter().map(|l| (*l).to_owned()));
        return sortie.join(saut);
    }

    // Cas 3 — la clé manque. Elle s'ajoute à la fin, avec sa ligne vide de
    // séparation, comme l'émetteur l'écrit.
    let mut sortie = source.to_owned();
    if !sortie.ends_with(saut) {
        sortie.push_str(saut);
    }
    let _ = write!(sortie, "{saut}{CLE}{saut}{bloc}");
    sortie
}

/// Le rang de la première ligne qui est exactement `attendu`, blancs de fin exclus.
///
/// La comparaison porte sur la **ligne**, jamais sur une sous-chaîne : un
/// commentaire qui citerait la clé ne doit pas être pris pour elle.
fn position_de_ligne(source: &str, attendu: &str) -> Option<usize> {
    source
        .split(if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        })
        .position(|l| l.trim_end() == attendu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etat_desire::EtatDesire;

    fn jour(litteral: &str) -> chrono::NaiveDate {
        litteral.parse().expect("date littérale valide")
    }

    fn instant() -> ks_core::Timestamp {
        chrono::DateTime::parse_from_rfc3339("2026-08-17T09:12:00+02:00")
            .expect("horodatage littéral valide")
            .with_timezone(&chrono::Utc)
    }

    fn decision(raison: &str) -> Decision {
        Decision::nouvelle(
            "security.services.fax.startup",
            raison,
            jour("2026-10-15"),
            "tene",
            jour("2026-08-17"),
            instant(),
        )
        .expect("décision valide")
    }

    /// Un document complet, avec commentaires, métadonnées et liste vide.
    const DOCUMENT: &str = "\
# Écrit par Keystone. Les textes portent des guillemets : sans eux, YAML relit
# « off » comme un booléen.
apiVersion: keystone/v1
kind: Workstation

metadata:
  name: \"WKS-EXEMPLE-01\"
  owner: \"tene\"
  description: \"poste d'ingénierie\"

desired:

  # security — posture de la plateforme
  security.defender.realtime: true
  security.services.fax.startup: \"desactive\"

acceptedDrift: []
";

    #[test]
    fn une_raison_vide_ou_blanche_est_refusee() {
        for vide in ["", "   ", "\t\n "] {
            let e = Decision::nouvelle(
                "un.chemin",
                vide,
                jour("2026-10-15"),
                "tene",
                jour("2026-08-17"),
                instant(),
            )
            .err();
            assert_eq!(
                e,
                Some(Refus::RaisonVide),
                "« {vide:?} » accepté comme raison"
            );
        }
    }

    #[test]
    fn une_echeance_deja_passee_est_refusee() {
        // Elle serait échue avant d'exister, donc sans effet. La refuser tout de
        // suite vaut mieux que d'écrire une ligne morte.
        let e = Decision::nouvelle(
            "un.chemin",
            "raison",
            jour("2026-08-16"),
            "tene",
            jour("2026-08-17"),
            instant(),
        )
        .expect_err("une échéance passée a été acceptée");
        assert!(matches!(e, Refus::EcheanceDepassee { .. }), "{e:?}");

        // Le jour même passe : l'échéance est inclusive, ici comme ailleurs.
        assert!(Decision::nouvelle(
            "un.chemin",
            "raison",
            jour("2026-08-17"),
            "tene",
            jour("2026-08-17"),
            instant()
        )
        .is_ok());
    }

    #[test]
    fn le_bloc_insere_est_relu_par_keystone() {
        // **La barrière qui compte.** Un bloc joliment formé mais que le lecteur
        // du produit refuse ne vaut rien : c'est le fichier de l'utilisateur qu'il
        // casserait. On insère, on relit avec le lecteur réel, et on retrouve la
        // tolérance demandée.
        let d = decision("pilote du scanner du labo");
        let sortie = inserer(DOCUMENT, &d.bloc());

        let document = EtatDesire::lire(&sortie).expect("Keystone refuse ce qu'il vient d'écrire");
        assert_eq!(document.accepted_drift.len(), 1);
        let t = &document.accepted_drift[0];
        assert_eq!(t.item, "security.services.fax.startup");
        assert_eq!(t.reason.texte(), "pilote du scanner du labo");
        assert_eq!(t.expires, jour("2026-10-15"));
        assert_eq!(t.decided_by, "tene");
    }

    #[test]
    fn une_raison_piegeuse_se_relit_telle_quelle() {
        // Ce que le guillemetage systématique protège, éprouvé sur ce qui casse
        // vraiment : un deux-points suivi d'un espace coupe la ligne en deux,
        // un guillemet ferme la chaîne au milieu, une contre-oblique disparaît,
        // et « off » revient en booléen.
        for piegeuse in [
            "off",
            "raison : avec un deux-points",
            "il a dit \"non\"",
            r"chemin C:\Program Files\Truc",
            "26200",
            "- pas une liste",
            "#pas un commentaire",
        ] {
            let sortie = inserer(DOCUMENT, &decision(piegeuse).bloc());
            let document = EtatDesire::lire(&sortie)
                .unwrap_or_else(|e| panic!("« {piegeuse} » casse le document : {e}"));
            assert_eq!(
                document.accepted_drift[0].reason.texte(),
                piegeuse,
                "« {piegeuse} » n'est pas revenue telle quelle"
            );
        }
    }

    #[test]
    fn linsertion_ne_touche_que_le_bloc_accepted_drift() {
        // **La promesse de ce module, vérifiée par soustraction.** On retire du
        // résultat exactement les lignes ajoutées, et ce qui reste doit être
        // l'original, octet pour octet — commentaires, métadonnées, lignes vides
        // et indentation comprises.
        let d = decision("pilote du scanner du labo");
        let sortie = inserer(DOCUMENT, &d.bloc());

        let bloc = d.bloc();
        let mut a_retirer: std::collections::VecDeque<&str> = bloc.lines().collect();
        assert!(!a_retirer.is_empty(), "le bloc n'a aucune ligne");

        let mut restant: Vec<&str> = Vec::new();
        for ligne in sortie.lines() {
            if a_retirer.front().is_some_and(|l| *l == ligne) {
                a_retirer.pop_front();
                continue;
            }
            restant.push(ligne);
        }
        assert!(
            a_retirer.is_empty(),
            "toutes les lignes du bloc n'ont pas été retrouvées : {a_retirer:?}"
        );

        // `acceptedDrift: []` est devenue `acceptedDrift:` : c'est la seule
        // modification tolérée hors ajout, et elle est nommée ici plutôt que
        // laissée passer par une comparaison approximative.
        let attendu: Vec<&str> = DOCUMENT.lines().collect();
        let recompose: Vec<String> = restant
            .iter()
            .map(|l| {
                if *l == CLE {
                    CLE_VIDE.to_owned()
                } else {
                    (*l).to_owned()
                }
            })
            .collect();
        assert_eq!(recompose, attendu, "l'insertion a modifié autre chose");
    }

    #[test]
    fn une_seconde_tolerance_sajoute_a_la_suite_de_la_premiere() {
        // Le cas 2 de l'insertion, et l'ordre : la liste est chronologique, donc
        // la nouvelle va en queue. Le fichier se relit ensuite comme l'historique
        // des décisions prises.
        let premiere = inserer(DOCUMENT, &decision("première raison").bloc());
        let seconde = Decision::nouvelle(
            "security.defender.realtime",
            "seconde raison",
            jour("2026-11-30"),
            "tene",
            jour("2026-08-17"),
            instant(),
        )
        .expect("décision valide");
        let sortie = inserer(&premiere, &seconde.bloc());

        let document = EtatDesire::lire(&sortie).expect("document valide");
        assert_eq!(document.accepted_drift.len(), 2);
        assert_eq!(document.accepted_drift[0].reason.texte(), "première raison");
        assert_eq!(document.accepted_drift[1].reason.texte(), "seconde raison");
    }

    #[test]
    fn un_document_sans_la_cle_la_recoit() {
        // Le cas 3. `acceptedDrift` est écrite par `ks import`, mais un fichier
        // rédigé à la main peut ne pas la porter, et refuser à ce moment-là
        // obligerait l'utilisateur à deviner la syntaxe.
        let sans = DOCUMENT.replace("\nacceptedDrift: []\n", "\n");
        assert!(
            !sans.contains("acceptedDrift"),
            "la clé n'a pas été retirée"
        );

        let sortie = inserer(&sans, &decision("raison").bloc());
        let document = EtatDesire::lire(&sortie).expect("document valide");
        assert_eq!(document.accepted_drift.len(), 1);
    }

    #[test]
    fn un_document_en_crlf_reste_en_crlf() {
        // `workstation.yaml` vit chez l'utilisateur, pas dans ce dépôt : il peut
        // avoir été enregistré par un éditeur Windows. Insérer en LF produirait
        // un document mixte, que git signalerait à chaque ligne.
        let crlf = DOCUMENT.replace('\n', "\r\n");
        let sortie = inserer(&crlf, &decision("raison").bloc());

        assert!(
            !sortie.replace("\r\n", "").contains('\n'),
            "l'insertion a laissé des fins de ligne en LF dans un document CRLF"
        );
        EtatDesire::lire(&sortie).expect("document CRLF valide");
    }

    #[test]
    fn une_cle_citee_dans_un_commentaire_nest_pas_prise_pour_la_vraie() {
        // La comparaison porte sur la ligne entière, pas sur une sous-chaîne :
        // sinon un commentaire d'explication déplacerait l'insertion, et le
        // document deviendrait illisible sans qu'aucun test ne le voie.
        let avec_commentaire = DOCUMENT.replace(
            "acceptedDrift: []",
            "# la liste ci-dessous s'écrit acceptedDrift: []\nacceptedDrift: []",
        );
        let sortie = inserer(&avec_commentaire, &decision("raison").bloc());

        let document = EtatDesire::lire(&sortie).expect("document valide");
        assert_eq!(document.accepted_drift.len(), 1);
        assert!(
            sortie.contains("# la liste ci-dessous s'écrit acceptedDrift: []"),
            "le commentaire a été modifié"
        );
    }
}

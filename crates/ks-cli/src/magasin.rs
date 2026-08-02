//! Persistance du journal (Phase 0.5).
//!
//! ## Écrire son propre journal n'est pas écrire sur la machine
//!
//! La règle des phases 0 et 1 est absolue : **aucune écriture système**. Elle
//! vise la configuration du poste — registre, services, fichiers d'autrui. Le
//! journal de Keystone, lui, vit dans le dossier de données de Keystone, comme
//! le rapport HTML vit là où l'utilisateur le demande.
//!
//! La distinction est écrite ici pour qu'elle ne se perde pas, parce qu'elle est
//! exactement le genre de nuance qui se transforme en dérive : « on écrit déjà
//! le journal, alors un petit fichier de cache… ».
//!
//! ## Et pourtant, rien ne s'écrit par défaut
//!
//! `ks scan` ne journalise pas. Il faut `ks scan --record`.
//!
//! Ce n'est pas de la timidité, c'est le principe P2 appliqué à la lettre : il
//! n'existe volontairement pas de `--dry-run` dans ce produit, seulement
//! `--apply`, pour qu'on ne puisse pas écrire par omission. Un scan qui
//! journaliserait sans qu'on l'ait demandé contredirait sa propre bannière —
//! « lecture seule, aucune écriture système » — et le contredirait *en silence*,
//! ce qui est pire que de le contredire tout court.
//!
//! ## Le mode WAL, et sa contrepartie
//!
//! La base est ouverte en WAL : les lectures ne bloquent pas l'écriture, ce qui
//! compte le jour où une interface lira le journal pendant qu'un scan l'écrit.
//! La contrepartie est que deux fichiers annexes apparaissent à côté de la base.
//! C'est acceptable **ici**, parce que c'est notre dossier. Ce serait
//! inacceptable chez winget, d'où l'ouverture en lecture seule stricte du
//! lecteur de ses bases de suivi.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ks_core::{JournalEntry, GENESIS_DIGEST, GENESIS_SEQ};
use rusqlite::{Connection, OpenFlags};

/// Le dossier de données de Keystone.
///
/// Pas de dépendance pour ça : deux variables d'environnement et une règle par
/// plateforme suffisent, et une crate de plus sur un chemin aussi simple ne se
/// justifierait pas (YAGNI armé).
#[must_use]
pub fn dossier_donnees() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);

    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        });

    base.map(|b| b.join("Keystone"))
}

/// Le journal persisté.
pub struct Magasin {
    connexion: Connection,
}

impl Magasin {
    /// Ouvre le journal en écriture, en créant ce qu'il faut.
    ///
    /// Le dossier est créé s'il manque : c'est la seule création de dossier de
    /// tout le produit en Phase 0, et elle est cantonnée à `Keystone`.
    pub fn ouvrir(chemin: &Path) -> Result<Self> {
        if let Some(parent) = chemin.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("création de « {} »", parent.display()))?;
        }
        let connexion = Connection::open(chemin)
            .with_context(|| format!("ouverture de « {} »", chemin.display()))?;

        // WAL : une lecture concurrente ne bloque pas l'écriture. `NORMAL` suffit
        // ici — `FULL` coûterait une synchronisation disque par entrée pour une
        // garantie que seul un arrêt brutal du système rendrait utile, et un
        // journal amputé de sa dernière entrée reste détecté par le chaînage.
        connexion
            .pragma_update(None, "journal_mode", "WAL")
            .context("passage en mode WAL")?;
        connexion
            .pragma_update(None, "synchronous", "NORMAL")
            .context("réglage de la synchronisation")?;

        connexion
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS journal (
                     seq         INTEGER PRIMARY KEY,
                     at          TEXT NOT NULL,
                     verb        TEXT NOT NULL,
                     target      TEXT NOT NULL,
                     prev_digest TEXT NOT NULL,
                     digest      TEXT NOT NULL,
                     payload     TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS journal_at ON journal(at);",
            )
            .context("création du schéma du journal")?;

        Ok(Self { connexion })
    }

    /// Ouvre le journal en **lecture seule**, sans rien créer.
    ///
    /// `ks journal` n'a aucune raison de créer une base : si elle n'existe pas,
    /// c'est que rien n'a jamais été enregistré, et le dire vaut mieux que
    /// fabriquer un journal vide qui ressemblerait à un journal effacé.
    pub fn ouvrir_en_lecture(chemin: &Path) -> Result<Self> {
        let connexion = Connection::open_with_flags(
            chemin,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("lecture de « {} »", chemin.display()))?;
        Ok(Self { connexion })
    }

    /// L'empreinte et le numéro de la dernière entrée, ou l'ancrage de genèse.
    fn dernier_maillon(&self) -> Result<(u64, String)> {
        let mut requete = self
            .connexion
            .prepare("SELECT seq, digest FROM journal ORDER BY seq DESC LIMIT 1")?;
        let mut lignes = requete.query([])?;
        match lignes.next()? {
            Some(l) => {
                let seq: i64 = l.get(0)?;
                Ok((u64::try_from(seq).unwrap_or(0), l.get(1)?))
            }
            None => Ok((GENESIS_SEQ - 1, GENESIS_DIGEST.to_owned())),
        }
    }

    /// Ajoute une entrée à la suite, en la chaînant à la précédente.
    ///
    /// Le numéro de séquence et l'empreinte du prédécesseur sont **imposés
    /// ici**, jamais fournis par l'appelant : les laisser choisir permettrait
    /// d'insérer une entrée au milieu, ce que tout le chaînage existe pour
    /// empêcher.
    pub fn ajouter(&self, mut entree: JournalEntry) -> Result<JournalEntry> {
        let (dernier_seq, empreinte_precedente) = self.dernier_maillon()?;
        entree.seq = dernier_seq + 1;
        entree.prev_digest = empreinte_precedente;

        let empreinte = entree.digest();
        let charge = serde_json::to_string(&entree).context("sérialisation de l'entrée")?;

        self.connexion
            .execute(
                "INSERT INTO journal (seq, at, verb, target, prev_digest, digest, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    i64::try_from(entree.seq).unwrap_or(i64::MAX),
                    entree.at.to_rfc3339(),
                    entree.verb,
                    entree.target,
                    entree.prev_digest,
                    empreinte,
                    charge,
                ],
            )
            .context("écriture de l'entrée de journal")?;

        Ok(entree)
    }

    /// Toutes les entrées, dans l'ordre.
    pub fn lire(&self, depuis: Option<&str>) -> Result<Vec<JournalEntry>> {
        // La borne est comparée en RFC 3339, dont l'ordre lexicographique suit
        // l'ordre chronologique tant que le fuseau est le même. Les entrées sont
        // écrites en UTC, donc la condition tient.
        // Une borne toujours présente, dont la valeur par défaut ne filtre rien :
        // deux branches de requête produiraient deux types d'itérateurs distincts,
        // pour un gain nul sur ce volume.
        let borne = depuis.unwrap_or("0000-01-01T00:00:00Z");
        let mut requete = self
            .connexion
            .prepare("SELECT payload FROM journal WHERE at >= ?1 ORDER BY seq")?;
        let lignes = requete.query_map([borne], |l| l.get::<_, String>(0))?;

        let mut entrees = Vec::new();
        for charge in lignes {
            let brut = charge.context("lecture d'une entrée")?;
            entrees.push(serde_json::from_str(&brut).context("entrée de journal illisible")?);
        }
        Ok(entrees)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ks_core::{Actor, Outcome};

    fn entree(verbe: &str) -> JournalEntry {
        JournalEntry {
            seq: 0,
            at: chrono::Utc::now(),
            actor: Actor::System,
            verb: verbe.to_owned(),
            target: "poste".to_owned(),
            diff: None,
            outcome: Outcome::Observed,
            prev_digest: String::new(),
        }
    }

    fn magasin_neuf(nom: &str) -> (Magasin, PathBuf) {
        let chemin = std::env::temp_dir().join(format!("ks-journal-{nom}.sqlite"));
        for suffixe in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffixe}", chemin.display()));
        }
        let m = Magasin::ouvrir(&chemin).expect("journal ouvrable");
        (m, chemin)
    }

    #[test]
    fn la_premiere_entree_sancre_sur_la_genese() {
        let (m, chemin) = magasin_neuf("genese");
        let e = m.ajouter(entree("scan")).expect("ajout");
        assert_eq!(e.seq, GENESIS_SEQ);
        assert_eq!(e.prev_digest, GENESIS_DIGEST);
        assert!(JournalEntry::verify_chain(&m.lire(None).expect("lecture")));
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn la_chaine_tient_sur_plusieurs_entrees() {
        let (m, chemin) = magasin_neuf("chaine");
        for _ in 0..5 {
            m.ajouter(entree("scan")).expect("ajout");
        }
        let lues = m.lire(None).expect("lecture");
        assert_eq!(lues.len(), 5);
        assert!(
            JournalEntry::verify_chain(&lues),
            "le chaînage doit survivre à l'aller-retour par SQLite"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn lappelant_ne_choisit_ni_le_numero_ni_lancrage() {
        // Laisser l'appelant fournir ces deux champs permettrait d'insérer une
        // entrée au milieu de la chaîne — ce que tout le mécanisme existe pour
        // empêcher. Le magasin les impose, quoi qu'on lui passe.
        let (m, chemin) = magasin_neuf("impose");
        m.ajouter(entree("scan")).expect("premier");

        let mut menteuse = entree("scan");
        menteuse.seq = 9_999;
        menteuse.prev_digest = "blake3:0000".to_owned();
        let ecrite = m.ajouter(menteuse).expect("second");

        assert_eq!(ecrite.seq, 2, "le numéro est imposé, pas accepté");
        assert_ne!(ecrite.prev_digest, "blake3:0000");
        assert!(JournalEntry::verify_chain(&m.lire(None).expect("lecture")));
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_entree_reecrite_casse_la_chaine() {
        // La barrière, éprouvée en la franchissant. Une vérification qu'on n'a
        // jamais mise en défaut ne prouve rien : on réécrit donc réellement un
        // champ, en base, comme le ferait quelqu'un ayant accès au fichier.
        let (m, chemin) = magasin_neuf("falsifie");
        m.ajouter(entree("scan")).expect("premier");
        m.ajouter(entree("scan")).expect("second");
        assert!(JournalEntry::verify_chain(&m.lire(None).expect("lecture")));

        // On altère la charge utile de la première entrée sans toucher aux
        // empreintes stockées — exactement ce que fait une réécriture discrète.
        m.connexion
            .execute(
                "UPDATE journal SET payload = replace(payload, '\"target\":\"poste\"', \
                 '\"target\":\"autre\"') WHERE seq = 1",
                [],
            )
            .expect("altération");

        let lues = m.lire(None).expect("lecture");
        assert_eq!(lues.len(), 2, "l'entrée est toujours là, mais changée");
        assert!(
            !JournalEntry::verify_chain(&lues),
            "une entrée réécrite doit rompre le chaînage, sinon il ne sert à rien"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_entree_retiree_casse_aussi_la_chaine() {
        // L'autre moitié de la menace : ne rien changer, seulement effacer.
        let (m, chemin) = magasin_neuf("tronque");
        for _ in 0..3 {
            m.ajouter(entree("scan")).expect("ajout");
        }
        m.connexion
            .execute("DELETE FROM journal WHERE seq = 2", [])
            .expect("suppression");

        assert!(
            !JournalEntry::verify_chain(&m.lire(None).expect("lecture")),
            "un maillon manquant doit se voir"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn un_journal_inexistant_ne_se_fabrique_pas_a_la_lecture() {
        // Un journal vide fabriqué à la volée ressemblerait à un journal effacé.
        // Mieux vaut échouer et le dire.
        let absent = std::env::temp_dir().join("ks-journal-jamais-cree.sqlite");
        let _ = std::fs::remove_file(&absent);
        assert!(Magasin::ouvrir_en_lecture(&absent).is_err());
    }

    #[test]
    fn le_filtre_de_date_ne_ramene_pas_le_passe() {
        let (m, chemin) = magasin_neuf("depuis");
        m.ajouter(entree("scan")).expect("ajout");
        assert_eq!(
            m.lire(Some("2999-01-01T00:00:00Z")).expect("lecture").len(),
            0
        );
        assert_eq!(
            m.lire(Some("2000-01-01T00:00:00Z")).expect("lecture").len(),
            1
        );
        let _ = std::fs::remove_file(chemin);
    }
}

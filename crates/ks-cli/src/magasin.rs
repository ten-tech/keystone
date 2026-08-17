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
//!
//! ## Deux tables, un seul fichier
//!
//! La table `observation` rejoint `journal` dans la même base (ADR-0014). Elles
//! n'ont pourtant aucune propriété commune : le journal est **chaîné et jamais
//! purgé**, la série d'observations est **indépendante, interrogeable par chemin
//! et purgeable**. Un chaînage et une rétention ne peuvent pas coexister sur la
//! même table — supprimer un maillon rompt la chaîne, et une chaîne qu'on
//! accepte de rompre ne prouve plus rien.
//!
//! Ce qui les réunit est plus prosaïque, et suffisant : un seul fichier, un seul
//! mode WAL, un seul cycle de vie, une seule chose à sauvegarder.
//!
//! ## L'encodage est par intervalles, pas par échantillons
//!
//! Un scan qui revoit la même valeur **avance `last_seen`** et n'insère rien ;
//! un scan qui en voit une autre ferme l'intervalle et en ouvre un. La taille du
//! magasin devient donc proportionnelle au **changement**, pas au temps.
//!
//! Ce n'est pas une élégance gratuite : l'intervalle *est* la donnée dont
//! l'attribution a besoin. « Le changement a eu lieu entre 14 h 03 et 15 h 03 »
//! est tout ce qu'un sondage sait dire, et c'est exactement ce que ces deux
//! bornes portent. Toute présentation qui afficherait un instant unique
//! mentirait.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ks_core::{Change, ItemValue, JournalEntry, GENESIS_DIGEST, GENESIS_SEQ};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

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
                 CREATE INDEX IF NOT EXISTS journal_at ON journal(at);

                 CREATE TABLE IF NOT EXISTS observation (
                     id         INTEGER PRIMARY KEY,
                     path       TEXT    NOT NULL,
                     value      TEXT    NOT NULL,   -- JSON d'ItemValue
                     first_seen TEXT    NOT NULL,   -- premier scan qui a vu cette valeur
                     last_seen  TEXT    NOT NULL,   -- dernier scan qui l'a CONFIRMÉE
                     closed     INTEGER NOT NULL DEFAULT 0
                 );
                 -- Au plus un intervalle ouvert par chemin. L'invariant est
                 -- porté par l'index, pas par la vigilance de l'appelant : une
                 -- règle qu'on se contente de respecter finit par être oubliée
                 -- une fois, et une seule fois suffit à dédoubler une série.
                 CREATE UNIQUE INDEX IF NOT EXISTS observation_ouverte
                     ON observation(path) WHERE closed = 0;
                 CREATE INDEX IF NOT EXISTS observation_serie ON observation(path, first_seen);

                 -- Ce qu'on n'a pas su lire, et pourquoi. C'est cette table qui
                 -- rend un changement survenu pendant une cécité *expliqué*
                 -- plutôt que faussement attribué.
                 CREATE TABLE IF NOT EXISTS lecture_refusee (
                     id     INTEGER PRIMARY KEY,
                     at     TEXT NOT NULL,
                     path   TEXT NOT NULL,
                     raison TEXT NOT NULL
                 );",
            )
            .context("création du schéma du journal et des observations")?;

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
    ///
    /// Les deux gestes — lire le dernier maillon, puis insérer à sa suite — ne
    /// forment un maillon correct que s'ils sont indivisibles. `BEGIN IMMEDIATE`
    /// prend le verrou d'écriture dès l'ouverture plutôt qu'à la première
    /// écriture : deux `ks scan --record` concurrents s'attendent au lieu de
    /// lire le même prédécesseur puis de se heurter sur la clé primaire.
    pub fn ajouter(&self, entree: JournalEntry) -> Result<JournalEntry> {
        self.connexion
            .execute_batch("BEGIN IMMEDIATE")
            .context("ouverture de la transaction du journal")?;
        match self.ajouter_chaine(entree) {
            Ok(ecrite) => {
                self.connexion
                    .execute_batch("COMMIT")
                    .context("validation de l'entrée de journal")?;
                Ok(ecrite)
            }
            Err(erreur) => {
                // L'annulation ne doit pas masquer la cause première.
                let _ = self.connexion.execute_batch("ROLLBACK");
                Err(erreur)
            }
        }
    }

    /// Le chaînage proprement dit, sous le verrou tenu par [`Self::ajouter`].
    fn ajouter_chaine(&self, mut entree: JournalEntry) -> Result<JournalEntry> {
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

    /// Numéro de la première entrée dont l'empreinte stockée ne correspond plus
    /// à sa charge utile, s'il en existe une.
    ///
    /// La colonne `digest` était écrite à chaque insertion et **jamais relue** :
    /// un champ qui avait toute l'apparence d'une garantie, et qui ne servait à
    /// rien. Le chaînage seul ne suffit pas, parce qu'il relie chaque entrée à
    /// la **précédente** : la dernière n'est reliée à rien, donc sa réécriture
    /// passait la vérification sans laisser de trace.
    ///
    /// Ce contrôle ferme la réécriture naïve d'une charge utile, la dernière
    /// comprise. Il ne ferme pas — et rien de local ne peut fermer — la
    /// réécriture d'une entrée **avec** son empreinte, ni la suppression des
    /// dernières entrées : quiconque écrit dans le fichier peut produire une
    /// séquence plus courte et parfaitement cohérente. C'est exactement la
    /// raison d'être de l'exigence SEC-04, qui exporte le journal hors du poste.
    pub fn premiere_empreinte_incoherente(&self) -> Result<Option<u64>> {
        let mut requete = self
            .connexion
            .prepare("SELECT digest, payload FROM journal ORDER BY seq")?;
        let lignes =
            requete.query_map([], |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))?;

        for ligne in lignes {
            let (stockee, charge) = ligne.context("lecture d'une empreinte")?;
            let entree: JournalEntry =
                serde_json::from_str(&charge).context("entrée de journal illisible")?;
            if entree.digest() != stockee {
                return Ok(Some(entree.seq));
            }
        }
        Ok(None)
    }
}

/// Un intervalle d'observation : une valeur, et la fenêtre de scans qui l'ont
/// confirmée.
///
/// **Un intervalle n'est pas un instant.** Entre le `last_seen` d'un intervalle
/// et le `first_seen` du suivant, le magasin ne sait rien : il sait seulement
/// que le changement s'est produit quelque part là-dedans. Cette fenêtre peut
/// couvrir plusieurs jours si l'item est resté illisible entre les deux.
#[derive(Debug, Clone, PartialEq)]
pub struct Intervalle {
    /// Le chemin de l'item observé.
    pub path: String,
    /// La valeur relevée, telle qu'`ItemValue` la sérialise.
    pub value: ItemValue,
    /// Premier scan qui a vu cette valeur.
    pub first_seen: DateTime<Utc>,
    /// Dernier scan qui l'a **confirmée** — pas le dernier scan tout court.
    pub last_seen: DateTime<Utc>,
    /// L'intervalle est-il refermé, c'est-à-dire suivi d'une autre valeur ?
    pub closed: bool,
}

/// La série d'observations, séparée du journal parce que ce sont deux choses.
///
/// `#[allow(dead_code)]` assumé et daté : le câblage depuis `ks scan --record`
/// arrive dans un second temps. Sans cet attribut, le binaire — où les tests
/// sont absents — signalerait ces méthodes comme mortes. L'attribut disparaît
/// avec le premier appelant.
#[allow(dead_code)]
impl Magasin {
    /// Enregistre ce qu'un scan a relevé pour **un** item.
    ///
    /// L'horodatage est un paramètre et non une lecture d'horloge interne : une
    /// fonction qui appelle `Utc::now()` ne se teste pas deux fois de la même
    /// façon, et une série dont les bornes ne sont pas reproductibles ne se
    /// vérifie pas du tout.
    ///
    /// Trois issues, et une quatrième qui n'en est pas une :
    ///
    /// * même valeur que l'intervalle ouvert → `last_seen` avance, rien n'est
    ///   inséré ;
    /// * valeur différente → l'intervalle ouvert est fermé, un nouveau s'ouvre ;
    /// * aucun intervalle ouvert → un s'ouvre, `first_seen = last_seen` ;
    /// * **relevé qui n'est pas un constat** → rien n'est ni fermé ni avancé, et
    ///   la lecture refusée est consignée. Voir [`Self::enregistrer_sous_point`]
    ///   pour la raison, qui est la décision la plus importante de ce module.
    pub fn enregistrer_observation(
        &self,
        path: &str,
        observed: &ItemValue,
        at: DateTime<Utc>,
    ) -> Result<()> {
        // Un `SAVEPOINT` plutôt qu'un `BEGIN` : il ouvre une transaction quand
        // il n'y en a pas, et s'imbrique dans celle du scan entier le jour où
        // l'appelant en ouvrira une. `BEGIN` échouerait dans ce second cas, et
        // il échouerait à l'exécution, chez l'utilisateur.
        self.connexion
            .execute_batch("SAVEPOINT observation")
            .context("ouverture du point de reprise d'observation")?;
        match self.enregistrer_sous_point(path, observed, at) {
            Ok(()) => {
                self.connexion
                    .execute_batch("RELEASE observation")
                    .context("validation de l'observation")?;
                Ok(())
            }
            Err(erreur) => {
                // L'annulation ne doit pas masquer la cause première.
                let _ = self
                    .connexion
                    .execute_batch("ROLLBACK TO observation; RELEASE observation");
                Err(erreur)
            }
        }
    }

    /// L'enregistrement proprement dit, sous le point de reprise tenu par
    /// [`Self::enregistrer_observation`].
    fn enregistrer_sous_point(
        &self,
        path: &str,
        observed: &ItemValue,
        at: DateTime<Utc>,
    ) -> Result<()> {
        let horodatage = at.to_rfc3339();

        // Un relevé qui n'est pas un constat ne confirme rien et ne dément
        // rien : on ignore si l'item a changé, ce qui n'est pas la même chose
        // que savoir qu'il n'a pas changé.
        //
        // Écrire l'illisible comme une valeur de la série fabriquerait DEUX
        // changements — vers l'illisible, puis vers la valeur retrouvée — là où
        // il y en a eu au plus un, à un instant qu'on ignore. Avancer
        // `last_seen` serait plus discret et tout aussi faux : ce serait
        // affirmer qu'on a vu la valeur alors qu'on n'a rien vu du tout.
        //
        // Le prix de ce refus est assumé : quand l'item redevient lisible avec
        // une autre valeur, l'intervalle de changement s'étend du dernier relevé
        // lisible au premier relevé lisible suivant, et peut couvrir plusieurs
        // jours. Aucun événement daté ne s'y attribuera, donc le changement
        // sortira sans auteur identifiable. C'est le résultat correct : un
        // changement survenu pendant qu'on était aveugle n'a pas d'auteur connu.
        if !observed.est_constat() {
            self.connexion
                .execute(
                    "INSERT INTO lecture_refusee (at, path, raison) VALUES (?1, ?2, ?3)",
                    rusqlite::params![horodatage, path, raison_de(observed)],
                )
                .context("consignation d'une lecture refusée")?;
            return Ok(());
        }

        let valeur =
            serde_json::to_string(observed).context("sérialisation de la valeur observée")?;

        let ouvert: Option<(i64, String)> = self
            .connexion
            .query_row(
                "SELECT id, value FROM observation WHERE path = ?1 AND closed = 0",
                [path],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .optional()
            .context("lecture de l'intervalle ouvert")?;

        match ouvert {
            // La même valeur : on avance la borne, on n'insère rien. C'est ce
            // seul geste qui rend la taille du magasin proportionnelle au
            // changement et non au temps.
            Some((id, precedente)) if precedente == valeur => {
                self.connexion
                    .execute(
                        "UPDATE observation SET last_seen = ?1 WHERE id = ?2",
                        rusqlite::params![horodatage, id],
                    )
                    .context("avancement de l'intervalle ouvert")?;
            }
            Some((id, _)) => {
                self.connexion
                    .execute("UPDATE observation SET closed = 1 WHERE id = ?1", [id])
                    .context("fermeture de l'intervalle précédent")?;
                self.ouvrir_intervalle(path, &valeur, &horodatage)?;
            }
            None => self.ouvrir_intervalle(path, &valeur, &horodatage)?,
        }
        Ok(())
    }

    /// Ouvre un intervalle sur la valeur donnée, vue pour la première fois.
    ///
    /// `first_seen` et `last_seen` partagent le même paramètre : un intervalle
    /// naissant a été vu une fois, donc ses deux bornes coïncident. Les laisser
    /// diverger à l'ouverture reviendrait à prétendre avoir confirmé une valeur
    /// qu'on vient de découvrir.
    fn ouvrir_intervalle(&self, path: &str, valeur: &str, horodatage: &str) -> Result<()> {
        self.connexion
            .execute(
                "INSERT INTO observation (path, value, first_seen, last_seen, closed)
                 VALUES (?1, ?2, ?3, ?3, 0)",
                rusqlite::params![path, valeur, horodatage],
            )
            .context("ouverture d'un intervalle d'observation")?;
        Ok(())
    }

    /// Les intervalles d'un chemin, du plus ancien au plus récent.
    pub fn serie(&self, path: &str) -> Result<Vec<Intervalle>> {
        let mut requete = self.connexion.prepare(
            "SELECT path, value, first_seen, last_seen, closed
             FROM observation WHERE path = ?1 ORDER BY first_seen, id",
        )?;
        // `id` départage deux intervalles nés du même horodatage : sans lui,
        // l'ordre serait celui que SQLite voudra bien rendre, donc instable.
        let lignes = requete.query_map([path], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
                l.get::<_, String>(3)?,
                l.get::<_, i64>(4)?,
            ))
        })?;

        let mut serie = Vec::new();
        for ligne in lignes {
            let (chemin, valeur, debut, fin, ferme) = ligne.context("lecture d'un intervalle")?;
            serie.push(Intervalle {
                path: chemin,
                value: serde_json::from_str(&valeur).context("valeur observée illisible")?,
                first_seen: instant(&debut)?,
                last_seen: instant(&fin)?,
                closed: ferme != 0,
            });
        }
        Ok(serie)
    }

    /// Les changements qu'une série d'intervalles atteste.
    ///
    /// **Fonction pure, et c'est délibéré** : elle porte la règle de l'ADR-0011
    /// — un changement est un *intervalle*, jamais un instant — et doit pouvoir
    /// s'éprouver sans base de données, sur des séries fabriquées à la main.
    ///
    /// Deux intervalles consécutifs donnent un changement, et un seul : la
    /// valeur de l'ancien, celle du nouveau, et la fenêtre qui les sépare. Cette
    /// fenêtre court du **dernier scan qui a confirmé l'ancienne valeur** au
    /// **premier scan qui a vu la nouvelle** : entre les deux, le magasin ne
    /// sait rien.
    ///
    /// Elle peut couvrir plusieurs jours si l'item est resté illisible entre
    /// les deux — un relevé qui n'est pas un constat ne ferme rien et n'avance
    /// rien. Le changement sortira alors sans auteur identifiable, et c'est le
    /// résultat correct : un changement survenu pendant qu'on était aveugle n'a
    /// pas d'auteur connu.
    ///
    /// Un intervalle unique ne produit **aucun** changement : une valeur qu'on
    /// a toujours vue n'a jamais bougé.
    pub fn changements_de(serie: &[Intervalle]) -> Vec<Change> {
        serie
            .windows(2)
            .filter_map(|paire| {
                let [avant, apres] = paire else { return None };
                Some(Change::unattributed(
                    &apres.path,
                    (avant.value.clone(), avant.last_seen),
                    (apres.value.clone(), apres.first_seen),
                ))
            })
            .collect()
    }

    /// Les changements attestés pour un chemin, du plus ancien au plus récent.
    ///
    /// **Sans auteur** : l'attribution est un second temps, qui a besoin de
    /// sources que le magasin ne connaît pas (`ks_collectors::attribution`).
    /// Les séparer garde le magasin ignorant de la plateforme, donc testable
    /// partout.
    pub fn changements(&self, path: &str) -> Result<Vec<Change>> {
        Ok(Self::changements_de(&self.serie(path)?))
    }

    /// La date du dernier scan qui a **confirmé** une valeur pour ce chemin.
    ///
    /// Elle répond à une question que rien d'autre ne sait poser : « ce
    /// collecteur a-t-il échoué toute la semaine sans que rien ne le signale ? ».
    /// Un item dont la confirmation date de sept jours peut être parfaitement
    /// stable ; il peut aussi être illisible depuis sept jours. Les deux se
    /// ressemblent à l'écran, et seule cette date les sépare — un écran qui
    /// afficherait « conforme » sur le second dirait une chose fausse.
    ///
    /// `None` signifie qu'aucun constat n'a jamais été enregistré pour ce
    /// chemin, ce qui n'est pas la même chose qu'une confirmation ancienne.
    pub fn derniere_confirmation(&self, path: &str) -> Result<Option<DateTime<Utc>>> {
        // `max()` sur zéro ligne rend une ligne dont la colonne est NULL : c'est
        // le `Option` interne qui porte l'absence, pas l'absence de ligne.
        let brut: Option<String> = self
            .connexion
            .query_row(
                "SELECT max(last_seen) FROM observation WHERE path = ?1",
                [path],
                |l| l.get::<_, Option<String>>(0),
            )
            .context("lecture de la dernière confirmation")?;
        brut.map(|b| instant(&b)).transpose()
    }
}

/// Le motif à consigner pour un relevé qui n'est pas un constat.
fn raison_de(valeur: &ItemValue) -> String {
    match valeur {
        ItemValue::Illisible { raison } => raison.clone(),
        // Aucune autre variante n'échoue à `est_constat` aujourd'hui. Si une
        // s'ajoutait, sa forme lisible vaut mieux qu'un motif vide, qui
        // laisserait la cécité sans explication.
        autre => autre.to_string(),
    }
}

/// Relit un horodatage RFC 3339 tel que le magasin l'écrit.
fn instant(brut: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(brut)
        .with_context(|| format!("horodatage illisible : « {brut} »"))?
        .with_timezone(&Utc))
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
    fn la_reecriture_de_la_derniere_entree_est_detectee() {
        // Le défaut, mesuré avant correction : `verify_chain` relie chaque
        // entrée à la précédente, donc la dernière n'est reliée à rien. On
        // réécrivait sa charge utile et Keystone répondait « intact ». Le
        // journal vit dans %LOCALAPPDATA%, inscriptible par l'utilisateur
        // lui-même : c'est exactement l'adversaire A1 du modèle de menace.
        let (m, chemin) = magasin_neuf("reecriture-fin");
        for _ in 0..3 {
            m.ajouter(entree("converge")).expect("ajout");
        }
        let derniere = m.lire(None).expect("lecture").pop().expect("trois entrées");

        // On réécrit la charge utile de la dernière entrée SANS toucher à son
        // empreinte stockée — c'est l'altération la moins coûteuse à produire.
        let mut falsifiee = derniere.clone();
        falsifiee.verb = "scan".to_owned();
        m.connexion
            .execute(
                "UPDATE journal SET payload = ?1 WHERE seq = ?2",
                rusqlite::params![
                    serde_json::to_string(&falsifiee).expect("sérialisation"),
                    i64::try_from(derniere.seq).expect("seq tient dans un i64"),
                ],
            )
            .expect("réécriture");

        assert!(
            JournalEntry::verify_chain(&m.lire(None).expect("lecture")),
            "le chaînage seul ne voit RIEN : c'est le constat qui justifie \
             le contrôle d'empreinte, pas un défaut du test"
        );
        assert_eq!(
            m.premiere_empreinte_incoherente().expect("vérification"),
            Some(derniere.seq),
            "l'empreinte stockée doit démentir la charge utile réécrite"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn un_journal_intact_ne_signale_aucune_empreinte_incoherente() {
        let (m, chemin) = magasin_neuf("empreintes-saines");
        for _ in 0..4 {
            m.ajouter(entree("scan")).expect("ajout");
        }
        assert_eq!(
            m.premiere_empreinte_incoherente().expect("vérification"),
            None
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn la_troncature_par_la_fin_reste_indetectable_et_cest_documente() {
        // Ce test ne verrouille pas une garantie : il verrouille une **limite**.
        // Supprimer les dernières entrées produit une séquence plus courte et
        // parfaitement cohérente ; aucun contrôle local ne peut la distinguer
        // d'un journal qui se serait arrêté là. Si un jour ce test échoue,
        // c'est qu'une garantie a été gagnée — et le message affiché par
        // `ks journal`, ainsi que l'ADR-0004, doivent être corrigés en
        // conséquence, dans le même commit.
        let (m, chemin) = magasin_neuf("troncature-fin");
        for _ in 0..5 {
            m.ajouter(entree("scan")).expect("ajout");
        }
        m.connexion
            .execute("DELETE FROM journal WHERE seq >= ?1", [3])
            .expect("troncature");

        let restantes = m.lire(None).expect("lecture");
        assert_eq!(restantes.len(), 2);
        assert!(
            JournalEntry::verify_chain(&restantes),
            "limite assumée : une séquence tronquée par la fin reste cohérente"
        );
        assert_eq!(
            m.premiere_empreinte_incoherente().expect("vérification"),
            None,
            "les empreintes restantes sont authentiques — c'est bien le cas"
        );
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

    // ————————————————————————————————————————————————————————————————
    // La série d'observations
    // ————————————————————————————————————————————————————————————————

    /// Un horodatage de scan, reproductible.
    ///
    /// Pas de `Utc::now()` dans un chemin asserté : une borne d'intervalle qui
    /// dépend de l'instant du test ne se compare à rien.
    fn scan_a(minute: u32) -> chrono::DateTime<Utc> {
        use chrono::TimeZone;
        Utc.with_ymd_and_hms(2026, 8, 3, 14, minute, 0)
            .single()
            .expect("date fixe et valide, sans ambiguïté de fuseau en UTC")
    }

    /// Le nombre de lectures refusées consignées pour un chemin.
    fn lectures_refusees(m: &Magasin, path: &str) -> i64 {
        m.connexion
            .query_row(
                "SELECT count(*) FROM lecture_refusee WHERE path = ?1",
                [path],
                |l| l.get(0),
            )
            .expect("décompte des lectures refusées")
    }

    #[test]
    fn dix_confirmations_de_la_meme_valeur_ne_font_quune_seule_ligne() {
        // C'est la propriété qui rend la taille du magasin proportionnelle au
        // changement et non au temps. Si elle cède, un scan par heure produit
        // 19 320 lignes la première semaine pour une machine qui n'a pas bougé.
        let (m, chemin) = magasin_neuf("observation-stable");
        let voie = "security.defender.realtime";
        for minute in 0..10 {
            m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(minute))
                .expect("enregistrement");
        }

        let serie = m.serie(voie).expect("série");
        assert_eq!(
            serie.len(),
            1,
            "dix confirmations de la même valeur sont un seul intervalle, pas dix"
        );
        assert_eq!(serie[0].value, ItemValue::Bool(true));
        assert_eq!(serie[0].first_seen, scan_a(0));
        assert_eq!(
            serie[0].last_seen,
            scan_a(9),
            "la borne haute doit AVANCER : sans elle, on ne sait plus depuis \
             quand la valeur est confirmée"
        );
        assert!(!serie[0].closed, "l'intervalle courant reste ouvert");
        assert_eq!(
            m.derniere_confirmation(voie).expect("confirmation"),
            Some(scan_a(9))
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_valeur_qui_change_ferme_lintervalle_et_en_ouvre_un_autre() {
        let (m, chemin) = magasin_neuf("observation-changement");
        let voie = "security.defender.realtime";
        m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(0))
            .expect("premier relevé");
        m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(1))
            .expect("confirmation");
        m.enregistrer_observation(voie, &ItemValue::Bool(false), scan_a(2))
            .expect("changement");

        let serie = m.serie(voie).expect("série");
        assert_eq!(serie.len(), 2, "un changement, donc deux intervalles");

        assert_eq!(serie[0].value, ItemValue::Bool(true));
        assert!(serie[0].closed, "l'intervalle précédent doit être refermé");
        assert_eq!(
            serie[0].last_seen,
            scan_a(1),
            "la dernière confirmation de l'ancienne valeur est le scan 1, \
             pas celui qui l'a démentie"
        );

        assert_eq!(serie[1].value, ItemValue::Bool(false));
        assert!(!serie[1].closed);
        assert_eq!(
            serie[1].first_seen,
            scan_a(2),
            "first_seen du nouvel intervalle est le scan qui l'a VU"
        );
        assert_eq!(serie[1].last_seen, scan_a(2));

        // Et voilà la donnée que l'attribution réclame : le changement a eu lieu
        // entre ces deux bornes, sans qu'on puisse dire où.
        assert!(serie[0].last_seen < serie[1].first_seen);
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn deux_intervalles_consecutifs_donnent_un_changement_date_et_sans_auteur() {
        // La matière de l'attribution, fabriquée depuis le magasin (ADR-0011).
        // Les deux bornes ne sont pas interchangeables : `after_scan_at` est le
        // dernier scan qui a vu l'ANCIENNE valeur, `before_scan_at` le premier
        // qui a vu la nouvelle.
        let (m, chemin) = magasin_neuf("changement-date");
        let voie = "inventory.os.kernel";
        for (minute, valeur) in [(0, "26100"), (1, "26100"), (2, "26200")] {
            m.enregistrer_observation(voie, &ItemValue::Text(valeur.into()), scan_a(minute))
                .expect("relevé");
        }

        let changements = m.changements(voie).expect("changements");
        assert_eq!(changements.len(), 1, "un changement, pas deux");
        let c = &changements[0];
        assert_eq!(c.path, voie);
        assert_eq!(c.before, ItemValue::Text("26100".into()));
        assert_eq!(c.after, ItemValue::Text("26200".into()));
        assert_eq!(
            c.after_scan_at,
            scan_a(1),
            "le changement est survenu APRÈS la dernière confirmation de l'ancienne valeur"
        );
        assert_eq!(
            c.before_scan_at,
            scan_a(2),
            "…et AVANT le scan qui a vu la nouvelle"
        );

        // Un changement naît sans auteur, donc en signal. L'attribution est un
        // second temps, et son échec doit laisser ce signal en place.
        assert_eq!(c.provenance, ks_core::Provenance::Unknown);
        assert!(c.is_security_signal());
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_valeur_qui_na_jamais_bouge_ne_produit_aucun_changement() {
        let (m, chemin) = magasin_neuf("changement-stable");
        let voie = "security.defender.realtime";
        for minute in 0..5 {
            m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(minute))
                .expect("relevé");
        }
        assert!(
            m.changements(voie).expect("changements").is_empty(),
            "une valeur toujours vue n'a jamais changé"
        );
        // Et un chemin jamais observé n'invente pas d'histoire non plus.
        assert!(m.changements("chemin.inexistant").expect("vide").is_empty());
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_periode_illisible_elargit_lintervalle_du_changement() {
        // La contrepartie assumée du refus d'écrire l'illisible dans la série :
        // quand l'item redevient lisible avec une autre valeur, la fenêtre du
        // changement s'étend du dernier relevé lisible au premier relevé lisible
        // suivant. Aucune source datée à la journée ne s'y attribuera, et le
        // changement sortira sans auteur — ce qui est le résultat correct, un
        // changement survenu pendant qu'on était aveugle n'ayant pas d'auteur
        // connu.
        let (m, chemin) = magasin_neuf("changement-aveugle");
        let voie = "security.firmware.microcode_revision";
        m.enregistrer_observation(voie, &ItemValue::Text("23410000".into()), scan_a(0))
            .expect("relevé");
        m.enregistrer_observation(voie, &ItemValue::illisible("accès refusé"), scan_a(1))
            .expect("aveu");
        m.enregistrer_observation(voie, &ItemValue::illisible("accès refusé"), scan_a(2))
            .expect("aveu");
        m.enregistrer_observation(voie, &ItemValue::Text("23500000".into()), scan_a(3))
            .expect("relevé");

        let changements = m.changements(voie).expect("changements");
        assert_eq!(
            changements.len(),
            1,
            "un aveu ne fabrique pas deux changements"
        );
        assert_eq!(
            (changements[0].after_scan_at, changements[0].before_scan_at),
            (scan_a(0), scan_a(3)),
            "la fenêtre couvre toute la période d'aveuglement"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn une_serie_dun_seul_intervalle_natteste_aucun_changement() {
        // La fonction pure, éprouvée hors base : c'est elle qui porte la règle,
        // et une série d'un seul élément est le cas où `windows(2)` ne rend
        // rien — donc celui qu'on vérifie plutôt que de le supposer.
        let un = Intervalle {
            path: "inventory.os.kernel".into(),
            value: ItemValue::Text("26100".into()),
            first_seen: scan_a(0),
            last_seen: scan_a(9),
            closed: false,
        };
        assert!(Magasin::changements_de(&[]).is_empty());
        assert!(Magasin::changements_de(std::slice::from_ref(&un)).is_empty());

        // Trois intervalles : deux changements, dans l'ordre du temps.
        let deux = Intervalle {
            value: ItemValue::Text("26200".into()),
            first_seen: scan_a(10),
            last_seen: scan_a(19),
            ..un.clone()
        };
        let trois = Intervalle {
            value: ItemValue::Text("26300".into()),
            first_seen: scan_a(20),
            last_seen: scan_a(20),
            ..un.clone()
        };
        let changements = Magasin::changements_de(&[un, deux, trois]);
        assert_eq!(changements.len(), 2);
        assert!(changements[0].before_scan_at <= changements[1].after_scan_at);
        for c in &changements {
            assert!(
                c.after_scan_at < c.before_scan_at,
                "toute fenêtre court dans le sens du temps"
            );
        }
    }

    #[test]
    fn un_releve_illisible_ne_ferme_rien_et_navance_rien() {
        // Le point le plus important du module. Un aveu de lecture n'est pas une
        // valeur : le traiter comme telle fabriquerait deux changements — vers
        // l'illisible, puis vers la valeur retrouvée — là où il y en a eu au
        // plus un. Les trois exclusions Defender sont illisibles à CHAQUE scan
        // faute d'élévation ; le défaut serait donc quotidien, pas théorique.
        let (m, chemin) = magasin_neuf("observation-illisible");
        let voie = "security.defender.exclusions";
        m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(0))
            .expect("premier relevé");
        m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(1))
            .expect("confirmation");
        m.enregistrer_observation(voie, &ItemValue::illisible("accès refusé"), scan_a(2))
            .expect("relevé illisible");

        let serie = m.serie(voie).expect("série");
        assert_eq!(serie.len(), 1, "zéro fermeture, zéro ouverture");
        assert!(!serie[0].closed, "rien n'a été fermé");
        assert_eq!(serie[0].value, ItemValue::Bool(true));
        assert_eq!(
            serie[0].last_seen,
            scan_a(1),
            "last_seen doit rester où il était : on n'a rien confirmé au scan 2"
        );
        assert_eq!(
            m.derniere_confirmation(voie).expect("confirmation"),
            Some(scan_a(1)),
            "la fraîcheur de la confirmation ne doit pas mentir sur une cécité"
        );

        // La cécité est consignée, avec son motif : c'est ce qui rend un
        // changement survenu pendant l'aveuglement *expliqué* plutôt que
        // silencieusement faux.
        assert_eq!(lectures_refusees(&m, voie), 1);
        let raison: String = m
            .connexion
            .query_row(
                "SELECT raison FROM lecture_refusee WHERE path = ?1",
                [voie],
                |l| l.get(0),
            )
            .expect("motif consigné");
        assert_eq!(raison, "accès refusé");
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn deux_intervalles_ouverts_pour_un_meme_chemin_sont_refuses_par_lindex() {
        // La barrière, éprouvée en tentant de la franchir. Relire l'index dans
        // le schéma ne prouve rien : un index partiel mal écrit se lit comme un
        // index correct. On insère donc réellement la ligne interdite.
        let (m, chemin) = magasin_neuf("observation-index");
        let voie = "security.firewall.profil";
        m.enregistrer_observation(voie, &ItemValue::Bool(true), scan_a(0))
            .expect("premier intervalle");

        let interdite = m.connexion.execute(
            "INSERT INTO observation (path, value, first_seen, last_seen, closed)
             VALUES (?1, ?2, ?3, ?3, 0)",
            rusqlite::params![voie, "false", scan_a(1).to_rfc3339()],
        );
        let erreur = interdite.expect_err(
            "un second intervalle OUVERT sur le même chemin doit être refusé \
             par la base, pas par la vigilance de l'appelant",
        );
        assert!(
            erreur.to_string().to_uppercase().contains("UNIQUE"),
            "c'est bien la contrainte d'unicité qui doit refuser, pas un autre \
             échec qui y ressemblerait : {erreur}"
        );

        // Et la moitié qui prouve que l'index est bien PARTIEL : un intervalle
        // fermé sur le même chemin, lui, doit passer. Un index unique complet
        // interdirait tout historique, ce qui casserait la série en silence.
        m.connexion
            .execute(
                "INSERT INTO observation (path, value, first_seen, last_seen, closed)
                 VALUES (?1, ?2, ?3, ?3, 1)",
                rusqlite::params![voie, "false", scan_a(1).to_rfc3339()],
            )
            .expect("un intervalle fermé ne heurte pas un index partiel");
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn deux_chemins_ne_partagent_pas_leur_serie() {
        let (m, chemin) = magasin_neuf("observation-cloison");
        m.enregistrer_observation("a.b", &ItemValue::Int(1), scan_a(0))
            .expect("a");
        m.enregistrer_observation("c.d", &ItemValue::Int(2), scan_a(0))
            .expect("c");
        m.enregistrer_observation("a.b", &ItemValue::Int(3), scan_a(1))
            .expect("a change");

        assert_eq!(m.serie("a.b").expect("série a").len(), 2);
        assert_eq!(
            m.serie("c.d").expect("série c").len(),
            1,
            "le changement d'un chemin ne touche pas la série d'un autre"
        );
        assert_eq!(
            m.derniere_confirmation("jamais.vu").expect("confirmation"),
            None,
            "aucun constat n'est autre chose qu'une confirmation ancienne"
        );
        let _ = std::fs::remove_file(chemin);
    }

    #[test]
    fn les_observations_ne_perturbent_pas_le_chainage_du_journal() {
        // Les deux tables vivent dans le même fichier ; c'est justement pour ça
        // qu'il faut vérifier que la seconde n'abîme pas les garanties de la
        // première. La série n'est PAS chaînée, et rien ici ne prétend le
        // contraire — le journal, lui, doit rester intact.
        let (m, chemin) = magasin_neuf("observation-et-journal");
        for minute in 0..4 {
            m.ajouter(entree("scan")).expect("ajout");
            m.enregistrer_observation(
                "space.volume[c].used_percent",
                &ItemValue::Int(61),
                scan_a(minute),
            )
            .expect("observation");
            m.enregistrer_observation(
                "security.defender.exclusions",
                &ItemValue::illisible("accès refusé"),
                scan_a(minute),
            )
            .expect("lecture refusée");
        }

        let lues = m.lire(None).expect("lecture");
        assert_eq!(lues.len(), 4);
        assert!(
            JournalEntry::verify_chain(&lues),
            "écrire des observations ne doit rien changer au chaînage"
        );
        assert_eq!(
            m.premiere_empreinte_incoherente().expect("vérification"),
            None
        );
        assert_eq!(
            m.serie("space.volume[c].used_percent")
                .expect("série")
                .len(),
            1
        );
        assert_eq!(lectures_refusees(&m, "security.defender.exclusions"), 4);
        let _ = std::fs::remove_file(chemin);
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

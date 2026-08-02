//! Attribution winget — lecture de ses bases de suivi (D1-01, Phase 0.3).
//!
//! ## Pourquoi ce module existe
//!
//! winget se détecte sans se laisser interroger. Sa présence est triviale à
//! constater — un exécutable dans `WindowsApps` — mais savoir **quelles**
//! applications il gère demande de lire son inventaire, et cet inventaire vit
//! dans des bases SQLite dont le format n'est contractuel nulle part.
//!
//! Tant que ce module n'existait pas, l'inventaire logiciel affichait « 63
//! applications, 63 non attribuées » : un décompte exact et une conclusion
//! inutilisable, puisque « non attribuée » y voulait dire « je n'ai pas su
//! regarder » et non « personne ne la met à jour ». Le critère de sortie de la
//! Phase 0 exige la seconde.
//!
//! ## Ce que winget range, et où
//!
//! Une base par **source**, sous `LocalState`. Chacune ne contient que ce que
//! winget a lui-même installé depuis cette source — trois paquets pour le dépôt
//! communautaire, sept pour le Store sur la machine de référence. Ce ne sont donc
//! pas les catalogues, qui pèsent des milliers d'entrées.
//!
//! Deux tables suffisent à l'attribution, et elles existent dans les deux
//! versions de schéma rencontrées (1.3 et 1.7) :
//!
//! * `productcodes` — le code produit, qui est **exactement** le nom de la clé de
//!   désinstallation du registre : `git_is1`, `{8cb0885f-…}` ;
//! * `pfns` — le nom de famille d'un paquet MSIX, pour les applications qui n'ont
//!   aucune entrée de désinstallation.
//!
//! Le rapprochement par code produit vaut mieux que par nom, et un cas de la
//! machine de référence le prouve : `readyfor` s'affiche « Smart Connect » dans le
//! registre et « Ready For Assistant » chez winget. Deux libellés sans rapport,
//! qu'aucune comparaison de chaînes n'aurait jamais reliés.
//!
//! ## Lecture seule, y compris les fichiers annexes
//!
//! Ouvrir une base SQLite peut créer un journal, un `-wal`, un `-shm`. La règle
//! du crate ne parle pas que de la donnée : **un collecteur n'écrit aucun octet,
//! nulle part.** D'où `SQLITE_OPEN_READ_ONLY` sans `SQLITE_OPEN_CREATE`, qui
//! ouvre le fichier sans droit d'écriture au niveau du système.
//!
//! `immutable=1` a été écarté sciemment : il ferait sauter la détection de
//! journal chaud, et SQLite prévient qu'il peut alors renvoyer des résultats faux
//! **en silence**. Si winget écrit pendant qu'on lit, on préfère l'échec propre
//! (`SQLITE_READONLY_ROLLBACK`) et un aveu d'illisibilité.

use std::collections::BTreeSet;

// La lecture SQLite est **entièrement** sous `cfg(windows)`, imports compris.
// `rusqlite` n'est déclarée que pour cette cible dans `Cargo.toml` : l'importer
// sans condition compilait sur le poste de développement et cassait les trois
// travaux Linux de l'intégration continue, y compris l'agent musl. La leçon est
// celle de tout ce module — la cible de développement n'est pas la seule cible.
#[cfg(windows)]
use std::path::{Path, PathBuf};

/// Ce que winget revendique, tel que ses bases de suivi le déclarent.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SuiviWinget {
    /// Codes produits, en minuscules — la clé de rapprochement avec le registre.
    pub codes_produits: BTreeSet<String>,
    /// Noms de famille des paquets MSIX, en minuscules.
    pub familles_msix: BTreeSet<String>,
    /// Bases présentes mais illisibles, avec la raison.
    ///
    /// Le champ n'est pas décoratif : tant qu'il n'est pas vide, l'attribution
    /// est incomplète et le décompte des non attribuées reste un **majorant**.
    /// Le taire ferait passer une lecture partielle pour une mesure.
    pub bases_illisibles: Vec<String>,
    /// Nombre de bases effectivement lues.
    pub bases_lues: usize,
}

impl SuiviWinget {
    /// winget a-t-il pu être interrogé complètement ?
    #[must_use]
    pub fn est_complet(&self) -> bool {
        self.bases_illisibles.is_empty() && self.bases_lues > 0
    }

    /// Le suivi revendique-t-il cette clé de désinstallation ?
    ///
    /// La comparaison est insensible à la casse : le registre écrit `git_is1` et
    /// les GUID indifféremment en majuscules ou en minuscules selon l'installeur.
    #[must_use]
    pub fn revendique_code(&self, code: &str) -> bool {
        self.codes_produits.contains(&code.to_lowercase())
    }

    /// Le suivi revendique-t-il cette famille de paquet MSIX ?
    #[must_use]
    pub fn revendique_famille(&self, famille: &str) -> bool {
        self.familles_msix.contains(&famille.to_lowercase())
    }
}

/// Version majeure de schéma que ce lecteur sait interpréter.
///
/// Au-delà, on refuse de lire plutôt que de deviner : un format qu'on croit
/// comprendre produirait une attribution fausse, et une attribution fausse est
/// pire qu'une attribution absente — elle rassure.
#[cfg(windows)]
const SCHEMA_MAJEUR_CONNU: &str = "1";

/// Lit les bases de suivi de winget.
///
/// Ne renvoie jamais d'erreur : une base absente est un cas normal (winget non
/// installé, ou installé sans avoir rien posé), et une base illisible se déclare
/// dans [`SuiviWinget::bases_illisibles`] plutôt que de faire échouer le scan.
#[must_use]
pub fn lire_suivi() -> SuiviWinget {
    #[cfg(windows)]
    {
        let Some(racine) = racine_local_state() else {
            return SuiviWinget::default();
        };
        lire_depuis(&racine)
    }
    #[cfg(not(windows))]
    {
        SuiviWinget::default()
    }
}

/// Le dossier d'état de l'App Installer, où vit une base par source.
#[cfg(windows)]
fn racine_local_state() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let chemin = PathBuf::from(base)
        .join("Packages")
        .join("Microsoft.DesktopAppInstaller_8wekyb3d8bbwe")
        .join("LocalState");
    chemin.is_dir().then_some(chemin)
}

/// Parcourt les sources et agrège ce que chacune revendique.
///
/// On énumère les sous-dossiers plutôt que de coder en dur les deux noms connus
/// (`Microsoft.Winget.Source_…` et `StoreEdgeFD`) : une source ajoutée par
/// l'utilisateur crée son propre dossier, et l'ignorer produirait exactement le
/// faux « orphelin » que ce module existe pour supprimer.
#[cfg(windows)]
fn lire_depuis(racine: &Path) -> SuiviWinget {
    let mut suivi = SuiviWinget::default();

    let Ok(entrees) = std::fs::read_dir(racine) else {
        suivi
            .bases_illisibles
            .push("dossier d'état de winget illisible".to_owned());
        return suivi;
    };

    for entree in entrees.flatten() {
        let base = entree.path().join("installed.db");
        if !base.is_file() {
            continue;
        }
        let nom = entree.file_name().to_string_lossy().to_string();
        match lire_une_base(&base) {
            Ok((codes, familles)) => {
                suivi.bases_lues += 1;
                suivi.codes_produits.extend(codes);
                suivi.familles_msix.extend(familles);
            }
            Err(raison) => suivi.bases_illisibles.push(format!("{nom} — {raison}")),
        }
    }

    suivi
}

/// Ouvre une base en lecture seule et en extrait les deux tables utiles.
///
/// L'erreur est une chaîne destinée à l'utilisateur, pas un type : à ce stade,
/// l'appelant n'a qu'une décision à prendre — déclarer l'attribution incomplète.
#[cfg(windows)]
fn lire_une_base(chemin: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    use rusqlite::{Connection, OpenFlags};

    // Sans `SQLITE_OPEN_CREATE`, et sans droit d'écriture : ni base créée, ni
    // journal, ni `-wal`. C'est ce qui rend l'appel compatible avec la règle
    // « aucun collecteur n'écrit », fichiers annexes compris.
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connexion = Connection::open_with_flags(chemin, flags)
        .map_err(|e| format!("ouverture refusée ({e})"))?;

    let majeur: String = connexion
        .query_row(
            "SELECT value FROM metadata WHERE name = 'majorVersion'",
            [],
            |ligne| ligne.get(0),
        )
        .map_err(|e| format!("version de schéma illisible ({e})"))?;

    if majeur != SCHEMA_MAJEUR_CONNU {
        return Err(format!(
            "schéma de version {majeur}, ce lecteur ne connaît que la {SCHEMA_MAJEUR_CONNU}"
        ));
    }

    let codes = colonne_texte(&connexion, "SELECT productcode FROM productcodes")?;
    let familles = colonne_texte(&connexion, "SELECT pfn FROM pfns")?;
    Ok((codes, familles))
}

/// Ramène une colonne de texte, normalisée en minuscules.
#[cfg(windows)]
fn colonne_texte(connexion: &rusqlite::Connection, requete: &str) -> Result<Vec<String>, String> {
    let mut preparee = connexion
        .prepare(requete)
        .map_err(|e| format!("requête refusée ({e})"))?;
    let lignes = preparee
        .query_map([], |ligne| ligne.get::<_, String>(0))
        .map_err(|e| format!("lecture refusée ({e})"))?;

    Ok(lignes
        .flatten()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_suivi_vide_nest_pas_un_suivi_complet() {
        // Distinction décisive : « winget ne gère rien » et « winget n'a pas pu
        // être lu » ne se ressemblent que de loin. Un suivi sans aucune base lue
        // ne doit jamais autoriser à conclure qu'une application est orpheline.
        let vide = SuiviWinget::default();
        assert!(!vide.est_complet());
        assert!(!vide.revendique_code("git_is1"));
    }

    #[test]
    fn une_base_illisible_empeche_de_conclure() {
        let partiel = SuiviWinget {
            bases_lues: 1,
            bases_illisibles: vec!["StoreEdgeFD — ouverture refusée".to_owned()],
            ..SuiviWinget::default()
        };
        assert!(
            !partiel.est_complet(),
            "une base lue sur deux ne fait pas une attribution complète"
        );
    }

    #[test]
    fn le_rapprochement_ignore_la_casse() {
        // Le registre écrit les GUID tantôt en majuscules, tantôt en minuscules,
        // selon l'installeur qui a posé la clé. Une comparaison stricte laisserait
        // passer une application pourtant gérée, donc un faux orphelin.
        let suivi = SuiviWinget {
            bases_lues: 1,
            codes_produits: ["{8cb0885f-8776-4fd1-898f-62b39fef6ae9}".to_owned()]
                .into_iter()
                .collect(),
            familles_msix: ["microsoft.powershell_8wekyb3d8bbwe".to_owned()]
                .into_iter()
                .collect(),
            ..SuiviWinget::default()
        };

        assert!(suivi.revendique_code("{8CB0885F-8776-4FD1-898F-62B39FEF6AE9}"));
        assert!(suivi.revendique_famille("Microsoft.PowerShell_8wekyb3d8bbwe"));
        assert!(!suivi.revendique_code("{00000000-0000-0000-0000-000000000000}"));
    }

    /// Fabrique une base au format de winget, pour éprouver le lecteur sans
    /// dépendre de la machine — et sans jamais écrire dans le dossier de winget.
    #[cfg(windows)]
    fn base_de_test(dossier: &Path, majeur: &str) -> PathBuf {
        use rusqlite::Connection;
        std::fs::create_dir_all(dossier).expect("dossier de test");
        let chemin = dossier.join("installed.db");
        let _ = std::fs::remove_file(&chemin);
        let c = Connection::open(&chemin).expect("base de test");
        c.execute_batch(&format!(
            "CREATE TABLE metadata (name TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
             INSERT INTO metadata VALUES ('majorVersion', '{majeur}');
             CREATE TABLE productcodes (rowid INTEGER PRIMARY KEY, productcode TEXT NOT NULL);
             INSERT INTO productcodes (productcode) VALUES ('Git_is1'), ('  '), ('rustup');
             CREATE TABLE pfns (rowid INTEGER PRIMARY KEY, pfn TEXT NOT NULL);
             INSERT INTO pfns (pfn) VALUES ('Microsoft.PowerShell_8wekyb3d8bbwe');"
        ))
        .expect("schéma de test");
        chemin
    }

    #[cfg(windows)]
    #[test]
    fn une_base_au_format_attendu_se_lit() {
        let racine = std::env::temp_dir().join("ks-winget-lecture");
        let _ = std::fs::remove_dir_all(&racine);
        base_de_test(&racine.join("Source.Test"), "1");

        let suivi = lire_depuis(&racine);
        assert!(suivi.est_complet(), "{:?}", suivi.bases_illisibles);
        assert!(suivi.revendique_code("git_is1"));
        assert!(suivi.revendique_famille("microsoft.powershell_8wekyb3d8bbwe"));
        assert_eq!(
            suivi.codes_produits.len(),
            2,
            "la valeur blanche ne doit pas compter comme un code produit"
        );
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[cfg(windows)]
    #[test]
    fn un_schema_inconnu_est_refuse_plutot_que_devine() {
        // Le format n'est contractuel nulle part : Microsoft peut le changer sans
        // préavis. Lire une version qu'on ne connaît pas produirait une
        // attribution fausse, c'est-à-dire rassurante à tort.
        let racine = std::env::temp_dir().join("ks-winget-schema");
        let _ = std::fs::remove_dir_all(&racine);
        base_de_test(&racine.join("Source.Future"), "2");

        let suivi = lire_depuis(&racine);
        assert!(!suivi.est_complet());
        assert!(
            suivi.codes_produits.is_empty(),
            "rien ne doit être attribué"
        );
        assert!(
            suivi.bases_illisibles[0].contains("version 2"),
            "la raison doit nommer la version rencontrée : {:?}",
            suivi.bases_illisibles
        );
        let _ = std::fs::remove_dir_all(&racine);
    }

    #[cfg(windows)]
    #[test]
    fn la_lecture_ne_cree_aucun_fichier_annexe() {
        // La règle du crate couvre les fichiers annexes, pas seulement la donnée.
        // Une ouverture SQLite ordinaire déposerait un journal ou un `-wal` à côté
        // de la base de winget — une écriture, dans le dossier d'un autre logiciel.
        let racine = std::env::temp_dir().join("ks-winget-annexes");
        let _ = std::fs::remove_dir_all(&racine);
        let dossier = racine.join("Source.Test");
        let base = base_de_test(&dossier, "1");

        let avant = std::fs::metadata(&base).expect("base présente").len();
        let _ = lire_depuis(&racine);

        let restants: Vec<_> = std::fs::read_dir(&dossier)
            .expect("dossier lisible")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "installed.db")
            .collect();

        assert!(
            restants.is_empty(),
            "la lecture a déposé des fichiers : {restants:?}"
        );
        assert_eq!(
            std::fs::metadata(&base).expect("base présente").len(),
            avant,
            "la base elle-même a changé de taille"
        );
        let _ = std::fs::remove_dir_all(&racine);
    }
}

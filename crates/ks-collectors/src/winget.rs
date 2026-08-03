//! Attribution winget — lecture de ses bases de suivi (D1-01, Phase 0.3).
//!
//! ## Pourquoi ce module existe
//!
//! winget se détecte sans se laisser interroger. Sa présence se constate par son
//! dossier d'état — et **surtout pas** par `winget.exe` dans `WindowsApps`, qui
//! n'est que son alias d'exécution d'application, donc un réglage que
//! l'utilisateur éteint sans rien désinstaller ; voir [`est_installe`]. Mais
//! savoir **quelles** applications il gère demande de lire son inventaire, et
//! cet inventaire vit dans des bases SQLite dont le format n'est contractuel
//! nulle part.
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

/// winget est-il installé sur ce poste ?
///
/// # Le témoin est la donnée, jamais l'alias d'exécution
///
/// La présence se constate par le **dossier d'état** de l'App Installer, et
/// surtout pas par `%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe`. Ce
/// dernier n'est pas l'exécutable de winget : c'est son *alias d'exécution
/// d'application*, un réglage que l'utilisateur éteint d'un interrupteur dans
/// Paramètres → Applications, et qui ne désinstalle rien.
///
/// Conditionner l'attribution à cet alias produisait le pire enchaînement
/// possible : alias éteint et bases pleines, [`lire_suivi`] n'était jamais
/// appelé, tous les paquets tombaient en « non attribué », et
/// `inventory.software.attribution` publiait pourtant « complète ». Tout
/// l'appareil d'honnêteté de [`SuiviWinget::est_complet`] était court-circuité
/// par un réglage d'interface.
#[must_use]
pub fn est_installe() -> bool {
    #[cfg(windows)]
    {
        racine_local_state().is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Le dossier d'état de l'App Installer, où vit une base par source.
#[cfg(windows)]
fn racine_local_state() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    racine_sous(&PathBuf::from(base))
}

/// Le dossier d'état sous une racine `LOCALAPPDATA` donnée.
///
/// Séparée de [`racine_local_state`] pour être éprouvable sans toucher au
/// `LOCALAPPDATA` du processus. C'est ce qui permet à
/// `winget_se_constate_par_ses_bases_jamais_par_son_alias` de vérifier que le
/// témoin de présence est bien la donnée : le test fabrique une racine qui
/// contient les bases et **pas** `winget.exe`, et exige un `Some`.
#[cfg(windows)]
fn racine_sous(base: &Path) -> Option<PathBuf> {
    let chemin = base
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
    use rusqlite::Connection;

    // Sans `SQLITE_OPEN_CREATE`, et sans droit d'écriture : ni base créée, ni
    // journal, ni `-wal`. C'est ce qui rend l'appel compatible avec la règle
    // « aucun collecteur n'écrit », fichiers annexes compris.
    // Constante nommée, et non une expression jetée dans l'appel : c'est elle
    // que la barrière `les_drapeaux_douverture_interdisent_toute_ecriture` va
    // interroger. Un drapeau vérifiable est un drapeau qu'on ne peut pas
    // élargir sans qu'un test le dise.
    let flags = OUVERTURE_LECTURE_SEULE;
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
///
/// Une ligne que SQLite ne rend pas en texte — un `BLOB`, un `NULL` — fait
/// **échouer la lecture entière**, et ne disparaît plus en silence.
///
/// Le code employait `flatten()`, qui jette les `Err` sans rien dire. Une ligne
/// perdue ainsi retirait un code produit du suivi ; l'application correspondante
/// tombait alors en « non attribuée », et `SuiviWinget::est_complet()` répondait
/// pourtant vrai. Le majorant que tout ce module existe pour tenir devenait un
/// minorant sans le dire — exactement le défaut que l'attribution winget vient
/// de corriger ailleurs.
///
/// La probabilité réelle est faible : l'affinité `TEXT` de la colonne convertit
/// les entiers, il faut un blob ou un `NULL` pour y arriver. Le principe, lui,
/// ne dépend pas de la probabilité. Une base qu'on ne sait pas lire entièrement
/// est une base illisible, et c'est ce que l'appelant doit entendre.
#[cfg(windows)]
fn colonne_texte(connexion: &rusqlite::Connection, requete: &str) -> Result<Vec<String>, String> {
    let mut preparee = connexion
        .prepare(requete)
        .map_err(|e| format!("requête refusée ({e})"))?;
    let lignes = preparee
        .query_map([], |ligne| ligne.get::<_, String>(0))
        .map_err(|e| format!("lecture refusée ({e})"))?;

    let mut valeurs = Vec::new();
    for ligne in lignes {
        let brut = ligne.map_err(|e| format!("ligne non textuelle dans la base ({e})"))?;
        let normalise = brut.trim().to_lowercase();
        if !normalise.is_empty() {
            valeurs.push(normalise);
        }
    }
    Ok(valeurs)
}

/// Les drapeaux d'ouverture de la base de winget, et rien d'autre.
///
/// Constante nommée au niveau du module, et non une expression jetée dans
/// l'appel : c'est elle qu'interroge la barrière
/// `les_drapeaux_douverture_interdisent_toute_ecriture`. Un drapeau qu'on peut
/// nommer est un drapeau qu'on ne peut pas élargir en silence.
///
/// `SQLITE_OPEN_READ_ONLY` **sans** `CREATE` : c'est ce qui garantit qu'aucun
/// journal ni fichier `-wal` n'apparaît à côté de la base d'un autre logiciel.
///
/// `cfg(windows)` comme tout ce qui touche à `rusqlite` : la dépendance vit sous
/// `[target.'cfg(windows)'.dependencies]`, donc son type n'existe pas ailleurs.
/// Sans ce garde, le crate entier cessait de compiler pour la cible Linux.
#[cfg(windows)]
const OUVERTURE_LECTURE_SEULE: rusqlite::OpenFlags =
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY.union(rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX);

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
    fn winget_se_constate_par_ses_bases_jamais_par_son_alias_dexecution() {
        // Le défaut mesuré : la présence de winget était constatée sur
        // `%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe`, qui n'est pas son
        // exécutable mais son **alias d'exécution d'application** — un réglage
        // que l'utilisateur éteint sans rien désinstaller.
        //
        // La racine fabriquée ici porte les bases et ne porte PAS l'alias. Le
        // jour où le témoin redeviendrait l'exécutable, ce test échouerait sur
        // n'importe quelle machine, alias allumé ou non : c'est la raison d'être
        // du paramètre de `racine_sous`.
        let faux = std::env::temp_dir().join("ks-winget-sans-alias");
        let _ = std::fs::remove_dir_all(&faux);
        let etat = faux
            .join("Packages")
            .join("Microsoft.DesktopAppInstaller_8wekyb3d8bbwe")
            .join("LocalState");
        base_de_test(&etat.join("Source.Test"), "1");

        let alias = faux
            .join("Microsoft")
            .join("WindowsApps")
            .join("winget.exe");
        assert!(!alias.exists(), "le décor doit être sans alias");

        let racine = racine_sous(&faux).expect("les bases attestent de winget, pas l'alias");
        let suivi = lire_depuis(&racine);
        assert!(suivi.est_complet(), "{:?}", suivi.bases_illisibles);
        assert!(suivi.revendique_code("git_is1"));

        let _ = std::fs::remove_dir_all(&faux);
    }

    #[cfg(windows)]
    /// La permission, et non son résidu.
    ///
    /// Deux barrières superposées, parce qu'elles ne couvrent pas la même
    /// chose. La première interroge la constante réellement passée à SQLite :
    /// elle prouve que **cette** ouverture est en lecture seule. La seconde lit
    /// le fichier source et refuse que le moindre drapeau d'écriture y
    /// apparaisse **où que ce soit** — donc aussi dans une ouverture qu'on
    /// ajouterait demain sans penser à ce test.
    ///
    /// C'est la différence entre vérifier un appel et fermer une classe
    /// d'appels. La règle du crate — « aucun collecteur n'écrit, jamais, nulle
    /// part » — porte sur la classe.
    #[test]
    fn les_drapeaux_douverture_interdisent_toute_ecriture() {
        use rusqlite::OpenFlags;

        // Ce que la constante autorise, et rien d'autre.
        assert!(
            OUVERTURE_LECTURE_SEULE.contains(OpenFlags::SQLITE_OPEN_READ_ONLY),
            "la base de winget doit s'ouvrir en lecture seule"
        );
        for interdit in [
            OpenFlags::SQLITE_OPEN_READ_WRITE,
            OpenFlags::SQLITE_OPEN_CREATE,
        ] {
            assert!(
                !OUVERTURE_LECTURE_SEULE.intersects(interdit),
                "un drapeau d'écriture est passé à SQLite : {interdit:?}"
            );
        }

        // Et qu'aucun autre appel du module n'en réintroduise un.
        const SOURCE: &str = include_str!("winget.rs");

        // Le module de tests est retiré du champ : il NOMME les drapeaux
        // interdits pour les refuser, et un contrôle qui échouerait sur sa
        // propre formulation serait ingérable — c'est le même piège que le
        // glossaire, qui doit pouvoir citer le vocabulaire qu'il proscrit.
        // La coupure se fait sur `#[cfg(test)]`, et le test le vérifie plus
        // bas : sans elle, le contrôle ne porterait plus sur rien.
        let production = SOURCE
            .split_once("#[cfg(test)]")
            .map_or(SOURCE, |(avant, _)| avant);
        assert!(
            production.len() < SOURCE.len(),
            "la coupure sur #[cfg(test)] n'a rien retiré — le contrôle porterait              sur le fichier entier, tests compris, et ne pourrait que échouer"
        );
        let code = production
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        for interdit in ["SQLITE_OPEN_READ_WRITE", "SQLITE_OPEN_CREATE"] {
            assert!(
                !code.contains(interdit),
                "« {interdit} » apparaît dans winget.rs :                  un collecteur n'ouvre jamais la base d'un autre logiciel en écriture"
            );
        }
    }

    /// Une ligne non textuelle rend la base illisible, elle ne s'efface pas.
    ///
    /// `flatten()` jetait les `Err` sans un mot. Une ligne perdue ainsi retirait
    /// un code produit du suivi ; l'application correspondante tombait en « non
    /// attribuée », et `est_complet()` répondait pourtant vrai. Le majorant que
    /// ce module existe pour tenir devenait un minorant, en silence.
    /// Windows seulement : `rusqlite`, `base_de_test` et `lire_depuis` n'y
    /// existent pas ailleurs.
    #[cfg(windows)]
    #[test]
    fn une_ligne_non_textuelle_rend_la_base_illisible() {
        use rusqlite::Connection;

        let racine = std::env::temp_dir().join("ks-winget-blob");
        let _ = std::fs::remove_dir_all(&racine);
        let dossier = racine.join("Source.Test");
        let base = base_de_test(&dossier, SCHEMA_MAJEUR_CONNU);

        // Un blob dans une colonne d'affinité TEXT : SQLite le conserve tel
        // quel, et `get::<_, String>` le refuse.
        let c = Connection::open(&base).expect("base de test");
        c.execute(
            "INSERT INTO productcodes (productcode) VALUES (?1)",
            [vec![0xFF_u8, 0x00]],
        )
        .expect("insertion du blob");
        drop(c);

        let issue = lire_depuis(&racine);
        assert!(
            issue
                .bases_illisibles
                .iter()
                .any(|raison| raison.contains("non textuelle")),
            "la base doit se déclarer illisible, pas rendre une liste amputée : {:?}",
            issue.bases_illisibles
        );
        assert!(
            !issue.est_complet(),
            "un suivi bâti sur une base illisible ne peut pas se dire complet"
        );
        let _ = std::fs::remove_dir_all(&racine);
    }

    /// Windows seulement : `base_de_test` et `lire_depuis` n'y existent pas ailleurs.
    #[cfg(windows)]
    #[test]
    fn la_lecture_ne_cree_aucun_fichier_annexe() {
        // La règle du crate couvre les fichiers annexes, pas seulement la donnée.
        // Une ouverture SQLite ordinaire déposerait un journal ou un `-wal` à côté
        // de la base de winget — une écriture, dans le dossier d'un autre logiciel.
        //
        // **Ce test ne prouve pas le droit d'écriture, et il faut le savoir.**
        // Il inspecte le dossier APRÈS fermeture de la connexion, or SQLite
        // nettoie ses fichiers annexes en se fermant proprement. Éprouvé : il
        // passe avec `SQLITE_OPEN_READ_WRITE | CREATE`, et il passe encore avec
        // la base en mode WAL. Il mesure un résidu, pas une permission.
        //
        // Il reste ici parce qu'il attrape une régression grossière — un fichier
        // laissé derrière soi — mais ce qui garde réellement la règle, c'est
        // `les_drapeaux_douverture_interdisent_toute_ecriture` ci-dessous, qui
        // interroge la permission elle-même.
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

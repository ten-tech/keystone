//! # `ks` — la CLI de Keystone
//!
//! **La CLI est la surface de référence du produit** (principe P4). L'interface
//! graphique n'est qu'un client de la même API : tout ce que l'UI sait faire est
//! atteignable ici, et la palette de commandes de l'UI affiche d'ailleurs la
//! commande `ks` équivalente à côté de chaque résultat.
//!
//! ## Deux règles visibles dans la définition des arguments
//!
//! * **`--dry-run` est le défaut.** Il n'existe pas de drapeau `--dry-run` sur les
//!   commandes mutantes : c'est `--apply` qui existe. On ne peut donc pas écrire
//!   par omission (principe P2).
//! * **`--json` sur tout.** La sortie machine est stable et versionnée ; c'est ce
//!   qui rend l'outil scriptable et testable.
//!
//! ## État
//!
//! Phase 0 : `scan`, `status`, `explain`, `journal` et `report` fonctionnent en
//! lecture seule. Phase 1 : `import` écrit `workstation.yaml` depuis l'état lu
//! de la machine, et `diff` le confronte à un scan. Les
//! commandes mutantes sont déclarées mais refusent de s'exécuter — délibérément :
//! la structure de la CLI est le contrat du produit, et il vaut mieux la figer tôt
//! qu'inventer les verbes au fil de l'eau.
//!
//! ## Les deux seuls fichiers que Keystone écrit
//!
//! `ks report` et `ks import`, tous deux là où l'utilisateur le demande, tous
//! deux refusant d'écraser sans `--force` (principe P3). Aucune écriture système,
//! et **aucun processus lancé** : `ks import` affiche la commande git, il ne
//! l'exécute pas, parce qu'un `git commit` déclenche les crochets du dépôt,
//! c'est-à-dire l'exécution d'un fichier du disque que Keystone n'a pas choisi
//! (ADR-0017). Le test `keystone_ne_lance_jamais_de_processus` en fait une
//! barrière plutôt qu'une intention.

#![forbid(unsafe_code)]

mod magasin;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ks_collectors::Inventory;
use ks_core::item::Verdict;
use ks_core::{Domain, DriftSummary, Item};
// Le générateur de rapport vit dans la bibliothèque du crate, pas dans ce
// binaire : la coque graphique `ks-ui` en a besoin, et deux copies d'un
// générateur de rapport divergent (voir `src/lib.rs`). Le chargement de l'état
// désiré y vit pour la même raison, et pour une plus visible encore : sans lui,
// la vue Dérive de la coque reste « sans objet » pour toujours.
use ks_cli::{confrontation, emetteur, lisible, rapport};

/// Plan de contrôle déclaratif pour poste de travail d'ingénieur.
#[derive(Parser)]
#[command(name = "ks", version, about, long_about = None)]
struct Cli {
    /// Sortie machine, stable et versionnée.
    #[arg(long, global = true)]
    json: bool,

    /// Chemin du fichier d'état désiré.
    #[arg(long, global = true, default_value = "workstation.yaml")]
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Posture composite et vitaux. Lecture seule.
    Status,

    /// Collecte l'inventaire. Lecture seule, aucune écriture système.
    Scan {
        /// Limiter à un domaine.
        #[arg(long)]
        domain: Option<String>,
        /// Consigner ce scan dans le journal local de Keystone.
        ///
        /// Absent par défaut, et volontairement : un scan qui journaliserait
        /// sans qu'on l'ait demandé contredirait sa propre bannière, et le
        /// contredirait en silence. C'est le principe P2 — on n'écrit jamais
        /// par omission.
        #[arg(long)]
        record: bool,
    },

    /// Écart entre le fichier d'état désiré et l'état réel. Lecture seule.
    Diff {
        /// Limiter à un domaine.
        #[arg(long)]
        domain: Option<String>,
    },

    /// Adopte l'état lu de la machine comme état désiré (D2-02).
    ///
    /// Aucun formulaire à remplir : Keystone écrit ce qu'il vient de lire. Seuls
    /// les items qui ont vocation à être déclarés y entrent, et jamais ceux dont
    /// la lecture a échoué.
    Import {
        /// Où écrire le fichier.
        #[arg(long, short = 'o', default_value = "workstation.yaml")]
        out: PathBuf,
        /// Remplacer le fichier s'il existe déjà.
        ///
        /// Absent par défaut, comme pour `report` et pour la même raison :
        /// écraser un fichier est irréversible, et un état désiré écrasé est la
        /// source de vérité du poste qui disparaît (principe P3).
        #[arg(long)]
        force: bool,
    },

    /// Fait converger l'état réel vers l'état désiré.
    ///
    /// Simule par défaut. `--apply` est obligatoire pour écrire.
    Converge {
        /// Écrire réellement. Sans ce drapeau, rien n'est modifié.
        #[arg(long)]
        apply: bool,
        /// Limiter à un item.
        #[arg(long)]
        item: Vec<String>,
    },

    /// Construit un plan de mise à jour multi-OS.
    Plan {
        /// Portée : `updates`, `converge`, `space`.
        scope: String,
        /// Limiter à un anneau : `canary`, `stable`, `manual`.
        #[arg(long)]
        ring: Option<String>,
        /// Écrire réellement. Sans ce drapeau, le plan est seulement affiché.
        #[arg(long)]
        apply: bool,
    },

    /// Revient à un instantané.
    Rollback {
        /// Identifiant de l'instantané.
        snapshot_id: String,
    },

    /// Attribution de l'espace disque, et récupération vers la quarantaine.
    Space {
        /// Afficher l'attribution par consommateur.
        #[arg(long)]
        attribute: bool,
        /// Récupérer l'espace — toujours vers la quarantaine, jamais par suppression.
        #[arg(long)]
        reclaim: bool,
    },

    /// Gère la quarantaine (liste, restauration, purge).
    Quarantine {
        #[command(subcommand)]
        action: QuarantineAction,
    },

    /// Sauvegarde des données de travail. Un instantané n'est pas une sauvegarde.
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Journal chaîné. La portée exacte de ce que le chaînage détecte est
    /// affichée par la commande elle-même — et elle n'est pas totale : la
    /// suppression des dernières entrées reste indétectable localement
    /// (ADR-0004). Le mot « inaltérable » figurait ici ; il promettait plus
    /// que ce que le code tient.
    Journal {
        /// Depuis cette date (RFC 3339).
        #[arg(long)]
        since: Option<String>,
        /// Sceller le journal et produire son empreinte.
        #[arg(long)]
        seal: bool,
    },

    /// Isolement d'urgence. Exige une confirmation Windows Hello.
    ///
    /// Coupe le réseau hors canal d'administration, verrouille la session, capture un
    /// instantané forensique et scelle le journal. Préserve les preuves au lieu de les
    /// détruire par un redémarrage réflexe.
    Isolate,

    /// Explique un item : à quoi il sert, quel risque si on le change.
    Explain {
        /// Chemin de l'item, ex. `security.defender.realtime`.
        path: String,
    },

    /// Écrit un rapport HTML autonome de l'état observé.
    ///
    /// Le fichier ne contacte aucun serveur pour s'afficher : ni police
    /// distante, ni script, ni image externe (principe P5).
    Report {
        /// Où écrire le fichier.
        #[arg(long, short = 'o', default_value = "keystone-rapport.html")]
        out: PathBuf,
        /// Remplacer le fichier s'il existe déjà.
        ///
        /// Absent par défaut : écraser un fichier est irréversible, et Keystone
        /// ne fait rien d'irréversible par omission (principe P3).
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum QuarantineAction {
    /// Liste les éléments en quarantaine et leur durée de vie restante.
    List,
    /// Restaure un élément.
    Restore { id: String },
    /// Purge définitivement. Exige Windows Hello.
    Purge { id: String },
}

#[derive(Subcommand)]
enum BackupAction {
    /// Vérifie une restauration sur un échantillon aléatoire.
    ///
    /// Une sauvegarde jamais restaurée n'est pas une sauvegarde.
    Verify,
    /// Restaure un chemin.
    Restore { path: String },
}

/// Code de sortie d'une commande déclarée mais pas encore implémentée.
///
/// Ni 0 (un script croirait la commande exécutée), ni 1 (qu'on réserve aux vraies
/// erreurs). 64 et suivants sont libres au sens de `sysexits.h` ; 69 y signifie
/// « service indisponible », ce qui décrit exactement la situation.
const EXIT_PAS_ENCORE: u8 = 69;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let issue = match &cli.command {
        Command::Scan { domain, record } => cmd_scan(cli.json, domain.as_deref(), *record),
        Command::Journal { since, seal } => cmd_journal(cli.json, since.as_deref(), *seal),
        Command::Status => cmd_status(cli.json),
        Command::Explain { path } => cmd_explain(path),
        Command::Report { out, force } => cmd_report(out, *force),
        Command::Import { out, force } => cmd_import(cli.json, out, *force),
        Command::Diff { domain } => cmd_diff(cli.json, &cli.config, domain.as_deref()),

        // Toutes les commandes qui écriront un jour sont déclarées mais inertes en
        // Phase 0. C'est volontaire : le contrat de la CLI est figé, l'implémentation
        // suit.
        Command::Converge { .. }
        | Command::Plan { .. }
        | Command::Rollback { .. }
        | Command::Space { .. }
        | Command::Quarantine { .. }
        | Command::Backup { .. }
        | Command::Isolate => return not_yet(),
    };

    match issue {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // La règle de voix du projet : une phrase d'abord, le code technique
            // ensuite, jamais un code d'erreur nu (docs/08-CONVENTIONS.md).
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Réponse d'une commande pas encore implémentée.
///
/// Le message dit *où* en est le projet, pas seulement que ça ne marche pas. Un
/// « non implémenté » sec est une impasse ; une phrase qui renvoie à la feuille de
/// route est une information.
fn not_yet() -> ExitCode {
    println!(
        "Cette commande arrive après la Phase 1.\n\
         \n\
         Les phases 0 et 1 sont en lecture seule par conception. Ce qui\n\
         fonctionne : `scan`, `status`, `explain`, `journal`, `report`,\n\
         `import` et `diff`.\n\
         \n\
         Voir docs/07-FEUILLE-DE-ROUTE.md."
    );
    // Le message reste sur la sortie standard — il informe, il n'alarme pas — mais
    // le code de sortie est non nul : un script qui enchaîne des commandes doit
    // pouvoir distinguer « fait » de « pas encore écrit ».
    ExitCode::from(EXIT_PAS_ENCORE)
}

/// Les noms acceptés par `--domain`, dans l'ordre des domaines D1 à D11.
///
/// La forme canonique est celle que produit la sérialisation de [`Domain`] ; les
/// autres sont des alias français ou usuels. `dev-env` figure explicitement :
/// c'est la forme kebab-case qu'affiche `ks scan`, et l'omettre obligeait à
/// deviner que seul `devenv` fonctionnait.
const DOMAINES: &[(&str, Domain)] = &[
    ("inventory", Domain::Inventory),
    ("inventaire", Domain::Inventory),
    ("configuration", Domain::Configuration),
    ("config", Domain::Configuration),
    ("updates", Domain::Updates),
    ("maj", Domain::Updates),
    ("space", Domain::Space),
    ("espace", Domain::Space),
    ("security", Domain::Security),
    ("securite", Domain::Security),
    ("sécurité", Domain::Security),
    ("backup", Domain::Backup),
    ("sauvegarde", Domain::Backup),
    ("identity", Domain::Identity),
    ("identite", Domain::Identity),
    ("profiles", Domain::Profiles),
    ("profils", Domain::Profiles),
    ("virtualization", Domain::Virtualization),
    ("wsl", Domain::Virtualization),
    ("vm", Domain::Virtualization),
    ("peripherals", Domain::Peripherals),
    ("reseau", Domain::Peripherals),
    ("réseau", Domain::Peripherals),
    ("dev-env", Domain::DevEnv),
    ("devenv", Domain::DevEnv),
    ("dev", Domain::DevEnv),
];

fn parse_domain(s: &str) -> Option<Domain> {
    let s = s.to_lowercase();
    DOMAINES.iter().find(|(nom, _)| *nom == s).map(|(_, d)| *d)
}

/// Le filtre demandé, ou une erreur qui nomme les domaines acceptés.
///
/// Un domaine non reconnu doit se dire. Auparavant, `and_then` faisait retomber
/// une faute de frappe sur « aucun filtre » : `ks scan --domain securty`
/// affichait TOUT en silence, ce qui est le pire des deux mondes — l'utilisateur
/// croit avoir filtré. La fonction est partagée par `scan` et `diff` : deux
/// écritures de ce refus finiraient par n'en garder qu'une seule à jour.
fn filtre_de_domaine(domain: Option<&str>) -> Result<Option<Domain>> {
    match domain {
        Some(d) => Ok(Some(parse_domain(d).ok_or_else(|| {
            let mut connus: Vec<&str> = DOMAINES.iter().map(|(nom, _)| *nom).collect();
            connus.sort_unstable();
            anyhow::anyhow!(
                "Le domaine « {d} » n'existe pas, donc rien n'a été filtré.\n\
                 \n\
                 Domaines acceptés : {}",
                connus.join(", ")
            )
        })?)),
        None => Ok(None),
    }
}

fn cmd_scan(json: bool, domain: Option<&str>, record: bool) -> Result<()> {
    let filtre = filtre_de_domaine(domain)?;

    let inv = Inventory::collect_all();

    let items: Vec<_> = match filtre {
        Some(d) => inv.items.iter().filter(|i| i.domain == d).collect(),
        None => inv.items.iter().collect(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    println!("Scan — lecture seule, aucune écriture système.\n");
    for item in &items {
        println!("  {:<44} {}", item.path, lisible::valeur(item));
    }
    println!("\n{} item(s) observé(s).", items.len());

    if record {
        // Le scan entier est consigné en UNE transaction : le journal et les
        // observations décrivent le même instant, ou ni l'un ni l'autre. Un
        // journal qui annoncerait un scan dont les observations manquent serait
        // pire qu'un journal muet — il ferait croire à une série continue.
        let issue = consigner_scan(&inv, &items)?;
        println!(
            "Consigné au journal, entrée {} — {}",
            issue.entree.seq,
            issue.entree.digest()
        );
        println!(
            "  {} observation(s) suivie(s), {} lecture(s) refusée(s).",
            issue.suivis, issue.refusees
        );
    }
    Ok(())
}

/// Ce qu'un scan consigné a produit.
struct Consignation {
    entree: ks_core::JournalEntry,
    suivis: usize,
    refusees: usize,
}

/// Un item entre-t-il dans la série d'observations ?
///
/// La nature décide, et rien d'autre (ADR-0009, ADR-0014). `Mesure` est exclue :
/// `uptime_seconds` change à **chaque** scan, donc produirait un intervalle par
/// scan, donc serait à lui seul la totalité de la croissance du magasin. Les
/// items de nature `Mesure` sont les seuls dont le volume dépende du temps ; les
/// écarter est ce qui rend la rétention inutile plutôt que reportée.
const fn entre_dans_la_serie(nature: ks_core::Nature) -> bool {
    match nature {
        ks_core::Nature::Reglage | ks_core::Nature::Objectif | ks_core::Nature::Constat => true,
        ks_core::Nature::Mesure => false,
    }
}

/// Chemin du journal local.
fn chemin_journal() -> Result<PathBuf> {
    magasin::dossier_donnees()
        .map(|d| d.join("journal.sqlite"))
        .context(
            "impossible de situer le dossier de données : ni LOCALAPPDATA, ni XDG_DATA_HOME, ni HOME",
        )
}

/// Consigne le scan : son entrée de journal, et les observations qui vont avec.
///
/// **L'horodatage est unique pour tout le scan**, et non relu à chaque item :
/// deux items observés au même passage doivent porter la même date, sinon la
/// série des intervalles raconte un ordre qui n'a pas eu lieu.
///
/// Les observations sont enregistrées sur l'inventaire **complet**, jamais sur
/// la vue filtrée par `--domain` : un filtre est une commodité d'affichage, et
/// laisser un filtre décider de ce qui entre dans la série ferait « disparaître »
/// tout ce qu'on n'a pas demandé à voir.
fn consigner_scan(inv: &Inventory, affiches: &[&ks_core::Item]) -> Result<Consignation> {
    let magasin = magasin::Magasin::ouvrir(&chemin_journal()?)?;
    let at = Utc::now();

    let mut suivis = 0_usize;
    let mut refusees = 0_usize;
    for item in &inv.items {
        if !entre_dans_la_serie(item.nature) {
            continue;
        }
        magasin.enregistrer_observation(&item.path, &item.observed, at)?;
        if item.observed.est_constat() {
            suivis += 1;
        } else {
            refusees += 1;
        }
    }

    // Ce que le scan a produit, collecteur par collecteur. C'est ce qui permet
    // de distinguer « l'item a disparu » de « son collecteur a échoué » — deux
    // faits que l'ancien `target: "115 item(s)"` confondait.
    let mut passages: Vec<String> = inv
        .passages()
        .iter()
        .map(|p| format!("{}={}", p.id, p.items))
        .collect();
    passages.sort_unstable();

    let entree = magasin.ajouter(ks_core::JournalEntry {
        seq: 0,
        at,
        // `Keystone`, parce que c'est bien Keystone qui a produit cette ENTRÉE.
        // Les items relevés, eux, restent `Provenance::Observed` : l'outil n'est
        // l'auteur d'aucune des valeurs qu'il a lues.
        actor: ks_core::Actor::System,
        verb: "scan".to_owned(),
        target: format!("{} item(s)", affiches.len()),
        // Le champ était vide. Il porte désormais ce que le scan a vraiment
        // produit, collecteur par collecteur, et le nombre d'observations
        // suivies — donc le chaînage couvre ces faits, puisqu'il couvre `diff`.
        diff: Some(format!(
            "collecteurs {} · suivis {suivis} · refuses {refusees}",
            passages.join(" ")
        )),
        outcome: ks_core::Outcome::Observed,
        prev_digest: String::new(),
    })?;

    Ok(Consignation {
        entree,
        suivis,
        refusees,
    })
}

/// Relit le journal et vérifie son chaînage.
fn cmd_journal(json: bool, since: Option<&str>, seal: bool) -> Result<()> {
    if seal {
        // Sceller exige d'expédier l'empreinte vers une ancre externe, hors de
        // portée d'un attaquant local. C'est SEC-04, Phase 3. Prétendre sceller
        // en écrivant l'empreinte à côté du journal ne protégerait de rien.
        anyhow::bail!(
            "Le scellement expédie l'empreinte vers une ancre externe, ce qui \
             n'existe pas encore (SEC-04, Phase 3).\n\
             \n\
             `ks journal` seul relit et vérifie le chaînage local."
        );
    }

    let chemin = chemin_journal()?;
    if !chemin.exists() {
        println!("Aucun journal : rien n'a encore été consigné.\n");
        println!("  `ks scan --record` enregistre un scan.");
        return Ok(());
    }

    let magasin = magasin::Magasin::ouvrir_en_lecture(&chemin)?;
    let entrees = magasin.lire(since)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entrees)?);
        return Ok(());
    }

    // La vérification porte sur ce qui est LU. Avec `--since`, la séquence
    // commence au milieu de la chaîne : son ancrage est donc légitimement
    // absent, et annoncer « chaînage rompu » serait un faux positif.
    let complet = since.is_none();

    println!("Journal — {} entrée(s).\n", entrees.len());
    for e in &entrees {
        println!(
            "  {:>4}  {}  {:<10} {}",
            e.seq,
            e.at.to_rfc3339(),
            e.verb,
            e.target
        );
    }

    if complet {
        let intact = ks_core::JournalEntry::verify_chain(&entrees);
        let empreintes = magasin.premiere_empreinte_incoherente()?;
        println!(
            "\n  Chaînage : {}",
            match (intact, empreintes) {
                (false, _) => "ROMPU — une entrée a été retirée au début, ou réécrite".to_owned(),
                (true, Some(seq)) =>
                    format!("ROMPU — l'entrée {seq} ne correspond plus à son empreinte"),
                (true, None) => "intact, ancré sur la genèse".to_owned(),
            }
        );
        // Ce que la portée disait avant : « détecte la corruption et
        // l'altération non privilégiée ». C'était faux dans les deux sens où ça
        // comptait. Le chaînage relie chaque entrée à la précédente, donc la
        // dernière n'est reliée à rien ; et rien ne relisait l'empreinte
        // stockée. Supprimer les dernières entrées, ou réécrire la dernière,
        // laissait Keystone répondre « intact » — précisément à l'adversaire A1
        // du modèle de menace, qui écrit dans %LOCALAPPDATA% sans élévation.
        println!(
            "  Portée   : détecte la corruption, le retrait d'une entrée au début ou au\n\
             \x20            milieu, et la réécriture d'une charge utile.\n\
             \x20            Ne détecte PAS le retrait des dernières entrées : une séquence\n\
             \x20            plus courte reste cohérente. Aucun contrôle local ne le peut ;\n\
             \x20            c'est la raison d'être de l'export hors du poste (SEC-04)."
        );
    } else {
        println!("\n  Chaînage : non vérifié — un extrait n'a pas d'ancrage.");
    }
    Ok(())
}

/// Écrit le rapport HTML autonome.
///
/// La seule commande de la Phase 0 qui produise un fichier. Elle écrit **où
/// l'utilisateur le demande**, jamais dans un dossier système : la règle « aucune
/// écriture » vise la configuration de la machine, pas un rapport que l'on
/// réclame explicitement. La distinction est écrite ici pour qu'elle ne se
/// perde pas.
fn cmd_report(destination: &std::path::Path, force: bool) -> Result<()> {
    let inv = Inventory::collect_all();
    let machine = rapport::nom_machine(&inv.items);

    let html = rapport::construire(&inv.items, &machine, &Utc::now().to_rfc3339());

    // `fs::write` tronque le fichier existant. `ks report -o notes.html`
    // détruisait donc `notes.html`, sans le dire et sans retour arrière —
    // principe P3. `create_new` refuse à la place, et la commande dit comment
    // passer outre. C'est la seule commande de la Phase 0 qui écrive un
    // fichier ; elle n'a pas le droit d'en détruire un.
    if destination.exists() && !force {
        anyhow::bail!(
            "« {} » existe déjà, et Keystone n'écrase pas un fichier sans qu'on le demande.\n\
             Ajoutez --force pour le remplacer, ou choisissez un autre chemin avec -o.",
            destination.display()
        );
    }
    std::fs::write(destination, html)
        .with_context(|| format!("écriture de « {} »", destination.display()))?;

    println!("Rapport écrit : {}", destination.display());
    println!("  {} items, sur {machine}.", inv.items.len());
    println!("  Aucune ressource réseau : le fichier s'ouvre hors ligne.");
    Ok(())
}

/// Écrit `workstation.yaml` depuis l'état lu de la machine — le moment M1.
///
/// « J'ai lu votre machine. Rien n'a été modifié. » La commande tient cette
/// phrase à la lettre : elle collecte, elle traduit, elle dépose **un** fichier
/// là où on le lui demande, et elle ne lance rien.
fn cmd_import(json: bool, destination: &std::path::Path, force: bool) -> Result<()> {
    let inv = Inventory::collect_all();
    let machine = rapport::nom_machine(&inv.items);
    let emission = emetteur::emettre(&inv.items, &machine);

    // Même refus que `ks report`, et pour la même raison : écraser un fichier
    // est irréversible, et Keystone ne fait rien d'irréversible par omission
    // (principe P3). Un état désiré écrasé, c'est la source de vérité du poste
    // qui disparaît sans retour arrière.
    if destination.exists() && !force {
        anyhow::bail!(
            "« {} » existe déjà, et Keystone n'écrase pas un fichier sans qu'on le demande.\n\
             Ajoutez --force pour le remplacer, ou choisissez un autre chemin avec -o.",
            destination.display()
        );
    }
    ecrire_atomiquement(destination, &emission.yaml)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "file": destination.display().to_string(),
                "machine": machine,
                "observed": inv.len(),
                "declarations": emission.declarations,
                "unreadable": emission.illisibles.iter()
                    .map(|(chemin, raison)| serde_json::json!({ "path": chemin, "reason": raison }))
                    .collect::<Vec<_>>(),
            }))?
        );
        return Ok(());
    }

    // La phrase du moment M1, telle que `design/ecrans-et-libelles.md` la fixe,
    // mot pour mot : c'est la thèse du produit en une ligne, et la CLI la dit
    // comme l'écran la dira.
    println!("J'ai lu votre machine. Rien n'a été modifié.\n");
    println!("Fichier écrit : {}", destination.display());
    println!(
        "  {} déclaration(s), sur {} item(s) observé(s) — seuls les réglages et les\n  \
         objectifs se déclarent.",
        emission.declarations,
        inv.len()
    );
    if !emission.illisibles.is_empty() {
        println!(
            "  {} item(s) non déclaré(s), faute d'avoir pu les lire :",
            emission.illisibles.len()
        );
        for (chemin, raison) in &emission.illisibles {
            println!("    {chemin:<44} {raison}");
        }
    }

    // ADR-0017 : la commande est proposée, jamais exécutée. Un `git commit`
    // déclenche les crochets du dépôt — des fichiers exécutables que git lance
    // sans rien demander — donc Keystone exécuterait un fichier arbitraire du
    // disque. C'est le geste que la doctrine du projet refuse sous le nom
    // `RunScript { path }`.
    println!("\nPour le versionner :");
    println!(
        "{}",
        emetteur::proposition_git(destination, &Utc::now().date_naive().to_string())
    );
    println!(
        "\n  Keystone n'exécute pas git : un commit déclenche les crochets du dépôt,\n  \
         c'est-à-dire l'exécution d'un fichier du disque qu'il n'a pas choisi."
    );
    Ok(())
}

/// Écrit un fichier sans jamais en laisser un tronqué derrière soi.
///
/// Fichier temporaire dans le **même répertoire**, puis renommage : un
/// `ks import` interrompu laisse l'ancien fichier intact, là où une écriture
/// directe laisserait la source de vérité d'un outil de sécurité amputée en
/// silence. Le même répertoire, parce qu'un renommage entre volumes n'est pas
/// atomique — et `%TEMP%` n'est pas toujours sur le volume de destination.
///
/// Ce que ça ne couvre pas, et l'ADR-0017 le consigne : deux `ks import`
/// simultanés se terminent par deux renommages, et le dernier gagne.
fn ecrire_atomiquement(destination: &std::path::Path, contenu: &str) -> Result<()> {
    let nom = destination
        .file_name()
        .with_context(|| format!("« {} » ne nomme pas un fichier", destination.display()))?;
    let mut nom_temporaire = nom.to_os_string();
    nom_temporaire.push(".ks-import-tmp");
    let temporaire = destination.with_file_name(nom_temporaire);

    std::fs::write(&temporaire, contenu)
        .with_context(|| format!("écriture de « {} »", temporaire.display()))?;
    std::fs::rename(&temporaire, destination)
        .inspect_err(|_| {
            // Un temporaire abandonné à côté du fichier de l'utilisateur serait
            // une trace de plus qu'on n'a pas demandée.
            let _ = std::fs::remove_file(&temporaire);
        })
        .with_context(|| format!("mise en place de « {} »", destination.display()))?;
    Ok(())
}

/// Confronte le fichier d'état désiré à ce que la machine porte aujourd'hui.
///
/// Un écart n'est pas une erreur de la commande : le code de sortie dit si la
/// comparaison a **eu lieu**, jamais ce qu'elle a trouvé. Un script qui veut
/// agir sur les écarts lit la sortie `--json`.
fn cmd_diff(json: bool, config: &str, domain: Option<&str>) -> Result<()> {
    let filtre = filtre_de_domaine(domain)?;
    let chemin = std::path::Path::new(config);

    let document = confrontation::charger(chemin)?;
    let mut inv = Inventory::collect_all();
    // La confrontation porte sur l'inventaire COMPLET, jamais sur la vue
    // filtrée : `--domain` est une commodité d'affichage, et laisser un filtre
    // décider de ce qui est comparé ferait passer un chemin déclaré pour
    // « non observé » alors qu'il l'a été.
    let bilan = confrontation::confronter(&document, &mut inv.items)?;

    let items: Vec<&Item> = match filtre {
        Some(d) => inv.items.iter().filter(|i| i.domain == d).collect(),
        None => inv.items.iter().collect(),
    };

    let mut non_contraints = 0_usize;
    let mut conformes = 0_usize;
    let mut ecarts: Vec<&Item> = Vec::new();
    let mut incomparables: Vec<(&Item, String)> = Vec::new();
    for item in &items {
        match confrontation::verdict_publie(item) {
            Verdict::NonContraint => non_contraints += 1,
            Verdict::Conforme => conformes += 1,
            Verdict::Ecart => ecarts.push(item),
            Verdict::Incomparable { raison } => incomparables.push((item, raison)),
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "config": chemin.display().to_string(),
                // `observed` compte les items AFFICHÉS, donc filtrés. Le filtre
                // voyage avec eux : un décompte partiel sans son critère est un
                // décompte qu'un script lira pour un total.
                "domain": filtre.map(rapport::titre_domaine),
                "observed": items.len(),
                "declared": bilan.declarees,
                "unconstrained": non_contraints,
                "compliant": conformes,
                "deviations": ecarts.iter().map(|i| serde_json::json!({
                    "path": i.path,
                    "desired": i.desired,
                    "observed": i.observed,
                })).collect::<Vec<_>>(),
                "incomparable": incomparables.iter().map(|(i, raison)| serde_json::json!({
                    "path": i.path,
                    "reason": raison,
                })).collect::<Vec<_>>(),
                "declaredNotObserved": bilan.non_observes,
            }))?
        );
        return Ok(());
    }

    println!("Écart entre l'état désiré et l'état réel — lecture seule.\n");
    println!("  Fichier            {}", chemin.display());
    println!(
        "  Déclarations       {} sur {} item(s) observé(s)",
        bilan.declarees,
        inv.items.len()
    );
    if let Some(d) = filtre {
        println!(
            "  Domaine            {} seulement",
            rapport::titre_domaine(d)
        );
    }
    println!("\n  écarts             {}", ecarts.len());
    println!("  conformes          {conformes}");
    println!("  incomparables      {}", incomparables.len());
    println!("  non contraints     {non_contraints}");

    if !ecarts.is_empty() {
        println!("\n  Écarts — la valeur constatée diffère de celle qui est déclarée :");
        for item in &ecarts {
            let voulu = item
                .desired
                .as_ref()
                .map_or_else(|| "-".to_owned(), lisible::libelle);
            println!(
                "    {:<44} déclaré {voulu}, constaté {}",
                item.path,
                lisible::valeur(item)
            );
        }
    }

    // ADR-0008 : un incomparable se lit **une ligne à la fois**, jamais en
    // agrégat. Un item dont la lecture a échoué n'est ni conforme ni en écart,
    // et le taire serait afficher du vert sur ce qu'on n'a pas regardé.
    if !incomparables.is_empty() {
        println!("\n  Incomparables — la comparaison n'a pas de sens, faute d'avoir pu lire :");
        for (item, raison) in &incomparables {
            println!("    {:<44} {raison}", item.path);
        }
    }

    if !bilan.non_observes.is_empty() {
        println!("\n  Déclarés, non observés — deux causes possibles, et rien ne les distingue :");
        println!("  une faute de frappe dans le chemin, ou un item qui a légitimement disparu.");
        for chemin in &bilan.non_observes {
            println!("    {chemin}");
        }
    }

    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let inv = Inventory::collect_all();
    // En Phase 0 il n'y a pas encore de fichier d'état désiré chargé : donc aucun
    // écart possible, par construction. Le résumé le dit honnêtement plutôt que
    // d'afficher un « 100 / 100 » qui ne voudrait rien dire.
    // Les illisibles se comptent, ils ne se déduisent pas. En Phase 0 aucun état
    // désiré n'existe, donc aucun écart — mais trois items ne sont pas lisibles
    // sans élévation, et les laisser tomber dans `compliant` afficherait vert
    // sur ce qu'on n'a pas regardé.
    let illisibles = inv
        .items
        .iter()
        .filter(|i| !i.observed.est_constat())
        .count();
    let summary = DriftSummary::build(inv.len(), illisibles, &[], chrono::Utc::now());

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("Keystone — Phase 0\n");
    println!("  items observés     {}", summary.observed);
    println!("  écarts actifs      {}", summary.active);
    println!("  sans auteur        {}", summary.unattributed);
    println!(
        "\n  `ks status` ne charge aucun fichier d'état désiré : la posture n'est donc\n  \
         pas calculable ici, et un « 100 / 100 » ne voudrait rien dire.\n  \
         `ks import` adopte l'état lu de la machine, `ks diff` le confronte au scan\n  \
         et publie les écarts (exigence D2-02)."
    );
    Ok(())
}

fn cmd_explain(path: &str) -> Result<()> {
    let inv = Inventory::collect_all();
    match inv.items.iter().find(|i| i.path == path) {
        Some(item) => {
            println!("{}\n", item.path);
            // Le libellé, pas le jeton : le relevé porte `imposee`, l'écran
            // affiche « imposée » (ADR-0015). La conversion d'octets n'est pas
            // appliquée — `explain` a toujours montré la valeur machine, et
            // c'est aussi elle qu'on écrira dans `workstation.yaml`.
            println!("  valeur constatée   {}", lisible::libelle(&item.observed));
            println!("  observée le        {}", item.observed_at.to_rfc3339());
            println!("  source             {:?}", item.provenance);
            println!("\n  À quoi ça sert     {}", item.purpose);
            println!("  Risque si changé   {}", item.risk);
            if let Some(r) = &item.reference {
                println!("  Référence          {r}");
            }
        }
        None => {
            println!("Item inconnu : « {path} ».\n\nListe des items observés : `ks scan`.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    /// Ce qui entre dans la série est décidé par la nature, et par rien d'autre.
    ///
    /// La barrière la plus forte est le `match` exhaustif sans bras `_` de
    /// `entre_dans_la_serie` : ajouter une variante à `Nature` casse la
    /// **compilation**. Ce test-ci garde l'autre moitié — qu'aucune des quatre
    /// classifications ne change d'avis en silence.
    ///
    /// `Mesure` est le cas qui compte. `uptime_seconds` change à chaque scan :
    /// l'y laisser entrer produirait un intervalle par scan, donc à lui seul la
    /// totalité de la croissance du magasin, et rendrait la rétention
    /// nécessaire là où elle est aujourd'hui inutile.
    #[test]
    fn seule_la_nature_decide_de_ce_qui_entre_dans_la_serie() {
        use ks_core::Nature;

        assert!(entre_dans_la_serie(Nature::Reglage));
        assert!(entre_dans_la_serie(Nature::Objectif));
        assert!(
            entre_dans_la_serie(Nature::Constat),
            "un constat qui change est un événement, pas une dérive — mais c'est              un fait daté, et il se stocke"
        );
        assert!(
            !entre_dans_la_serie(Nature::Mesure),
            "une mesure change à chaque scan : l'y laisser entrer ferait croître              le magasin avec le TEMPS et non avec le changement"
        );
    }

    /// Un relevé illisible ne fabrique pas d'intervalle, mesuré de bout en bout.
    ///
    /// Vérifié sur une base réelle, à travers la même méthode que le scan
    /// appelle. Sans cette garde, une panne de lecture de trois jours
    /// produirait deux « changements » sur un item qui n'a jamais bougé : un
    /// vers l'illisible, un pour en revenir.
    #[test]
    fn un_releve_illisible_nentre_jamais_dans_la_serie() {
        use ks_core::ItemValue;

        let chemin = std::env::temp_dir().join("ks-cablage-illisible.sqlite");
        for suffixe in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffixe}", chemin.display()));
        }
        let m = magasin::Magasin::ouvrir(&chemin).expect("magasin");
        let t = Utc::now();

        m.enregistrer_observation("a.b", &ItemValue::Bool(true), t)
            .expect("valeur lisible");
        m.enregistrer_observation("a.b", &ItemValue::illisible("accès refusé"), t)
            .expect("valeur illisible");

        let serie = m.serie("a.b").expect("série");
        assert_eq!(
            serie.len(),
            1,
            "l'aveu ne doit pas ouvrir un second intervalle"
        );
        assert!(!serie[0].closed, "ni fermer le premier");

        let _ = std::fs::remove_file(&chemin);
    }
    use super::*;

    #[test]
    fn un_domaine_inconnu_est_refuse_plutot_quignore() {
        // Le vrai piège n'est pas de refuser : c'est d'accepter en silence. Avant,
        // `--domain securty` retombait sur « aucun filtre » et affichait tout
        // l'inventaire, en laissant croire à un filtrage.
        assert!(parse_domain("securty").is_none());
        assert!(parse_domain("").is_none());
    }

    #[test]
    fn les_alias_dun_domaine_designent_le_meme_domaine() {
        assert_eq!(parse_domain("security"), Some(Domain::Security));
        assert_eq!(parse_domain("securite"), Some(Domain::Security));
        assert_eq!(parse_domain("sécurité"), Some(Domain::Security));
        assert_eq!(parse_domain("SÉCURITÉ"), Some(Domain::Security));
    }

    #[test]
    fn la_forme_affichee_par_scan_est_acceptee_par_domain() {
        // `ks scan` imprime les chemins d'items en kebab-case, `dev-env` compris.
        // Ne pas accepter en entrée ce qu'on produit en sortie oblige l'utilisateur
        // à deviner — et c'était le cas : seul `devenv` fonctionnait.
        for (nom, attendu) in DOMAINES {
            assert_eq!(
                parse_domain(nom),
                Some(*attendu),
                "l'alias « {nom} » devrait résoudre"
            );
        }
        assert_eq!(parse_domain("dev-env"), Some(Domain::DevEnv));
    }

    /// Keystone ne lance aucun processus, git compris (ADR-0017).
    ///
    /// `ks import` **affiche** la commande git et ne l'exécute pas : un
    /// `git commit` déclenche `pre-commit`, `commit-msg` et `post-commit`, qui
    /// sont des fichiers exécutables posés dans `.git/hooks` et que git lance
    /// sans rien demander. Keystone exécuterait alors un fichier arbitraire du
    /// disque — le geste que la doctrine du projet refuse sous le nom
    /// `RunScript { path }`.
    ///
    /// La barrière lit **tout** `src/`, y compris les fichiers ajoutés après
    /// elle : viser `main.rs` seul laisserait la porte ouverte d'un module
    /// voisin. Les motifs sont assemblés par `concat!` pour que ce fichier-ci
    /// ne contienne pas lui-même ce qu'il interdit.
    ///
    /// **Sa limite est assumée**, comme celle de la barrière SEC-02 : un
    /// lancement écrit autrement — un alias de type, une macro — passerait. Un
    /// test grossier ne remplace ni la revue ni l'ADR ; il rend seulement le
    /// geste évident impossible à commettre par distraction.
    #[test]
    fn keystone_ne_lance_jamais_de_processus() {
        let interdits = [
            concat!("Command", "::new"),
            concat!("process", "::Command"),
            concat!(".spawn", "("),
        ];

        let racine = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut lus = 0;
        for entree in std::fs::read_dir(&racine).expect("le dossier src doit être lisible") {
            let chemin = entree.expect("entrée de dossier").path();
            if chemin.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&chemin).expect("source lisible");
            lus += 1;
            for motif in interdits {
                assert!(
                    !source.contains(motif),
                    "« {} » lance un processus ({motif}) : ADR-0017 l'interdit en Phase 1",
                    chemin.display()
                );
            }
        }
        assert!(
            lus >= 5,
            "seuls {lus} fichiers lus dans {} — la barrière ne garde plus rien",
            racine.display()
        );
    }

    #[test]
    fn une_commande_pas_encore_ecrite_ne_sort_pas_en_succes() {
        // Un script qui enchaîne des commandes doit pouvoir distinguer « fait » de
        // « pas encore écrit ». `Ok(())` rendait les deux indiscernables.
        assert_ne!(EXIT_PAS_ENCORE, 0);
        assert_ne!(EXIT_PAS_ENCORE, 1, "1 reste réservé aux vraies erreurs");
    }
}

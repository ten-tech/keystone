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
//! Phase 0 : `scan`, `status` et `explain` fonctionnent en lecture seule. Les
//! commandes mutantes sont déclarées mais refusent de s'exécuter — délibérément :
//! la structure de la CLI est le contrat du produit, et il vaut mieux la figer tôt
//! qu'inventer les verbes au fil de l'eau.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use ks_collectors::Inventory;
use ks_core::{Domain, DriftSummary};

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
    },

    /// Écart entre le fichier d'état désiré et l'état réel. Lecture seule.
    Diff {
        /// Limiter à un domaine.
        #[arg(long)]
        domain: Option<String>,
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

    /// Journal inaltérable.
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Scan { domain } => cmd_scan(cli.json, domain.as_deref()),
        Command::Status => cmd_status(cli.json),
        Command::Explain { path } => cmd_explain(path),

        // Toutes les commandes mutantes sont déclarées mais inertes en Phase 0.
        // C'est volontaire : le contrat de la CLI est figé, l'implémentation suit.
        Command::Diff { .. }
        | Command::Converge { .. }
        | Command::Plan { .. }
        | Command::Rollback { .. }
        | Command::Space { .. }
        | Command::Quarantine { .. }
        | Command::Backup { .. }
        | Command::Journal { .. }
        | Command::Isolate => not_yet(),
    }
}

/// Réponse d'une commande pas encore implémentée.
///
/// Le message dit *où* en est le projet, pas seulement que ça ne marche pas. Un
/// « non implémenté » sec est une impasse ; une phrase qui renvoie à la feuille de
/// route est une information.
fn not_yet() -> Result<()> {
    println!(
        "Cette commande arrive après la Phase 0.\n\
         \n\
         La Phase 0 est en lecture seule par conception : `scan`, `status` et `explain`\n\
         fonctionnent, rien d'autre n'écrit. Voir docs/07-FEUILLE-DE-ROUTE.md."
    );
    Ok(())
}

fn parse_domain(s: &str) -> Option<Domain> {
    match s.to_lowercase().as_str() {
        "inventory" | "inventaire" => Some(Domain::Inventory),
        "configuration" | "config" => Some(Domain::Configuration),
        "updates" | "maj" => Some(Domain::Updates),
        "space" | "espace" => Some(Domain::Space),
        "security" | "securite" | "sécurité" => Some(Domain::Security),
        "backup" | "sauvegarde" => Some(Domain::Backup),
        "identity" | "identite" => Some(Domain::Identity),
        "profiles" | "profils" => Some(Domain::Profiles),
        "virtualization" | "wsl" | "vm" => Some(Domain::Virtualization),
        "peripherals" | "reseau" | "réseau" => Some(Domain::Peripherals),
        "devenv" | "dev" => Some(Domain::DevEnv),
        _ => None,
    }
}

fn cmd_scan(json: bool, domain: Option<&str>) -> Result<()> {
    let inv = Inventory::collect_all();

    let items: Vec<_> = match domain.and_then(parse_domain) {
        Some(d) => inv.items.iter().filter(|i| i.domain == d).collect(),
        None => inv.items.iter().collect(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    println!("Scan — lecture seule, aucune écriture système.\n");
    for item in &items {
        println!("  {:<44} {}", item.path, item.observed);
    }
    println!("\n{} item(s) observé(s).", items.len());
    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let inv = Inventory::collect_all();
    // En Phase 0 il n'y a pas encore de fichier d'état désiré chargé : donc aucun
    // écart possible, par construction. Le résumé le dit honnêtement plutôt que
    // d'afficher un « 100 / 100 » qui ne voudrait rien dire.
    let summary = DriftSummary::build(inv.len(), &[], chrono::Utc::now());

    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("Keystone — Phase 0\n");
    println!("  items observés     {}", summary.observed);
    println!("  écarts actifs      {}", summary.active);
    println!("  sans auteur        {}", summary.unattributed);
    println!(
        "\n  Aucun fichier d'état désiré chargé : la posture n'est pas encore calculable.\n  \
         `ks diff` deviendra utile en Phase 1, quand workstation.yaml sera généré\n  \
         depuis l'état réel de la machine (exigence D2-02)."
    );
    Ok(())
}

fn cmd_explain(path: &str) -> Result<()> {
    let inv = Inventory::collect_all();
    match inv.items.iter().find(|i| i.path == path) {
        Some(item) => {
            println!("{}\n", item.path);
            println!("  valeur constatée   {}", item.observed);
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

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

use std::process::ExitCode;

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

/// Code de sortie d'une commande déclarée mais pas encore implémentée.
///
/// Ni 0 (un script croirait la commande exécutée), ni 1 (qu'on réserve aux vraies
/// erreurs). 64 et suivants sont libres au sens de `sysexits.h` ; 69 y signifie
/// « service indisponible », ce qui décrit exactement la situation.
const EXIT_PAS_ENCORE: u8 = 69;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let issue = match &cli.command {
        Command::Scan { domain } => cmd_scan(cli.json, domain.as_deref()),
        Command::Status => cmd_status(cli.json),
        Command::Explain { path } => cmd_explain(path),

        // Toutes les commandes qui écriront un jour sont déclarées mais inertes en
        // Phase 0. C'est volontaire : le contrat de la CLI est figé, l'implémentation
        // suit. `diff` et `journal` ne mutent rien non plus, mais dépendent d'un état
        // désiré et d'une persistance qui n'existent pas encore.
        Command::Diff { .. }
        | Command::Converge { .. }
        | Command::Plan { .. }
        | Command::Rollback { .. }
        | Command::Space { .. }
        | Command::Quarantine { .. }
        | Command::Backup { .. }
        | Command::Journal { .. }
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
        "Cette commande arrive après la Phase 0.\n\
         \n\
         La Phase 0 est en lecture seule par conception : `scan`, `status` et `explain`\n\
         fonctionnent, rien d'autre n'écrit. Voir docs/07-FEUILLE-DE-ROUTE.md."
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

fn cmd_scan(json: bool, domain: Option<&str>) -> Result<()> {
    // Un domaine non reconnu doit se dire. Auparavant, `and_then` faisait retomber
    // une faute de frappe sur « aucun filtre » : `ks scan --domain securty`
    // affichait TOUT en silence, ce qui est le pire des deux mondes — l'utilisateur
    // croit avoir filtré.
    let filtre = match domain {
        Some(d) => Some(parse_domain(d).ok_or_else(|| {
            let mut connus: Vec<&str> = DOMAINES.iter().map(|(nom, _)| *nom).collect();
            connus.sort_unstable();
            anyhow::anyhow!(
                "Le domaine « {d} » n'existe pas, donc rien n'a été filtré.\n\
                 \n\
                 Domaines acceptés : {}",
                connus.join(", ")
            )
        })?),
        None => None,
    };

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
        println!("  {:<44} {}", item.path, valeur_lisible(item));
    }
    println!("\n{} item(s) observé(s).", items.len());
    Ok(())
}

/// Rend une valeur lisible par un humain, sans toucher au modèle.
///
/// L'item porte des octets, parce qu'un octet est ce que la machine a mesuré, et
/// que la Phase 1 comparera des octets. Mais `56043241472` à l'écran ne dit rien
/// à personne, et le principe P6 refuse ce qu'on ne peut pas comprendre. La
/// conversion appartient donc à l'affichage, jamais au relevé.
fn valeur_lisible(item: &ks_core::Item) -> String {
    if let (true, ks_core::ItemValue::Int(n)) = (item.path.ends_with("_bytes"), &item.observed) {
        if let Ok(octets) = u64::try_from(*n) {
            return octets_lisibles(octets);
        }
    }
    item.observed.to_string()
}

/// Formate une taille en unités binaires, avec la ponctuation française.
///
/// Une décimale suffit : la deuxième donnerait une précision que ni le calcul ni
/// le besoin ne justifient. L'espace avant l'unité est **insécable**, sans quoi le
/// terminal coupe « 52,2 » et « Gio » sur deux lignes.
fn octets_lisibles(octets: u64) -> String {
    const UNITES: [&str; 5] = ["o", "Kio", "Mio", "Gio", "Tio"];
    let mut valeur = octets as f64;
    let mut rang = 0;
    while valeur >= 1024.0 && rang < UNITES.len() - 1 {
        valeur /= 1024.0;
        rang += 1;
    }
    let arrondi = if rang == 0 {
        format!("{octets}")
    } else {
        format!("{valeur:.1}").replace('.', ",")
    };
    format!("{arrondi}\u{a0}{}", UNITES[rang])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_taille_saffiche_en_unites_lisibles() {
        // La valeur relevée sur la machine de référence. « 56043241472 » ne dit
        // rien à personne ; « 52,2 Gio » situe immédiatement le disque WSL comme
        // le premier poste d'occupation du poste.
        assert_eq!(octets_lisibles(56_043_241_472), "52,2\u{a0}Gio");
        assert_eq!(octets_lisibles(100_663_296), "96,0\u{a0}Mio");
        // En deçà du kibioctet, l'arrondi n'apporte rien : on garde l'entier.
        assert_eq!(octets_lisibles(0), "0\u{a0}o");
        assert_eq!(octets_lisibles(512), "512\u{a0}o");
        assert_eq!(octets_lisibles(1024), "1,0\u{a0}Kio");
    }

    #[test]
    fn la_virgule_est_francaise_et_lespace_insecable() {
        let rendu = octets_lisibles(1_610_612_736);
        assert!(rendu.contains(','), "séparateur décimal français");
        assert!(!rendu.contains('.'), "jamais le point décimal anglais");
        assert!(
            rendu.contains('\u{a0}'),
            "espace insécable : sinon le terminal coupe le nombre de son unité"
        );
        assert!(!rendu.contains(' '), "aucune espace ordinaire");
    }

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

    #[test]
    fn une_commande_pas_encore_ecrite_ne_sort_pas_en_succes() {
        // Un script qui enchaîne des commandes doit pouvoir distinguer « fait » de
        // « pas encore écrit ». `Ok(())` rendait les deux indiscernables.
        assert_ne!(EXIT_PAS_ENCORE, 0);
        assert_ne!(EXIT_PAS_ENCORE, 1, "1 reste réservé aux vraies erreurs");
    }
}

//! # ks-agent — l'agent satellite
//!
//! Un binaire ELF statique (`x86_64-unknown-linux-musl`) déposé dans chaque
//! distribution WSL2 et chaque VM Linux. **Il parle le même protocole et produit le
//! même schéma d'items que l'hôte** : c'est ce qui permet une vue consolidée
//! multi-OS sans dupliquer la logique de collecte.
//!
//! ## Il se compile depuis Windows
//!
//! Volontairement. Un seul dépôt, un seul `cargo build`, aucune synchronisation de
//! clones à maintenir :
//!
//! ```powershell
//! .\scripts\build-agent.ps1
//! ```
//!
//! On n'entre dans WSL que pour *exécuter* l'agent. Copier un binaire à travers
//! `/mnt` ne coûte rien ; y compiler un workspace, si.
//!
//! ## Canal de transport
//!
//! * **VM Hyper-V** : Hyper-V Socket (`AF_HYPERV`). Choisi précisément parce qu'il
//!   fonctionne sans réseau dans l'invité — indispensable quand la VM est justement
//!   cassée au niveau réseau, c'est-à-dire le moment où on en a le plus besoin.
//! * **Distros WSL2** : socket Unix relayé par l'hôte.
//!
//! Voir `docs/adr/0002-grpc-sur-named-pipe.md`.
//!
//! ## Lecture seule, comme l'hôte
//!
//! L'agent partage la règle de `ks-collectors` : il observe, il n'écrit pas. Les
//! opérations mutantes dans une distro (mise à jour de paquets, compaction du
//! `vhdx`) sont demandées par le broker, avec instantané préalable — et dans une
//! distro l'instantané est instantané, ce qui autorise une automatisation bien plus
//! poussée que sur l'hôte (exigence D3-15).

#![forbid(unsafe_code)]

use anyhow::Result;
use ks_collectors::Inventory;

/// Forme de sortie demandée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sortie {
    /// Lisible par un humain.
    Humaine,
    /// Machine, stable et versionnée.
    Json,
}

/// Analyse les arguments, **et refuse ce qu'elle ne connaît pas**.
///
/// L'agent se contentait de chercher `--json` parmi les arguments et ignorait
/// tout le reste. `ks-agent --josn` affichait donc la sortie humaine sans un
/// mot, alors que l'appelant croyait avoir demandé du JSON — et un script qui
/// tente de la lire échoue plus loin, sur une erreur qui ne désigne pas la cause.
///
/// C'est le même défaut que `ks scan --domain securty`, corrigé côté CLI et
/// resté ici : une faute de frappe qui retombe sur un défaut silencieux est le
/// pire des deux mondes.
///
/// # Erreurs
///
/// Renvoie le message destiné à l'utilisateur si un argument n'est pas reconnu.
pub fn analyser(arguments: &[String]) -> Result<Sortie, String> {
    let mut sortie = Sortie::Humaine;
    // Le premier argument est le chemin du binaire.
    for argument in arguments.iter().skip(1) {
        match argument.as_str() {
            "--json" => sortie = Sortie::Json,
            inconnu => {
                return Err(format!(
                    "L'argument « {inconnu} » n'existe pas, donc rien n'a été pris en compte.\n\
                     \n\
                     Arguments acceptés : --json"
                ))
            }
        }
    }
    Ok(sortie)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let json = match analyser(&args) {
        Ok(sortie) => sortie == Sortie::Json,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    let inv = Inventory::collect_all();

    if json {
        println!("{}", serde_json::to_string_pretty(&inv.items)?);
        return Ok(());
    }

    println!("ks-agent — scan local, lecture seule.\n");
    for item in &inv.items {
        println!("  {:<44} {}", item.path, item.observed);
    }
    println!("\n{} item(s) observé(s).", inv.len());
    println!(
        "\nPhase 0 : l'agent scanne et se tait. Le mode service (systemd) et le canal\n\
         vers le broker arrivent en Phase 1 — voir docs/07-FEUILLE-DE-ROUTE.md."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{analyser, Sortie};

    fn args(suite: &[&str]) -> Vec<String> {
        std::iter::once("ks-agent")
            .chain(suite.iter().copied())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn sans_argument_la_sortie_est_humaine() {
        assert_eq!(analyser(&args(&[])), Ok(Sortie::Humaine));
    }

    #[test]
    fn json_se_demande_explicitement() {
        assert_eq!(analyser(&args(&["--json"])), Ok(Sortie::Json));
    }

    #[test]
    fn un_argument_inconnu_est_refuse_plutot_quignore() {
        // Le vrai piège n'est pas de refuser, c'est d'accepter en silence.
        // Avant, `--josn` retombait sur la sortie humaine sans un mot : un
        // script qui tente de lire du JSON échoue alors plus loin, sur une
        // erreur qui ne désigne pas la cause.
        let erreur = analyser(&args(&["--josn"])).expect_err("une faute doit se dire");
        assert!(erreur.contains("--josn"), "l'argument fautif est cité");
        assert!(
            erreur.contains("--json"),
            "et la forme correcte est proposée"
        );

        // Y compris quand il suit un argument valide : on ne s'arrête pas au
        // premier drapeau reconnu.
        assert!(analyser(&args(&["--json", "--verbeux"])).is_err());
    }

    #[test]
    fn le_chemin_du_binaire_nest_pas_un_argument() {
        // `args()` rend le chemin d'exécution en premier. L'ignorer n'est pas un
        // détail : sans le saut, tout lancement échouerait sur son propre nom.
        let avec_chemin = vec!["/usr/local/bin/ks-agent".to_owned(), "--json".to_owned()];
        assert_eq!(analyser(&avec_chemin), Ok(Sortie::Json));
    }
}

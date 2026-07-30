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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let json = args.iter().any(|a| a == "--json");

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

//! Les items de la machine de référence, figés pour les tests.
//!
//! ## Pourquoi une machine en dur plutôt qu'un scan
//!
//! Un test qui appelle `Inventory::collect_all()` mesure la machine qui
//! l'exécute. Sur l'hôte Windows il voit trente-sept items déclarables ; sur la
//! CI Linux, aucun. Le même test y prouverait donc deux choses différentes, et
//! la seconde ne prouve rien du tout — c'est ainsi qu'une barrière devient une
//! étape verte de plus.
//!
//! Cette table est le relevé réel du **2026-08-15**, réduit à ce qui compte pour
//! l'import : les items déclarables, plus quelques constats et mesures dont
//! l'absence du fichier est justement ce qu'on éprouve. Les valeurs sont celles
//! que les collecteurs ont produites, jetons compris (ADR-0015). Le nom de la
//! machine, lui, est fictif : la configuration réelle d'un poste vit dans un
//! dépôt privé, jamais ici.
//!
//! ## Ce qu'elle porte, et pourquoi chaque famille y est
//!
//! * **trois valeurs illisibles** — les exclusions Defender, sous une clé
//!   protégée par ACL qu'une CLI non élevée ne lit pas (SEC-01). Elles ne se
//!   déclarent pas, et c'est le cas nominal du produit, pas un cas limite ;
//! * **cinq absences** — une clé de politique qui n'existe pas est un constat,
//!   et elle se déclare `{ absent: true }` ;
//! * **une liste vide** — les règles ASR locales. Elle doit se relire en liste
//!   vide, jamais en absence ;
//! * **des textes qui ressemblent à autre chose** — `time.windows.com,0x9`
//!   porte une virgule et un hexadécimal, et deux constats valent `26200` et
//!   `23410000`, que YAML relirait en entiers sans guillemets ;
//! * **des natures mêlées** — la nature ne se déduit pas du préfixe du chemin
//!   (ADR-0009), et la table l'écrit donc item par item.

use chrono::TimeZone as _;
use ks_core::{Domain, Item, ItemValue, Nature, Provenance};

/// Fabrique un item observé, tel qu'un collecteur le produit.
///
/// L'horodatage est **fixe** : une horloge dans un test rend son résultat
/// dépendant de l'instant où on le joue.
pub(crate) fn item(chemin: &str, domaine: Domain, nature: Nature, valeur: ItemValue) -> Item {
    Item {
        path: chemin.to_owned(),
        domain: domaine,
        nature,
        desired: None,
        observed: valeur,
        observed_at: chrono::Utc
            .with_ymd_and_hms(2026, 8, 15, 12, 0, 0)
            .single()
            .expect("date fixe valide"),
        // `Observed`, jamais `Keystone` : un collecteur lit, il ne produit pas
        // la valeur. Marquer un relevé `Keystone` rendrait `Unknown`
        // inatteignable, donc le signal de sécurité du modèle inopérant.
        provenance: Provenance::Observed,
        purpose: "Item de la machine de référence, figé pour les tests.".to_owned(),
        risk: "Aucun — lecture seule.".to_owned(),
        reference: None,
    }
}

/// Un texte, tel qu'un collecteur l'écrit — un jeton, une version, un chemin.
fn t(valeur: &str) -> ItemValue {
    ItemValue::Text(valeur.to_owned())
}

/// Une valeur illisible, telle qu'une clé protégée par ACL en produit une.
fn refuse() -> ItemValue {
    ItemValue::illisible("accès refusé sans élévation")
}

/// L'inventaire figé.
///
/// L'ordre est celui du scan, c'est-à-dire **pas** celui du fichier : c'est ce
/// qui permet de vérifier que l'émetteur trie et regroupe lui-même.
/// La table est écrite d'un bloc, et `cargo fmt` l'étale : un relevé se vérifie
/// ligne à ligne contre un scan réel, et le découper en morceaux rendrait cette
/// vérification impossible.
pub(crate) fn items() -> Vec<Item> {
    use Domain::{Inventory, Security, Space, Virtualization};
    use ItemValue::{Absent, Bool, Int, List};
    use Nature::{Constat, Mesure, Objectif, Reglage};

    let table: Vec<(&str, Domain, Nature, ItemValue)> = vec![
        // ── Ce qui ne se déclare jamais, et doit rester hors du fichier ──────
        ("inventory.os.name", Inventory, Constat, t("Windows 11 Pro")),
        // Relu sans guillemets, il reviendrait en entier. C'est un constat, donc
        // il n'entre pas dans le fichier — mais la famille est vivante.
        ("inventory.os.kernel", Inventory, Constat, t("26200")),
        (
            "inventory.host.name",
            Inventory,
            Constat,
            t("WKS-EXEMPLE-01"),
        ),
        ("inventory.cpu.cores", Inventory, Constat, Int(16)),
        ("inventory.uptime_seconds", Inventory, Mesure, Int(48_213)),
        ("space.volume[C:].used_percent", Space, Mesure, Int(61)),
        (
            "security.firmware.microcode_revision",
            Security,
            Constat,
            t("23410000"),
        ),
        // ── Posture de la plateforme ────────────────────────────────────────
        (
            "security.platform.secure_boot",
            Security,
            Objectif,
            Bool(true),
        ),
        (
            "security.platform.hvci_policy",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.platform.hvci_uefi_lock",
            Security,
            Reglage,
            Absent,
        ),
        ("security.platform.vbs_policy", Security, Reglage, Absent),
        (
            "security.platform.lsa_protection",
            Security,
            Reglage,
            t("active"),
        ),
        (
            "security.platform.lsa_protection_uefi_lock",
            Security,
            Objectif,
            Bool(false),
        ),
        (
            "security.platform.credential_guard",
            Security,
            Reglage,
            Absent,
        ),
        (
            "security.platform.vbs_running",
            Security,
            Objectif,
            t("en-execution"),
        ),
        (
            "security.platform.hvci_running",
            Security,
            Objectif,
            t("en-execution"),
        ),
        (
            "security.platform.credential_guard_running",
            Security,
            Objectif,
            t("arrete"),
        ),
        (
            "security.platform.code_integrity_enforcement",
            Security,
            Objectif,
            t("imposee"),
        ),
        (
            "security.platform.dma_protection_available",
            Security,
            Objectif,
            t("disponible-sur-ce-materiel"),
        ),
        // ── Defender ────────────────────────────────────────────────────────
        ("security.defender.realtime", Security, Reglage, Bool(true)),
        (
            "security.defender.tamper_protection",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.defender.behavior_monitoring",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.defender.exclusions.paths",
            Security,
            Reglage,
            refuse(),
        ),
        (
            "security.defender.exclusions.extensions",
            Security,
            Reglage,
            refuse(),
        ),
        (
            "security.defender.exclusions.processes",
            Security,
            Reglage,
            refuse(),
        ),
        (
            "security.defender.asr_rules.policy",
            Security,
            Reglage,
            Absent,
        ),
        (
            "security.defender.asr_rules.local",
            Security,
            Reglage,
            List(Vec::new()),
        ),
        // ── Pare-feu, horloge, services ─────────────────────────────────────
        (
            "security.firewall.domain.enabled",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.firewall.private.enabled",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.firewall.public.enabled",
            Security,
            Reglage,
            Bool(true),
        ),
        (
            "security.clock.ntp_server",
            Security,
            Reglage,
            t("time.windows.com,0x9"),
        ),
        (
            "security.clock.timezone",
            Security,
            Reglage,
            t("Romance Standard Time"),
        ),
        (
            "security.services.windefend.startup",
            Security,
            Reglage,
            t("automatique"),
        ),
        (
            "security.services.mpssvc.startup",
            Security,
            Reglage,
            t("automatique"),
        ),
        (
            "security.services.eventlog.startup",
            Security,
            Reglage,
            t("automatique"),
        ),
        ("security.services.sense.startup", Security, Reglage, Absent),
        (
            "security.services.wscsvc.startup",
            Security,
            Reglage,
            t("automatique"),
        ),
        (
            "security.services.bits.startup",
            Security,
            Reglage,
            t("automatique"),
        ),
        // ── Virtualisation ──────────────────────────────────────────────────
        (
            "virtualization.wsl[debian].version",
            Virtualization,
            Constat,
            Int(2),
        ),
        (
            "virtualization.wsl[debian].interop",
            Virtualization,
            Reglage,
            Bool(true),
        ),
        (
            "virtualization.wsl[debian].drive_mounting",
            Virtualization,
            Reglage,
            Bool(true),
        ),
        (
            "virtualization.wsl[dockerdesktop].interop",
            Virtualization,
            Reglage,
            Bool(true),
        ),
        (
            "virtualization.wsl[dockerdesktop].drive_mounting",
            Virtualization,
            Reglage,
            Bool(true),
        ),
        (
            "virtualization.wsl[kslab].interop",
            Virtualization,
            Reglage,
            Bool(true),
        ),
        (
            "virtualization.wsl[kslab].drive_mounting",
            Virtualization,
            Reglage,
            Bool(true),
        ),
    ];

    table
        .into_iter()
        .map(|(chemin, domaine, nature, valeur)| item(chemin, domaine, nature, valeur))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La machine de référence porte bien ce que les tests lui font dire.
    ///
    /// Sans ce garde, une modification de la table pourrait vider les autres
    /// tests de leur substance sans qu'aucun échoue : plus d'illisible, plus
    /// d'absence, et les barrières correspondantes deviennent vacantes.
    #[test]
    fn la_machine_de_reference_porte_les_familles_quon_lui_prete() {
        let items = items();
        let declarables = items.iter().filter(|i| i.nature.est_declarable()).count();
        let illisibles = items
            .iter()
            .filter(|i| i.nature.est_declarable() && !i.observed.est_constat())
            .count();

        assert_eq!(
            declarables, 37,
            "le relevé du 2026-08-15 en compte trente-sept"
        );
        assert_eq!(
            illisibles, 3,
            "les trois exclusions Defender, protégées par ACL"
        );
        assert!(
            items.iter().any(|i| i.observed == ItemValue::Absent),
            "aucune absence : la forme « {{ absent: true }} » ne serait plus éprouvée"
        );
        assert!(
            items
                .iter()
                .any(|i| matches!(&i.observed, ItemValue::List(v) if v.is_empty())),
            "aucune liste vide : elle se relirait en absence sans qu'on le voie"
        );
        assert!(
            items.iter().any(|i| !i.nature.est_declarable()),
            "aucun item non déclarable : le filtrage ne serait plus éprouvé"
        );

        // Deux chemins identiques feraient une clé en double dans le fichier,
        // donc un document que Keystone refuse de relire.
        let mut chemins: Vec<&str> = items.iter().map(|i| i.path.as_str()).collect();
        chemins.sort_unstable();
        let avant = chemins.len();
        chemins.dedup();
        assert_eq!(avant, chemins.len(), "deux items portent le même chemin");
    }
}

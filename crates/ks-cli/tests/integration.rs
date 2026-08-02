//! Tests d'intégration : la CLI lancée comme un **vrai processus**.
//!
//! ## Ce que les tests unitaires ne peuvent pas voir
//!
//! Toute la Phase 0 s'est testée en appelant des fonctions. Cela couvre la
//! logique, et laisse dehors ce qui fait qu'une CLI est utilisable depuis un
//! script :
//!
//! * le **code de sortie**, qui est un contrat — `0` succès, `1` erreur, `69`
//!   « pas encore implémenté ». Un binaire qui renverrait `0` sur une commande
//!   inerte rendrait le succès indiscernable de l'inaction ;
//! * la séparation **sortie standard / erreur standard** : un message d'erreur
//!   sur la sortie standard pollue un `--json` redirigé ;
//! * le fait que `--json` produise du JSON **réellement analysable**, et rien
//!   d'autre avant ni après.
//!
//! `CARGO_BIN_EXE_ks` est fourni par cargo à la compilation du test : il désigne
//! le binaire qui vient d'être construit, sans qu'on ait à deviner un chemin.

use std::process::{Command, Output};

/// Lance `ks` avec ces arguments.
fn ks(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ks"))
        .args(arguments)
        .output()
        .expect("le binaire ks doit être exécutable")
}

fn texte(flux: &[u8]) -> String {
    String::from_utf8_lossy(flux).into_owned()
}

#[test]
fn un_scan_reussit_et_ne_dit_rien_sur_la_sortie_derreur() {
    let sortie = ks(&["scan"]);
    assert!(sortie.status.success(), "un scan doit réussir");
    assert!(
        texte(&sortie.stderr).trim().is_empty(),
        "rien ne doit partir sur la sortie d'erreur : {}",
        texte(&sortie.stderr)
    );
    assert!(texte(&sortie.stdout).contains("lecture seule"));
}

#[test]
fn le_json_est_analysable_et_seul_sur_sa_sortie() {
    // La promesse « --json sur tout » ne vaut que si la sortie est du JSON pur.
    // Une seule ligne de bannière avant, et tout script qui la redirige casse.
    let sortie = ks(&["scan", "--json"]);
    assert!(sortie.status.success());

    let brut = texte(&sortie.stdout);
    let valeur: serde_json::Value =
        serde_json::from_str(&brut).expect("la sortie --json doit être du JSON, et rien d'autre");
    let items = valeur.as_array().expect("un scan rend un tableau d'items");
    assert!(!items.is_empty(), "un scan produit toujours quelque chose");

    // Et chaque item porte ce que le principe P6 exige : on ne peut pas
    // afficher ce qu'on ne sait pas expliquer.
    for item in items {
        for champ in [
            "path",
            "domain",
            "observed",
            "purpose",
            "risk",
            "provenance",
        ] {
            assert!(
                item.get(champ).is_some(),
                "l'item {item} n'a pas de champ « {champ} »"
            );
        }
    }
}

#[test]
fn un_domaine_inconnu_echoue_bruyamment() {
    // Le défaut d'origine : `--domain securty` retombait sur « aucun filtre » et
    // affichait TOUT en silence. L'utilisateur croyait avoir filtré.
    let sortie = ks(&["scan", "--domain", "securty"]);
    assert!(!sortie.status.success(), "une faute de frappe doit échouer");

    let erreur = texte(&sortie.stderr);
    assert!(erreur.contains("securty"), "le domaine fautif est cité");
    assert!(
        erreur.contains("security"),
        "et la liste des domaines acceptés est proposée : {erreur}"
    );
    assert!(
        texte(&sortie.stdout).trim().is_empty(),
        "une erreur ne s'écrit pas sur la sortie standard"
    );
}

#[test]
fn une_commande_inerte_ne_se_fait_pas_passer_pour_un_succes() {
    // Ni 0 (un script croirait la commande exécutée), ni 1 (réservé aux vraies
    // erreurs). 69 signifie « service indisponible » au sens de `sysexits.h`,
    // ce qui décrit exactement une commande déclarée mais pas implémentée.
    for commande in [
        vec!["diff"],
        vec!["converge"],
        vec!["rollback", "snap-1"],
        vec!["isolate"],
    ] {
        let sortie = ks(&commande);
        assert_eq!(
            sortie.status.code(),
            Some(69),
            "« {} » doit sortir en 69, pas en 0 ni en 1",
            commande.join(" ")
        );
    }
}

#[test]
fn le_journal_absent_se_dit_sans_echouer() {
    // Un journal qui n'existe pas n'est pas une erreur : c'est un état normal
    // tant que rien n'a été consigné. Le dire vaut mieux que fabriquer un
    // journal vide, qui ressemblerait à un journal effacé.
    let sortie = ks(&["journal"]);
    let rendu = texte(&sortie.stdout);
    // Selon la machine, un journal peut exister ou non : les deux issues sont
    // valides, et aucune ne doit produire un échec.
    assert!(
        sortie.status.success(),
        "lire le journal ne doit jamais échouer : {}",
        texte(&sortie.stderr)
    );
    assert!(
        rendu.contains("Journal") || rendu.contains("Aucun journal"),
        "la sortie doit être explicite : {rendu}"
    );
}

#[test]
fn le_sceau_refuse_plutot_que_de_pretendre() {
    // Sceller exige d'expédier l'empreinte vers une ancre externe (SEC-04,
    // Phase 3). Écrire l'empreinte à côté du journal ne protégerait de rien :
    // mieux vaut refuser en le disant que prétendre sceller.
    let sortie = ks(&["journal", "--seal"]);
    assert!(!sortie.status.success());
    assert!(
        texte(&sortie.stderr).contains("ancre externe"),
        "le refus doit expliquer pourquoi : {}",
        texte(&sortie.stderr)
    );
}

#[test]
fn explain_sur_un_item_inconnu_oriente_au_lieu_de_planter() {
    let sortie = ks(&["explain", "nimporte.quoi"]);
    assert!(sortie.status.success(), "ce n'est pas une erreur d'usage");
    assert!(
        texte(&sortie.stdout).contains("ks scan"),
        "le message doit dire où trouver la liste"
    );
}

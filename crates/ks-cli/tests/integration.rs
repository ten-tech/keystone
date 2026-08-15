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
            // `nature` fait partie du contrat de sortie : c'est elle qui dira
            // pourquoi un item figure — ou non — dans `workstation.yaml`
            // (ADR-0009). Un item sans elle n'est pas explicable.
            "nature",
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
    // `diff` a quitté cette liste : il compare désormais un fichier d'état
    // désiré à un scan, et son code de sortie dit si la comparaison a eu lieu.
    for commande in [
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

/// Un chemin de travail à soi, hors du dépôt.
///
/// Chaque test porte le sien : la suite s'exécute en parallèle dans le même
/// processus, et deux tests qui écriraient le même fichier se marcheraient
/// dessus sans le dire.
fn fichier_de_travail(nom: &str) -> std::path::PathBuf {
    let chemin = std::env::temp_dir().join(nom);
    let _ = std::fs::remove_file(&chemin);
    chemin
}

/// Le critère du moment M1, joué sur la machine qui exécute les tests.
///
/// `ks import` puis `ks diff`, sans rien toucher entre les deux : **aucun
/// écart**. C'est le seul test du lot qui mesure la vraie machine, et c'est
/// pour cela qu'il ne cite aucun chiffre absolu — sur un hôte Linux, aucun item
/// n'est déclarable, et l'invariant tient quand même.
#[test]
fn ce_que_limport_ecrit_ne_produit_aucun_ecart() {
    let fichier = fichier_de_travail("ks-import-m1.yaml");
    let chemin = fichier.display().to_string();

    let import = ks(&["import", "-o", &chemin]);
    assert!(
        import.status.success(),
        "l'import doit réussir : {}",
        texte(&import.stderr)
    );
    let annonce = texte(&import.stdout);
    assert!(
        annonce.contains("Rien n'a été modifié"),
        "le moment M1 se dit en toutes lettres : {annonce}"
    );
    assert!(
        fichier.exists(),
        "le fichier annoncé doit exister sur le disque"
    );

    // ADR-0017 : la commande git est proposée, jamais exécutée. Le test ne peut
    // pas prouver qu'aucun processus n'a démarré — la barrière de source s'en
    // charge — mais il exige que le service soit rendu.
    assert!(
        annonce.contains("git -C"),
        "la commande git n'est pas proposée : {annonce}"
    );
    assert!(
        annonce.contains("n'exécute pas git"),
        "le refus doit s'expliquer, pas se taire : {annonce}"
    );

    let diff = ks(&["diff", "--config", &chemin, "--json"]);
    assert!(
        diff.status.success(),
        "la comparaison doit avoir lieu : {}",
        texte(&diff.stderr)
    );
    let rendu: serde_json::Value =
        serde_json::from_str(&texte(&diff.stdout)).expect("la sortie --json doit être du JSON");

    let ecarts = rendu["deviations"]
        .as_array()
        .expect("la sortie porte la liste des écarts");
    assert!(
        ecarts.is_empty(),
        "adopter l'état lu puis le comparer à lui-même ne peut pas produire d'écart : {ecarts:?}"
    );
    assert_eq!(
        rendu["declared"], rendu["compliant"],
        "toute déclaration posée sur un item observé doit être conforme"
    );
    assert_eq!(
        rendu["declaredNotObserved"]
            .as_array()
            .map(Vec::len)
            .expect("la sortie porte la liste des chemins non observés"),
        0,
        "un chemin écrit depuis un scan existe dans ce scan"
    );

    let _ = std::fs::remove_file(&fichier);
}

/// Écraser un état désiré est irréversible : il faut le demander (principe P3).
#[test]
fn un_import_necrase_pas_un_fichier_existant_sans_force() {
    let fichier = fichier_de_travail("ks-import-ecrasement.yaml");
    let chemin = fichier.display().to_string();
    std::fs::write(&fichier, "ceci n'est pas un état désiré\n").expect("préparation");

    let refus = ks(&["import", "-o", &chemin]);
    assert!(!refus.status.success(), "un fichier existant a été écrasé");
    assert!(
        texte(&refus.stderr).contains("--force"),
        "le refus doit dire comment passer outre : {}",
        texte(&refus.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&fichier).expect("relecture"),
        "ceci n'est pas un état désiré\n",
        "le fichier de l'utilisateur a été touché malgré le refus"
    );

    let accepte = ks(&["import", "-o", &chemin, "--force"]);
    assert!(
        accepte.status.success(),
        "--force doit remplacer : {}",
        texte(&accepte.stderr)
    );
    assert!(std::fs::read_to_string(&fichier)
        .expect("relecture")
        .contains("apiVersion: keystone/v1"));

    let _ = std::fs::remove_file(&fichier);
}

/// Sans fichier, `ks diff` oriente au lieu de se plaindre.
#[test]
fn un_diff_sans_fichier_detat_desire_renvoie_vers_import() {
    let fichier = fichier_de_travail("ks-diff-sans-fichier.yaml");
    let sortie = ks(&["diff", "--config", &fichier.display().to_string()]);

    assert!(
        !sortie.status.success(),
        "comparer sans fichier n'est pas un succès"
    );
    let erreur = texte(&sortie.stderr);
    assert!(
        erreur.contains("ks import"),
        "le message doit dire par où commencer : {erreur}"
    );
    assert!(
        texte(&sortie.stdout).trim().is_empty(),
        "une erreur ne s'écrit pas sur la sortie standard"
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

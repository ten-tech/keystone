//! # ks-broker — le seul composant privilégié
//!
//! ## Lire ceci avant d'écrire une ligne ici
//!
//! Keystone administre tout, avec les droits les plus élevés de la machine.
//! **Compromis, il constitue l'outil d'attaque le plus efficace imaginable sur ce
//! poste.** Ce crate est cet outil. Sa surface est donc volontairement minuscule, et
//! toute contribution qui l'élargit passe par une ADR.
//!
//! ## Les invariants du broker
//!
//! | Réf. | Invariant |
//! |---|---|
//! | SEC-01 | L'interface et la CLI sont **non privilégiées**. Seul ce binaire est élevé. |
//! | SEC-02 | L'API expose des **verbes typés et énumérés**. Aucune primitive d'exécution libre — pas de `run_command`, pas de `eval`, pas de « juste pour le debug ». |
//! | SEC-03 | Tout passage par l'API est journalisé : appelant, verbe, paramètres, diff, résultat. |
//! | SEC-07 | Contrôle d'intégrité au démarrage. En cas d'échec : **démarrage en lecture seule** et alerte, jamais un démarrage normal. |
//! | SEC-08 | Windows Hello obligatoire sur les verbes à coût réel. |
//! | SEC-10 | Limitation de débit sur les verbes destructeurs, y compris demandés légitimement. |
//! | SEC-12 | **Aucun port réseau en écoute.** Transport : named pipe uniquement. |
//!
//! ## Le test qui doit rester vrai
//!
//! Un audit de l'API du broker ne doit révéler aucune primitive d'exécution
//! arbitraire (critère d'acceptation A11). Si tu ajoutes un verbe qui prend une
//! commande, un script, un chemin d'exécutable ou une chaîne de format en paramètre,
//! tu viens de casser le produit.
//!
//! ## État
//!
//! Phase 0 : **ce binaire ne fait rien et c'est correct.** La Phase 0 n'a pas besoin
//! du broker — les collecteurs sont en lecture seule et tournent sans privilège.
//! Le broker entre en jeu en Phase 2, avec les instantanés et la convergence.

#![cfg_attr(not(windows), allow(unused_imports))]

use anyhow::Result;

/// Verbes exposés par l'API du broker.
///
/// **Cette énumération est le contrat de sécurité du projet.** Elle est fermée :
/// un client ne peut demander que ce qui est listé ici, avec des paramètres typés.
/// Il n'existe aucun moyen de faire exécuter au broker quelque chose qui ne soit pas
/// une variante de cette énumération.
///
/// Chaque nouveau verbe doit répondre à quatre questions, documentées dans son ADR :
/// sait-il se simuler ? sait-il s'annuler ? exige-t-il une présence humaine ?
/// est-il idempotent ?
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "verb")]
#[non_exhaustive]
pub enum Verb {
    /// Lit l'inventaire. Aucun privilège requis, listé ici pour l'uniformité du journal.
    Scan { domain: Option<String> },

    /// Prend un instantané avant une opération.
    TakeSnapshot { kind: String, target: String },

    /// Revient à un instantané.
    RestoreSnapshot { snapshot_id: String },

    /// Change le type de démarrage d'un service. Simulable, annulable, idempotent.
    SetServiceStartup { service: String, startup: String },

    /// Écrit une valeur de registre. Simulable, annulable via export préalable.
    SetRegistryValue {
        hive: String,
        path: String,
        name: String,
        value: String,
    },

    /// Ajoute une exclusion Defender. **Ciblée et justifiée uniquement** (D11-02) :
    /// la raison et l'expiration sont des paramètres obligatoires, pas des options.
    AddDefenderExclusion {
        path: String,
        reason: String,
        expires: String,
    },

    /// Isolement d'urgence. Exige Windows Hello (SEC-08).
    Isolate,
    //
    // ─────────────────────────────────────────────────────────────────────────
    // CE QUI NE SERA JAMAIS AJOUTÉ ICI, et pourquoi :
    //
    //   RunCommand { cmd: String }          → SEC-02. C'est une porte dérobée.
    //   RunScript { path: String }          → idem, avec un détour.
    //   SetAnyRegistryKey { .. } sans ACL   → équivaut à RunCommand via IFEO.
    //   DisableDefender                     → n'existe pas comme verbe atomique :
    //                                         passe par la convergence, avec diff,
    //                                         instantané et Hello.
    //
    // Si un besoin semble exiger l'un de ces verbes, le besoin est mal formulé.
    // Ouvrir une ADR plutôt qu'un raccourci.
    // ─────────────────────────────────────────────────────────────────────────
}

/// Résultat de l'exécution d'un verbe.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VerbResult {
    /// Le verbe a-t-il seulement été simulé ?
    pub simulated: bool,
    /// Diff produit, lisible par un humain.
    pub diff: Option<String>,
    /// Instantané pris avant l'opération, s'il y en a eu un.
    pub snapshot_id: Option<String>,
}

fn main() -> Result<()> {
    #[cfg(not(windows))]
    eprintln!(
        "ks-broker ne s'exécute que sur Windows.\n\
         \n\
         Ce n'est pas une limitation temporaire : le broker manipule les services\n\
         Windows, le registre, BitLocker, Defender, le TPM et Hyper-V. Il n'a aucun\n\
         sens ailleurs. Voir docs/05-ENVIRONNEMENT-DE-DEV.md."
    );

    #[cfg(windows)]
    println!(
        "ks-broker — squelette de Phase 0.\n\
         \n\
         Le broker n'entre en jeu qu'en Phase 2 : la Phase 0 est en lecture seule\n\
         et ses collecteurs tournent sans privilège. Rien à démarrer ici pour\n\
         l'instant.\n\
         \n\
         Avant d'implémenter :\n\
           1. lire docs/04-MODELE-DE-MENACE.md en entier ;\n\
           2. lire docs/adr/0002-grpc-sur-named-pipe.md ;\n\
           3. ne jamais tester ce binaire sur la machine hôte —\n\
              la VM de labo existe pour ça (docs/06-VM-DE-LABO.md)."
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tous les noms de verbes, tels que **serde** les connaît.
    ///
    /// On désérialise un verbe inexistant et on lit la liste des variantes attendues
    /// dans le message d'erreur. Cette liste est **générée par le derive
    /// `Deserialize`** à partir de l'énumération : c'est la seule source de vérité
    /// qui suive automatiquement toute variante ajoutée, sans dépendance
    /// supplémentaire — et sur `ks-broker`, une crate de plus exigerait une ADR.
    ///
    /// C'est ce qui rend la barrière réellement exhaustive. Le `match` de
    /// `nom_du_verbe` force à *nommer* une nouvelle variante ; il ne force pas à
    /// l'échantillonner, et un bras trompeur (`RunCommand { .. } => "widget"`)
    /// passerait. La liste ci-dessous, elle, contiendra « run-command » quoi qu'il
    /// arrive.
    fn noms_connus_de_serde() -> Vec<String> {
        let err = serde_json::from_str::<Verb>(r#"{"verb":"__inexistant__"}"#)
            .expect_err("un verbe inexistant doit être refusé")
            .to_string();
        let liste = err
            .split("expected one of ")
            .nth(1)
            .expect("serde énumère les variantes attendues dans son message");
        // Les noms sont encadrés d'accents graves : on garde un élément sur deux.
        liste
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect()
    }

    /// Recense toute variante de [`Verb`], de façon **exhaustive**.
    ///
    /// Le `match` n'a pas de bras `_ =>`, donc **ajouter une variante à `Verb`
    /// casse la compilation** de ce fichier — et donc la CI — avant même qu'un test
    /// s'exécute. Le contributeur est obligé de venir ici, ce qui est exactement
    /// l'endroit où lire les quatre questions du modèle de menace.
    ///
    /// **Ce que ce `match` ne garantit pas**, et il faut le dire : il force à
    /// *nommer* la variante, pas à l'échantillonner ni à la nommer honnêtement. Un
    /// bras `Verb::RunCommand { .. } => "widget"` compile. C'est
    /// `noms_connus_de_serde` qui ferme ce trou, en dérivant la liste réelle de
    /// l'énumération elle-même.
    fn nom_du_verbe(v: &Verb) -> &'static str {
        match v {
            Verb::Scan { .. } => "scan",
            Verb::TakeSnapshot { .. } => "take-snapshot",
            Verb::RestoreSnapshot { .. } => "restore-snapshot",
            Verb::SetServiceStartup { .. } => "set-service-startup",
            Verb::SetRegistryValue { .. } => "set-registry-value",
            Verb::AddDefenderExclusion { .. } => "add-defender-exclusion",
            Verb::Isolate => "isolate",
        }
    }

    /// Ce test est une **barrière de conception**, pas une vérification de comportement.
    ///
    /// Il vérifie trois choses :
    ///
    /// 1. que tout verbe est recensé (via `nom_du_verbe`, exhaustif à la compilation) ;
    /// 2. qu'aucun **nom de verbe** n'évoque l'exécution de code ;
    /// 3. qu'aucun **champ** ne porte un nom qui l'évoque.
    ///
    /// Ce que ce test ne sait **pas** faire, et il faut le dire : il n'attrape pas un
    /// verbe dangereux dont les noms seraient anodins — `RunScript { path }` passerait
    /// le filtre, `path` étant légitimement employé par deux verbes existants. Aucun
    /// test grossier ne remplacera la revue et l'ADR ; son rôle est de rendre le
    /// raccourci évident bruyant, pas de rendre la revue superflue.
    #[test]
    fn aucun_verbe_ne_transporte_dexecution_arbitraire() {
        let interdits = [
            "cmd",
            "command",
            "script",
            "shell",
            "exec",
            "eval",
            "powershell",
            "run",
            "invoke",
            "spawn",
            "process",
        ];

        let echantillons = vec![
            Verb::Scan { domain: None },
            Verb::TakeSnapshot {
                kind: "hyperv-checkpoint".into(),
                target: "ks-lab".into(),
            },
            Verb::RestoreSnapshot {
                snapshot_id: "snap-1".into(),
            },
            Verb::SetServiceStartup {
                service: "Fax".into(),
                startup: "Disabled".into(),
            },
            Verb::SetRegistryValue {
                hive: "HKLM".into(),
                path: r"SOFTWARE\Policies".into(),
                name: "Example".into(),
                value: "1".into(),
            },
            Verb::AddDefenderExclusion {
                path: r"D:\src\target".into(),
                reason: "artefacts de build".into(),
                expires: "2027-01-01".into(),
            },
            Verb::Isolate,
        ];

        // 1. Le nom de CHAQUE variante, y compris non échantillonnée, passe le
        //    filtre. C'est le contrôle qui tient réellement : la liste vient de
        //    serde, donc de l'énumération elle-même.
        let mut connus = noms_connus_de_serde();
        connus.sort();
        for nom in &connus {
            for mot in nom.split('-') {
                assert!(
                    !interdits.contains(&mot),
                    "SEC-02 violé : le verbe « {nom} » est nommé comme une primitive \
                     d'exécution. Voir docs/04-MODELE-DE-MENACE.md § « verbes interdits »."
                );
            }
        }

        // 2. Complétude : tout verbe que serde connaît doit être échantillonné, et
        //    sous le même nom. Un bras trompeur du `match` (« widget » pour
        //    RunCommand) fait diverger les deux listes, donc échouer ici.
        let mut recenses: Vec<String> = echantillons
            .iter()
            .map(|v| nom_du_verbe(v).to_owned())
            .collect();
        recenses.sort();
        recenses.dedup();
        assert_eq!(
            recenses, connus,
            "la liste des verbes échantillonnés diverge de celle que serde dérive de \
             l'énumération. Un verbe a été ajouté sans être échantillonné, ou son bras \
             de `nom_du_verbe` ne porte pas son vrai nom. Corrige, puis ouvre l'ADR que \
             docs/08-CONVENTIONS.md exige pour tout nouveau verbe."
        );

        for verbe in &echantillons {
            let json = serde_json::to_string(verbe).expect("verbe sérialisable");

            // 2. Le nom du verbe lui-même. `rename_all = "kebab-case"` produit des
            //    tirets, que le filtre sur les clefs écarte : sans ce contrôle,
            //    « run-command » n'était jamais examiné.
            let nom = nom_du_verbe(verbe);
            for mot in nom.split('-') {
                assert!(
                    !interdits.contains(&mot),
                    "SEC-02 violé : le verbe « {nom} » est nommé comme une primitive \
                     d'exécution. Voir docs/04-MODELE-DE-MENACE.md § « verbes interdits »."
                );
            }

            // 3. Les noms de champs transportés.
            let clefs: Vec<&str> = json
                .split('"')
                .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
                .collect();
            for clef in clefs {
                assert!(
                    !interdits.contains(&clef),
                    "SEC-02 violé : le verbe {verbe:?} transporte un champ « {clef} ». \
                     Ouvre une ADR avant d'aller plus loin."
                );
            }
        }
    }

    #[test]
    fn une_exclusion_defender_exige_raison_et_expiration() {
        // Exigence D11-02 : pas de champ optionnel ici. Le typage impose la
        // justification — on ne peut pas construire l'appel sans elle.
        let v = Verb::AddDefenderExclusion {
            path: r"D:\src\target".into(),
            reason: "400k fichiers, +6 min par build".into(),
            expires: "2027-01-01".into(),
        };
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("reason"));
        assert!(json.contains("expires"));
    }
}

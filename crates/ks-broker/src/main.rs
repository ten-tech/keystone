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
// Pas de `#[non_exhaustive]`, et c'est une décision de sécurité, pas un oubli.
//
// Il n'apporte rien ici — aucun crate externe ne consomme ce type — et il
// désarmerait la barrière SEC-02 dès que `Verb` sortira de ce binaire (extraction
// d'un `ks-proto` pour la CLI et l'UI, ajout d'une cible `lib`, déplacement du
// test dans `tests/`). Hors du crate définisseur, `#[non_exhaustive]` rend un
// `match` sans bras `_` impossible : rustc suggère lui-même `_ => todo!()`, le
// contributeur suit la suggestion, et la garantie disparaît sans que personne
// ne le voie.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "verb")]
pub enum Verb {
    /// Lit l'inventaire. Aucun privilège requis, listé ici pour l'uniformité du journal.
    Scan { domain: Option<String> },

    /// Prend un instantané avant une opération.
    TakeSnapshot { kind: String, target: String },

    /// Revient à un instantané.
    RestoreSnapshot { snapshot_id: String },

    /// Change le type de démarrage d'un service **de la liste gérée**.
    ///
    /// Le nom du service n'est pas une chaîne : `SetServiceStartup { service:
    /// "WinDefend", startup: "disabled" }` aurait été `DisableDefender` écrit
    /// autrement, alors que le §7 exige que celui-ci passe par la convergence,
    /// avec diff, instantané et Windows Hello (ADR-0006).
    SetServiceStartup {
        service: ManagedService,
        startup: StartupType,
    },

    /// Applique un réglage **désigné**, jamais un chemin de registre.
    ///
    /// La variante précédente, `SetRegistryValue { hive, path, name, value }`,
    /// était le verbe que le §7 interdit nommément : « SetRegistryValue sans ACL
    /// → équivaut à RunCommand via IFEO ». Quatre chaînes libres suffisaient à
    /// faire exécuter du code — `…\Image File Execution Options\<exe>\Debugger`
    /// détourne tout lancement, `Services\<svc>\ImagePath` donne SYSTEM.
    ///
    /// Le client dit ce qu'il veut obtenir ; c'est le broker qui détient la
    /// correspondance vers la clé (ADR-0006).
    SetManagedSetting {
        setting: ManagedSetting,
        value: SettingValue,
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

/// Les services dont le broker sait changer le démarrage.
///
/// **Sans champ, et c'est la barrière.** Une variante unitaire ne peut pas
/// transporter un nom de service : l'ensemble des cibles atteignables est fini,
/// énuméré ici, et lisible par un relecteur en un écran. Un test le vérifie sur
/// le texte du source, parce qu'une garantie qu'on ne peut pas relire n'en est
/// pas une.
///
/// La liste ne contient que des services dont l'arrêt est un signal au sens du
/// §6 du modèle de menace. Il n'y a donc pas de cas « de confort » : chacun
/// exige une présence humaine (SEC-08).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedService {
    /// Antivirus Microsoft Defender.
    WindowsDefender,
    /// Pare-feu Windows Defender.
    WindowsFirewall,
    /// Journal des événements — sa perte efface la piste d'audit.
    EventLog,
}

/// Types de démarrage acceptés.
///
/// Volontairement plus pauvre que le registre, qui en encode cinq : le broker
/// n'a aucune raison de placer un service en démarrage noyau, et l'y autoriser
/// ouvrirait une capacité dont personne n'a besoin.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartupType {
    /// Démarre avec le système.
    Automatic,
    /// Démarre à la demande.
    Manual,
    /// Ne démarre pas.
    Disabled,
}

/// Les réglages que le broker sait écrire.
///
/// **Sans champ, même raison que [`ManagedService`].** Le client désigne un
/// réglage ; la correspondance vers la clé de registre vit dans le broker, où
/// elle se relit. Ajouter un réglage est un changement du broker, donc une
/// revue — c'est précisément le coût qu'on veut payer.
///
/// La liste est volontairement minimale en Phase 0. Elle se peuplera en Phase 2,
/// au fil des besoins réels de la convergence : l'ADR-0006 fige la **forme**,
/// pas le contenu.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedSetting {
    /// Protection en temps réel de Defender.
    DefenderRealtimeProtection,
    /// Pare-feu actif sur le profil public.
    FirewallPublicProfile,
}

/// Valeurs qu'un réglage géré peut prendre.
///
/// Un booléen suffit aujourd'hui, et le type le dit plutôt que d'accepter une
/// chaîne « pour plus tard ». Le jour où un réglage entier existera, ajouter une
/// variante sera un changement visible, pas un élargissement silencieux.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingValue {
    /// Activer.
    Enabled,
    /// Désactiver.
    Disabled,
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

    /// Le texte source de ce fichier, incorporé à la compilation.
    ///
    /// C'est la seule source de vérité qu'un attribut ne peut pas déplacer. Les
    /// contrôles fondés sur serde ont été mis en échec par injection réelle :
    /// `#[serde(rename = "apply-template")]` sur une variante et
    /// `#[serde(rename = "template")]` sur son champ font passer un
    /// `RunCommand { cmd: String }` complet — test vert, clippy propre, verbe
    /// pleinement invocable depuis un client. `#[serde(skip)]` le fait
    /// simplement disparaître de la liste dérivée.
    ///
    /// L'identifiant Rust, lui, est ce qu'un relecteur lit dans l'énumération.
    const SOURCE: &str = include_str!("main.rs");

    /// Extrait le corps d'un bloc délimité par des accolades, à partir d'une
    /// **déclaration en début de ligne**.
    ///
    /// « En début de ligne » n'est pas un détail : les appels à cette fonction
    /// contiennent eux-mêmes le texte recherché, en littéral. Un simple `find`
    /// tombait sur l'appel plutôt que sur la déclaration, et le test lisait alors
    /// son propre code. Exiger que l'ancre ouvre sa ligne écarte les littéraux.
    fn bloc_apres(source: &str, declaration: &str) -> String {
        let debut = source
            .match_indices(declaration)
            .map(|(i, _)| i)
            .find(|i| {
                let debut_ligne = source[..*i].rfind('\n').map_or(0, |n| n + 1);
                source[debut_ligne..*i].trim().is_empty()
            })
            .unwrap_or_else(|| panic!("déclaration « {declaration} » introuvable"));
        let reste = &source[debut..];
        let ouvrante = reste.find('{').expect("le bloc doit avoir une accolade");
        let mut profondeur = 0usize;
        for (i, c) in reste[ouvrante..].char_indices() {
            match c {
                '{' => profondeur += 1,
                '}' => {
                    profondeur -= 1;
                    if profondeur == 0 {
                        return reste[ouvrante + 1..ouvrante + i].to_owned();
                    }
                }
                _ => {}
            }
        }
        panic!("bloc « {declaration} » non refermé");
    }

    /// Les identifiants de variantes déclarés dans `enum Verb`, lus dans le source.
    fn variantes_declarees(bloc: &str) -> Vec<String> {
        bloc.lines()
            .map(str::trim)
            .filter(|l| !l.starts_with("//") && !l.starts_with('#'))
            .filter_map(|l| {
                let ident: String = l.chars().take_while(char::is_ascii_alphanumeric).collect();
                let suite = &l[ident.len()..];
                let est_variante = ident.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && (suite.starts_with(" {")
                        || suite.starts_with(',')
                        || suite.starts_with('('));
                est_variante.then_some(ident)
            })
            .collect()
    }

    /// Barrière SEC-02, niveau source — celle qui résiste aux attributs.
    ///
    /// Trois contrôles que ni `rename`, ni `skip`, ni un bras de `match` trompeur
    /// ne peuvent contourner, parce qu'ils portent sur le texte que le relecteur
    /// humain a sous les yeux.
    #[test]
    fn lenumeration_des_verbes_est_lisible_telle_quelle() {
        let bloc = bloc_apres(SOURCE, "pub enum Verb");

        // 1. Aucun renommage ni masquage sur les variantes ou leurs champs.
        //    Le `rename_all` global, lui, est sur la ligne du type, hors du bloc.
        assert!(
            !bloc.contains("#[serde("),
            "SEC-02 : un attribut serde dans le corps de `Verb` découple l'identifiant \
             Rust du nom transporté. C'est précisément par là qu'un verbe interdit \
             passe en se faisant appeler autrement. Retire-le."
        );

        // 2. `#[non_exhaustive]` désarmerait le `match` exhaustif dès que le type
        //    quitte ce crate.
        assert!(
            !SOURCE.contains("#[non_exhaustive]\npub enum Verb"),
            "SEC-02 : `#[non_exhaustive]` sur `Verb` autorise un bras `_` hors de ce \
             crate, donc supprime la barrière de compilation."
        );

        // 3. Chaque identifiant déclaré a son bras dans `nom_du_verbe`, sous son
        //    vrai nom. Un verbe ajouté ne peut donc être ni oublié, ni déguisé.
        let variantes = variantes_declarees(&bloc);
        assert!(
            variantes.len() >= 7,
            "extraction des variantes défaillante : {variantes:?}"
        );
        let arms = bloc_apres(SOURCE, "fn nom_du_verbe");
        for v in &variantes {
            assert!(
                arms.contains(&format!("Verb::{v}")),
                "SEC-02 : la variante « {v} » n'a pas de bras dans `nom_du_verbe`. \
                 Ajoute-le, échantillonne le verbe, et ouvre son ADR."
            );
            for mot in mots_de_pascal_case(v) {
                assert!(
                    !INTERDITS.contains(&mot.as_str()),
                    "SEC-02 violé : la variante « {v} » est nommée comme une primitive \
                     d'exécution. Voir docs/04-MODELE-DE-MENACE.md § « verbes interdits »."
                );
            }
        }
    }

    /// Les énumérations de paramètres n'ont **aucun champ**.
    ///
    /// C'est la barrière que l'ADR-0006 installe, et elle porte sur le texte du
    /// source parce qu'elle doit rester lisible par un relecteur humain.
    ///
    /// Le raisonnement : un verbe est sûr tant qu'il ne peut désigner qu'une
    /// cible d'un ensemble fini. Dès qu'une variante de paramètre porte un
    /// champ — `ManagedSetting::Custom { path: String }` — l'ensemble redevient
    /// infini, et le verbe redevient `SetRegistryValue` sans ACL, c'est-à-dire
    /// une exécution de code arbitraire par IFEO.
    ///
    /// La barrière SEC-02 voisine ne l'aurait pas vu : elle refuse les verbes
    /// *nommés* comme une primitive d'exécution, pas ceux qui en sont une sans
    /// le dire. Les deux sont nécessaires.
    #[test]
    fn les_parametres_de_verbe_ne_designent_quun_ensemble_fini() {
        for enumeration in ["pub enum ManagedService", "pub enum ManagedSetting"] {
            let bloc = bloc_apres(SOURCE, enumeration);
            for ligne in bloc.lines().map(str::trim) {
                if ligne.starts_with("//") || ligne.starts_with('#') || ligne.is_empty() {
                    continue;
                }
                assert!(
                    !ligne.contains('{') && !ligne.contains('('),
                    "SEC-02 / ADR-0006 : « {ligne} » porte un champ. Un paramètre de \
                     verbe qui transporte une donnée libre rend l'ensemble des cibles \
                     infini — le verbe redevient une écriture de registre arbitraire, \
                     donc une exécution de code par IFEO. Ajoute une variante unitaire, \
                     et la correspondance dans le broker."
                );
            }
        }
    }

    /// Aucun verbe ne reçoit en chaîne la **désignation d'un réglage système**.
    ///
    /// Complément du test précédent : celui-ci regarde les champs des variantes
    /// de `Verb`, l'autre les énumérations qu'elles référencent.
    /// `SetRegistryValue { hive, path, name, value }` a vécu ici toute la
    /// Phase 0 sans que rien ne bronche, alors que le §7 le nomme.
    ///
    /// # La frontière, et pourquoi ce n'est pas « aucun chemin »
    ///
    /// Ce test a d'abord interdit `path: String` partout, et il avait tort : il
    /// refusait `AddDefenderExclusion { path }`, dont le chemin est le **sujet**
    /// de l'opération, choisi par l'utilisateur parmi tous les chemins possibles.
    /// Le typer n'aurait aucun sens.
    ///
    /// Ce qui est interdit, c'est que le client **désigne un réglage du
    /// système** : une ruche, une clé, un service. Là, l'ensemble des cibles doit
    /// rester fini et détenu par le broker, sans quoi le verbe redevient une
    /// écriture de registre arbitraire, donc une exécution de code par IFEO.
    ///
    /// Une exclusion Defender ouvre un angle mort — grave, et encadré par D11-02
    /// qui impose raison et expiration — mais n'exécute rien. Les deux dangers
    /// sont réels et de nature différente ; les confondre aurait affaibli ce test
    /// au lieu de le renforcer.
    #[test]
    fn aucun_verbe_ne_recoit_la_designation_dun_reglage_systeme() {
        let bloc = bloc_apres(SOURCE, "pub enum Verb");
        for ligne in bloc.lines().map(str::trim) {
            if ligne.starts_with("//") {
                continue;
            }
            for interdit in [
                "hive: String",
                "key: String",
                "service: String",
                "setting: String",
                "registry_path: String",
                "value_name: String",
            ] {
                assert!(
                    !ligne.contains(interdit),
                    "SEC-02 / ADR-0006 : « {ligne} » laisse le client désigner un \
                     réglage du système. Le broker doit détenir la correspondance, \
                     pas la recevoir — sinon l'ensemble des cibles est infini."
                );
            }
        }
    }

    /// « RunCommand » → [« run », « command »].
    fn mots_de_pascal_case(ident: &str) -> Vec<String> {
        let mut mots = Vec::new();
        let mut courant = String::new();
        for c in ident.chars() {
            if c.is_ascii_uppercase() && !courant.is_empty() {
                mots.push(std::mem::take(&mut courant));
            }
            courant.push(c.to_ascii_lowercase());
        }
        if !courant.is_empty() {
            mots.push(courant);
        }
        mots
    }

    /// Les mots qui trahissent une primitive d'exécution.
    const INTERDITS: &[&str] = &[
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
            Verb::SetManagedSetting { .. } => "set-managed-setting",
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
        let interdits = INTERDITS;

        let echantillons = vec![
            Verb::Scan { domain: None },
            Verb::TakeSnapshot {
                kind: "hyperv-checkpoint".into(),
                target: "ks-lab".into(),
            },
            Verb::RestoreSnapshot {
                snapshot_id: "snap-1".into(),
            },
            // Les cibles ne sont plus des chaînes : `service` ne peut désigner
            // que l'un des trois services de `ManagedService`, et l'échantillon
            // ne peut donc plus inventer « Fax » ni quoi que ce soit d'autre.
            Verb::SetServiceStartup {
                service: ManagedService::WindowsDefender,
                startup: StartupType::Disabled,
            },
            Verb::SetManagedSetting {
                setting: ManagedSetting::FirewallPublicProfile,
                value: SettingValue::Enabled,
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

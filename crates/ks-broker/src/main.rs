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
    ///
    /// # Le comptage se fait sur du code, jamais sur du commentaire
    ///
    /// Une revue adverse a franchi cette barrière avec **une seule ligne de
    /// documentation** contenant une accolade fermante :
    ///
    /// ```text
    /// /// Le JSON attendu se termine par `"value": "1" }` — voir le protocole.
    /// ```
    ///
    /// Le compteur voyait cette accolade, refermait le bloc par anticipation, et
    /// tout ce qui suivait devenait invisible aux deux barrières textuelles. Un
    /// `SetTuning { hive, key, value }` déguisé par `#[serde(rename)]` passait
    /// alors au vert, clippy propre — c'est-à-dire le verbe que le §7 interdit,
    /// restauré sans que rien ne bronche.
    ///
    /// D'où deux protections cumulées, parce qu'une seule s'était déjà révélée
    /// insuffisante : les commentaires sont **retirés avant le comptage**, et la
    /// fin du bloc est en outre ancrée sur une accolade **seule sur sa ligne**,
    /// ce qui est la forme qu'impose `rustfmt` à toute fermeture de bloc réelle.
    fn bloc_apres(source: &str, declaration: &str) -> String {
        let epure = sans_commentaires(source);
        let debut = epure
            .match_indices(declaration)
            .map(|(i, _)| i)
            .find(|i| {
                let debut_ligne = epure[..*i].rfind('\n').map_or(0, |n| n + 1);
                epure[debut_ligne..*i].trim().is_empty()
            })
            .unwrap_or_else(|| panic!("déclaration « {declaration} » introuvable"));

        let reste = &epure[debut..];
        let ouvrante = reste.find('{').expect("le bloc doit avoir une accolade");
        let corps = &reste[ouvrante + 1..];

        // L'ancrage : la première ligne réduite à « } », donc la fermeture que
        // `rustfmt` produit. Une accolade en fin de ligne de code ne referme
        // jamais une déclaration d'énumération.
        let mut position = 0usize;
        for ligne in corps.split_inclusive('\n') {
            if ligne.trim_end() == "}" {
                return corps[..position].to_owned();
            }
            position += ligne.len();
        }
        panic!("bloc « {declaration} » non refermé");
    }

    /// Remplace le contenu des commentaires par des espaces, en gardant les
    /// positions et le découpage en lignes intacts.
    ///
    /// On ne les supprime pas, on les blanchit : cela préserve les décalages, donc
    /// la lisibilité des messages d'assertion qui citent la ligne fautive.
    fn sans_commentaires(source: &str) -> String {
        source
            .lines()
            .map(|ligne| match ligne.find("//") {
                Some(debut) => {
                    let mut propre = ligne[..debut].to_owned();
                    propre.push_str(&" ".repeat(ligne.len() - debut));
                    propre
                }
                None => ligne.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("\n")
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

    /// Champs `String` tolérés dans `Verb`, **chacun nommé et justifié ici**.
    ///
    /// La logique est inversée par rapport au premier jet, qui listait six noms
    /// de champs interdits. Une liste noire est un échantillon : une revue
    /// adverse l'a franchie en une ligne, avec
    /// `SetPosture { root, location, entry, data }` — quatre mots qu'aucun
    /// interdit ne couvrait, pour exactement la même écriture de registre
    /// arbitraire. `security.md` le dit d'ailleurs sans détour : liste blanche
    /// plutôt que liste noire.
    ///
    /// Ce qui suit est donc la liste **complète** des champs textuels admis. Tout
    /// autre champ de `Verb` doit porter le type d'une énumération fermée
    /// déclarée dans ce fichier.
    const CHAMPS_TEXTE_ADMIS: &[(&str, &str)] = &[
        (
            "domain",
            "filtre d'affichage d'un scan : ne désigne aucune écriture",
        ),
        (
            "path",
            "sujet d'une exclusion Defender, choisi par l'utilisateur parmi tous \
             les chemins possibles (D11-02). Le typer n'aurait aucun sens.",
        ),
        ("reason", "motif destiné à un humain, libre par nature"),
        (
            "expires",
            "DETTE, ADR-0006 : devrait être un horodatage typé. « hier » et « » \
             compilent aujourd'hui.",
        ),
        (
            "snapshot_id",
            "DETTE, ADR-0006 : identifiant à typer et à valider. Restaurer, c'est \
             appliquer en SYSTEM un contenu désigné par l'appelant.",
        ),
        (
            "kind",
            "DETTE, ADR-0006 : devrait être un genre d'instantané fermé.",
        ),
        (
            "target",
            "DETTE, ADR-0006 : cible d'instantané. « registry-export » sur \
             HKLM\\SAM fait extraire les empreintes de comptes par le broker.",
        ),
    ];

    /// Les couples (nom, type) des champs déclarés dans `enum Verb`.
    ///
    /// Lecture textuelle, sur un bloc dont les commentaires ont déjà été
    /// blanchis par [`bloc_apres`] — sans quoi une ligne de documentation
    /// contenant « : » fabriquerait un faux champ.
    fn champs_de_verbe(bloc: &str) -> Vec<(String, String)> {
        let plat = bloc.replace(['\n', '\r'], " ");
        let mut champs = Vec::new();
        for morceau in plat.split(',') {
            let Some((gauche, droite)) = morceau.split_once(':') else {
                continue;
            };
            let nom = gauche
                .rsplit(['{', ' '])
                .find(|s| !s.is_empty())
                .unwrap_or_default()
                .trim();
            let type_champ = droite
                .split(['}', ' '])
                .find(|s| !s.is_empty())
                .unwrap_or_default()
                .trim();
            if !nom.is_empty() && !type_champ.is_empty() {
                champs.push((nom.to_owned(), type_champ.to_owned()));
            }
        }
        champs
    }

    /// **Tout paramètre de verbe désigne un ensemble fini, ou est un texte admis.**
    ///
    /// C'est la barrière que l'ADR-0006 installe, et elle a été réécrite après
    /// qu'une revue adverse l'a franchie de trois façons. Ce qui a changé :
    ///
    /// * la liste des énumérations à contrôler n'est plus **écrite en dur** — elle
    ///   est **dérivée des types de champs de `Verb`**. `SettingValue` et
    ///   `StartupType` n'étaient pas dans la liste, et `SettingValue::Raw(String)`
    ///   passait donc au vert, rendant sa chaîne libre au verbe ;
    /// * les champs textuels ne sont plus filtrés par une liste noire de six
    ///   noms, mais par la liste blanche ci-dessus ;
    /// * une énumération de paramètre déplacée dans un autre fichier fait
    ///   **paniquer** l'extraction, avec un message explicite, plutôt que de
    ///   disparaître silencieusement du contrôle.
    ///
    /// Le raisonnement de fond n'a pas bougé : un verbe est sûr tant qu'il ne peut
    /// désigner qu'une cible d'un ensemble fini. Dès qu'un paramètre transporte
    /// une donnée libre, l'ensemble redevient infini et le verbe redevient
    /// `SetRegistryValue` sans ACL, c'est-à-dire une exécution de code par IFEO.
    #[test]
    fn tout_parametre_de_verbe_designe_un_ensemble_fini() {
        let champs = champs_de_verbe(&bloc_apres(SOURCE, "pub enum Verb"));
        assert!(
            champs.len() >= 8,
            "extraction des champs défaillante : {champs:?}"
        );

        for (nom, type_champ) in &champs {
            if type_champ.contains("String") {
                assert!(
                    CHAMPS_TEXTE_ADMIS.iter().any(|(admis, _)| admis == nom),
                    "SEC-02 / ADR-0006 : le champ « {nom}: {type_champ} » est un texte \
                     libre non répertorié. Soit il désigne une cible, et il faut une \
                     énumération fermée ; soit il est légitimement libre, et il faut \
                     l'inscrire dans CHAMPS_TEXTE_ADMIS avec sa justification — ce qui \
                     est précisément la revue qu'on veut provoquer."
                );
                continue;
            }

            // Le type doit être une énumération déclarée ICI. Si elle vit
            // ailleurs, l'extraction panique en le disant : c'est le seul
            // comportement acceptable, la barrière ne lisant qu'un fichier.
            let bloc_enum = bloc_apres(SOURCE, &format!("pub enum {type_champ}"));
            for ligne in bloc_enum.lines().map(str::trim) {
                if ligne.is_empty() || ligne.starts_with('#') {
                    continue;
                }
                assert!(
                    !ligne.contains('{') && !ligne.contains('('),
                    "SEC-02 / ADR-0006 : « {ligne} », dans l'énumération « {type_champ} » \
                     du champ « {nom} », porte une donnée. Une variante porteuse rend \
                     l'ensemble des cibles infini — le verbe redevient une écriture de \
                     registre arbitraire. Ajoute une variante unitaire, et la \
                     correspondance dans le broker."
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

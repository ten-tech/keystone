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
    ///
    /// Le sujet est **désigné**, jamais décrit : `kind: "registry-export"` avec
    /// `target: r"HKLM\SAM"` faisait extraire les empreintes de comptes par le
    /// broker lui-même (ADR-0006, dette payée).
    TakeSnapshot { subject: SnapshotSubject },

    /// Revient à un instantané.
    ///
    /// L'identifiant est validé à la construction et **jamais concaténé à un
    /// chemin** : restaurer, c'est appliquer en SYSTEM un contenu que l'appelant
    /// désigne.
    RestoreSnapshot { snapshot_id: SnapshotId },

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
        path: ExclusionPath,
        reason: String,
        expires: Expiry,
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

/// Ce dont on prend un instantané.
///
/// Remplace le couple `kind: String, target: String`, qui était la dette la plus
/// dangereuse de l'ADR-0006. `TakeSnapshot { kind: "registry-export", target:
/// r"HKLM\SAM" }` faisait écrire par le broker, **en SYSTEM**, la ruche des
/// comptes locaux dans un fichier : c'est `reg save HKLM\sam`, soit l'extraction
/// hors ligne des empreintes de mots de passe (ATT&CK T1003.002). Un appelant non
/// privilégié obtenait ainsi ce qu'il ne pouvait pas lire.
///
/// Sans champ, comme les autres énumérations de paramètres : le sujet désigne une
/// intention, et le broker détient la cible concrète.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSubject {
    /// Point de restauration système.
    SystemRestorePoint,
    /// Point de contrôle de la machine virtuelle de laboratoire.
    LabVirtualMachine,
    /// Export des distributions WSL gérées.
    WslDistributions,
    /// Export des branches de registre que Keystone sait écrire — celles, et
    /// **seulement celles**, que [`ManagedSetting`] référence.
    ManagedRegistryBranches,
}

/// Identifiant d'instantané, refusé s'il n'a pas la forme attendue.
///
/// `snapshot_id: String` était le second danger de l'ADR-0006, et le plus
/// insidieux : restaurer, c'est **appliquer en SYSTEM un contenu que l'appelant
/// désigne**. Un instantané contient légitimement des exports de registre et de
/// la configuration de service ; en faire pointer l'identifiant vers un
/// instantané fabriqué donne l'écriture de `Services\<svc>\ImagePath`, donc du
/// code SYSTEM au démarrage — ce qui contourne toutes les énumérations fermées
/// que l'ADR-0006 a installées.
///
/// D'où un jeu de caractères clos et une longueur bornée : ni séparateur de
/// chemin, ni `..`, ni caractère de contrôle. **Cette valeur n'est jamais
/// concaténée à un chemin** ; elle se résout par l'index du journal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct SnapshotId(String);

impl SnapshotId {
    /// Longueur maximale acceptée. Large pour un ULID (26) ou un UUID (36).
    const LONGUEUR_MAX: usize = 64;

    /// L'identifiant, tel qu'il a été validé.
    #[must_use]
    pub fn tel_quel(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SnapshotId {
    type Error = String;

    fn try_from(brut: String) -> Result<Self, Self::Error> {
        if brut.is_empty() || brut.len() > Self::LONGUEUR_MAX {
            return Err(format!(
                "un identifiant d'instantané fait de 1 à {} caractères",
                Self::LONGUEUR_MAX
            ));
        }
        // Liste blanche. Une liste noire de séparateurs laisserait passer les
        // formes exotiques — chemin UNC, flux alternatif, encodage pourcent.
        if !brut
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "un identifiant d'instantané ne contient que des lettres, des chiffres, \
                 « - » et « _ » : il ne doit jamais pouvoir désigner un chemin"
                    .to_owned(),
            );
        }
        Ok(Self(brut))
    }
}

/// Date d'expiration d'une dérogation, **future et bornée**.
///
/// `expires: String` acceptait « hier », « » et « 2099-01-01 » sans broncher.
/// D11-02 exige une expiration ; une chaîne ne garantissait que la présence d'un
/// champ, jamais sa validité — et l'architecture rangeait pourtant ce verbe parmi
/// les invariants « portés par le typage ».
///
/// L'horizon maximal n'est pas décoratif : une exclusion Defender sans plafond
/// est un angle mort permanent, c'est-à-dire exactement ce que D11-02 refuse.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct Expiry(String);

impl Expiry {
    /// Horizon maximal, en jours. Un an : au-delà, la dérogation se redemande.
    const HORIZON_JOURS: i64 = 366;

    /// La date, au format RFC 3339.
    #[must_use]
    pub fn tel_quel(&self) -> &str {
        &self.0
    }

    /// Valide contre un instant de référence.
    ///
    /// L'horloge est **injectée** plutôt que lue ici : un contrôle qui appelle
    /// `Utc::now()` ne se teste pas deux fois de la même façon.
    ///
    /// # Erreurs
    ///
    /// Renvoie le message destiné à l'utilisateur si la date est mal formée,
    /// déjà passée, ou au-delà de l'horizon.
    pub fn depuis(brut: &str, maintenant: chrono::DateTime<chrono::Utc>) -> Result<Self, String> {
        let date = chrono::DateTime::parse_from_rfc3339(brut)
            .map_err(|_| format!("« {brut} » n'est pas une date RFC 3339"))?
            .with_timezone(&chrono::Utc);

        if date <= maintenant {
            return Err(format!(
                "« {brut} » est déjà passée : une dérogation expirée à sa création \
                 n'est pas une dérogation"
            ));
        }
        if (date - maintenant).num_days() > Self::HORIZON_JOURS {
            return Err(format!(
                "« {brut} » dépasse l'horizon de {} jours : une dérogation sans \
                 plafond est un angle mort permanent (D11-02)",
                Self::HORIZON_JOURS
            ));
        }
        Ok(Self(brut.to_owned()))
    }
}

impl TryFrom<String> for Expiry {
    type Error = String;

    fn try_from(brut: String) -> Result<Self, Self::Error> {
        Self::depuis(&brut, chrono::Utc::now())
    }
}

/// Chemin d'une exclusion Defender, refusé s'il ouvre trop grand.
///
/// Le chemin reste libre — c'est le **sujet** de l'exclusion, choisi par
/// l'utilisateur parmi tous les chemins possibles, et le typer en énumération
/// n'aurait aucun sens. Mais l'argument opposé à `SetServiceStartup { service:
/// "WinDefend" }` s'applique mot pour mot : une exclusion de dossier couvre tous
/// ses sous-dossiers, les jokers sont acceptés, et les variables d'environnement
/// sont développées. `C:\` et `%SystemDrive%\*` sont donc `DisableDefender` sous
/// un autre nom, que le §7 refuse comme verbe atomique.
///
/// # Le détour qui casse P2, et que personne n'avait vu
///
/// Le service Defender tourne sous LocalSystem : `%TEMP%` s'y résout en
/// `C:\Windows\TEMP`, pas dans le profil de l'utilisateur. Une chaîne transmise
/// verbatim produirait donc un **diff qui ne désigne pas le dossier réellement
/// exclu** — une simulation qui ment, ce qui est pire qu'une absence de
/// simulation. D'où le refus des variables : le broker n'a pas à deviner dans
/// quel contexte elles seront développées.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct ExclusionPath(String);

impl ExclusionPath {
    /// Le chemin, tel qu'il a été validé.
    #[must_use]
    pub fn tel_quel(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExclusionPath {
    type Error = String;

    fn try_from(brut: String) -> Result<Self, Self::Error> {
        let normalise = brut.replace('/', "\\");

        if brut.contains('%') || brut.contains('$') {
            return Err(
                "un chemin d'exclusion ne contient pas de variable d'environnement : \
                 Defender tourne sous LocalSystem et la résoudrait ailleurs que vous, \
                 donc le diff montré ne désignerait pas le dossier réellement exclu"
                    .to_owned(),
            );
        }
        if brut.contains('*') || brut.contains('?') {
            return Err(
                "un chemin d'exclusion ne contient pas de joker : « C:\\*\\* » exclut \
                 tout le disque, ce qui est « désactiver Defender » sous un autre nom"
                    .to_owned(),
            );
        }
        if normalise.contains("..") {
            return Err("un chemin d'exclusion ne remonte pas l'arborescence".to_owned());
        }

        // Absolu, avec une lettre de lecteur. Un chemin relatif se résoudrait
        // contre un répertoire courant que le broker ne contrôle pas.
        let mut caracteres = normalise.chars();
        let lettre = caracteres.next().filter(char::is_ascii_alphabetic);
        if lettre.is_none() || !normalise[1..].starts_with(":\\") {
            return Err(
                "un chemin d'exclusion est absolu et commence par une lettre de lecteur".to_owned(),
            );
        }

        // Une racine de volume exclut tout le disque.
        let apres_racine = normalise[3..].trim_end_matches('\\');
        if apres_racine.is_empty() {
            return Err(
                "une racine de volume ne s'exclut pas : cela revient à éteindre \
                 Defender, ce que le §7 refuse comme verbe atomique"
                    .to_owned(),
            );
        }

        // Les répertoires système, dont l'exclusion est le geste d'attaque
        // documenté le plus courant.
        const INTERDITS: &[&str] = &["windows", "program files", "program files (x86)", "users"];
        let premier = apres_racine
            .split('\\')
            .next()
            .unwrap_or_default()
            .to_lowercase();
        if INTERDITS.contains(&premier.as_str()) && !apres_racine.contains('\\') {
            return Err(format!(
                "« {brut} » est un répertoire système entier : l'exclure ouvre un angle \
                 mort que rien ne referme"
            ));
        }

        Ok(Self(brut))
    }
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
        let epure = sans_commentaires_ni_chaines(source);
        let mut candidats = epure
            .match_indices(declaration)
            .map(|(i, _)| i)
            .filter(|i| {
                let debut_ligne = epure[..*i].rfind('\n').map_or(0, |n| n + 1);
                epure[debut_ligne..*i].trim().is_empty()
            });
        let debut = candidats
            .next()
            .unwrap_or_else(|| panic!("déclaration « {declaration} » introuvable"));

        // **L'unicité est une condition d'ancrage, pas une hygiène.**
        //
        // Sans elle, un leurre placé plus haut détourne toute la lecture — et il
        // n'a même pas besoin de compiler :
        //
        //     #[cfg(any())]
        //     mod protocole_v2 { pub enum Verb { … SetTuning, } }
        //
        // Le leurre est syntaxiquement valide, jamais compilé, et suffit à faire
        // lire une énumération inoffensive pendant que la vraie porte trois
        // chaînes libres. Tests verts, format propre. C'est la dixième attaque
        // de cette revue, et la première qui ne vise ni le lexeur ni la
        // délimitation : elle vise l'ancre elle-même.
        assert!(
            candidats.next().is_none(),
            "SEC-02 : « {declaration} » est déclarée plusieurs fois dans ce fichier. \
             La lecture textuelle ne sait pas laquelle est la vraie, donc elle ne \
             garantit plus rien — un leurre non compilé suffirait à la détourner."
        );

        // L'indentation de la déclaration est celle de sa fermeture : c'est la
        // règle que `rustfmt` applique sans exception. Anchorer sur « } » seul
        // ne valait que pour les éléments de premier niveau — appliqué à une
        // fonction imbriquée dans `mod tests`, l'analyse courait jusqu'à
        // l'accolade finale du module et rendait 141 lignes au lieu de dix.
        let debut_ligne = epure[..debut].rfind('\n').map_or(0, |n| n + 1);
        let fermeture = format!("{}}}", &epure[debut_ligne..debut]);

        let reste = &epure[debut..];
        let ouvrante = reste.find('{').expect("le bloc doit avoir une accolade");
        let corps = &reste[ouvrante + 1..];

        let mut position = 0usize;
        for ligne in corps.split_inclusive('\n') {
            if ligne.trim_end() == fermeture {
                return corps[..position].to_owned();
            }
            position += ligne.len();
        }
        panic!("bloc « {declaration} » non refermé");
    }

    /// Blanchit **tout ce que l'auteur du fichier peut écrire librement** :
    /// commentaires de ligne, commentaires de bloc, chaînes ordinaires et
    /// chaînes brutes. Les fins de ligne sont préservées, donc les numéros de
    /// ligne et les messages d'assertion restent justes.
    ///
    /// # Pourquoi cette fonction a coûté trois passes de revue
    ///
    /// Elle ne blanchissait d'abord rien, puis seulement les commentaires de
    /// ligne. À chaque fois la même attaque revenait par une région voisine, et
    /// la troisième est la plus instructive : un littéral fournissait **à la
    /// fois** l'accolade qui tronquait le bloc analysé **et** la fausse
    /// déclaration de variante qui rétablissait l'égalité avec la liste de
    /// serde.
    ///
    /// ```text
    /// #[doc = "
    /// SetTuning,
    /// }
    /// "]
    /// SetTuning { hive: String, key: String, value: String },
    /// ```
    ///
    /// Six tests verts, `rustfmt` conforme — et `SetRegistryValue` restauré,
    /// donc du code SYSTEM par IFEO. La leçon vaut plus que le correctif :
    /// **ancrer une lecture sur une source dérivée du type ne suffit pas si les
    /// deux listes comparées se fabriquent dans la même région que l'attaquant
    /// contrôle.** Il faut d'abord lui retirer cette région.
    ///
    /// Reste la limite de fond, qu'il faut dire : ceci est un analyseur écrit à
    /// la main, et il perdra un jour contre quelqu'un qui connaît mieux la
    /// grammaire. La sortie structurelle — analyser réellement, avec `syn` en
    /// dépendance de développement — mérite son ADR ; elle n'est pas gratuite,
    /// une dépendance de développement s'exécutant tout de même sur les postes
    /// et en intégration continue.
    fn sans_commentaires_ni_chaines(source: &str) -> String {
        let car: Vec<char> = source.chars().collect();
        let mut sortie = String::with_capacity(source.len());
        let mut i = 0;

        /// Efface `n` caractères en préservant les fins de ligne rencontrées.
        fn effacer(sortie: &mut String, car: &[char], i: &mut usize, n: usize) {
            for _ in 0..n {
                match car.get(*i) {
                    Some('\n') => sortie.push('\n'),
                    Some(_) => sortie.push(' '),
                    None => return,
                }
                *i += 1;
            }
        }

        while i < car.len() {
            let c = car[i];

            if c == '/' && car.get(i + 1) == Some(&'/') {
                let mut fin = i;
                while fin < car.len() && car[fin] != '\n' {
                    fin += 1;
                }
                let combien = fin - i;
                effacer(&mut sortie, &car, &mut i, combien);
                continue;
            }

            if c == '/' && car.get(i + 1) == Some(&'*') {
                effacer(&mut sortie, &car, &mut i, 2);
                let mut profondeur = 1usize;
                while i < car.len() && profondeur > 0 {
                    if car[i] == '/' && car.get(i + 1) == Some(&'*') {
                        profondeur += 1;
                        effacer(&mut sortie, &car, &mut i, 2);
                    } else if car[i] == '*' && car.get(i + 1) == Some(&'/') {
                        profondeur -= 1;
                        effacer(&mut sortie, &car, &mut i, 2);
                    } else {
                        effacer(&mut sortie, &car, &mut i, 1);
                    }
                }
                continue;
            }

            // Chaîne brute : `r"…"`, `r#"…"#`, et ainsi de suite.
            if c == 'r' {
                let mut j = i + 1;
                let mut diese = 0usize;
                while car.get(j) == Some(&'#') {
                    diese += 1;
                    j += 1;
                }
                if car.get(j) == Some(&'"') {
                    let combien = j - i + 1;
                    effacer(&mut sortie, &car, &mut i, combien);
                    while i < car.len() {
                        if car[i] == '"' {
                            let mut k = i + 1;
                            let mut vus = 0usize;
                            while vus < diese && car.get(k) == Some(&'#') {
                                vus += 1;
                                k += 1;
                            }
                            if vus == diese {
                                let combien = k - i;
                                effacer(&mut sortie, &car, &mut i, combien);
                                break;
                            }
                        }
                        effacer(&mut sortie, &car, &mut i, 1);
                    }
                    continue;
                }
            }

            if c == '"' {
                effacer(&mut sortie, &car, &mut i, 1);
                while i < car.len() {
                    if car[i] == '\\' {
                        effacer(&mut sortie, &car, &mut i, 2);
                        continue;
                    }
                    let ferme = car[i] == '"';
                    effacer(&mut sortie, &car, &mut i, 1);
                    if ferme {
                        break;
                    }
                }
                continue;
            }

            // Littéral de caractère — la cinquième forme, celle qui manquait.
            //
            // `'"'` ouvrait une chaîne fictive et **inversait la polarité** du
            // blanchiment : le code devenait chaîne et la chaîne devenait code.
            // Ce fichier en contient déjà cinq, dont un dans ce lexeur même.
            //
            // Une durée de vie commence par le même caractère et doit passer
            // intacte : on ne consomme que si la forme se referme, `'x'` ou
            // `'\n'`. `'static` ne se referme pas, donc reste du code.
            if c == '\'' {
                let echappe = car.get(i + 1) == Some(&'\\');
                let ferme = if echappe { i + 3 } else { i + 2 };
                if car.get(ferme) == Some(&'\'') {
                    let combien = ferme - i + 1;
                    effacer(&mut sortie, &car, &mut i, combien);
                    continue;
                }
            }

            sortie.push(c);
            i += 1;
        }
        sortie
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
        // Pas de seuil numérique ici : `la_lecture_textuelle_voit_tous_les_verbes`
        // exige déjà l'égalité avec la liste que serde dérive du type, ce qui est
        // strictement plus fort qu'un plancher. Le seuil précédent n'avait tenu
        // que par accident, et un accident n'est pas une défense.
        let variantes = variantes_declarees(&bloc);
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

    /// Champs `String` tolérés, **par couple (variante, champ)**.
    ///
    /// La clé porte la variante, et ce n'est pas un raffinement. Une liste
    /// indexée sur le seul nom de champ transforme une dette localisée en
    /// exemption générale : l'exception accordée à `AddDefenderExclusion.path`
    /// — un chemin qui est le *sujet* de l'opération — bénissait du même coup
    /// un futur `ApplyProfile { path: String }`, c'est-à-dire très exactement le
    /// `RunScript { path }` que ce fichier documente comme son angle mort.
    ///
    /// Une revue adverse l'a fait passer, vert et conforme à `rustfmt`. Ajouter
    /// un couple est désormais un geste visible, donc la revue qu'on veut.
    /// **Il n'en reste que deux, et c'était tout l'enjeu.**
    ///
    /// La liste en comptait sept, dont quatre marquées DETTE. Les quatre sont
    /// payées : `kind` et `target` sont devenus un [`SnapshotSubject`] fermé,
    /// `snapshot_id` un [`SnapshotId`] validé, `expires` un [`Expiry`] borné.
    ///
    /// Ce qui change n'est pas seulement le compte. Une liste d'exemptions qu'on
    /// relit en deux lignes est une liste qu'on relit ; à sept, on la parcourt.
    /// Et surtout, un `String` nu dans `Verb` est désormais une **anomalie
    /// visuelle** au milieu de types dédiés, repérée à l'œil avant tout test.
    /// Une barrière alignée sur la pente naturelle du code survit ; une barrière
    /// qui la remonte s'érode.
    const CHAMPS_TEXTE_ADMIS: &[(&str, &str, &str)] = &[
        (
            "Scan",
            "domain",
            "filtre d'affichage : ne désigne aucune écriture",
        ),
        (
            "AddDefenderExclusion",
            "reason",
            "motif destiné à un humain, libre par nature. C'est le seul champ du \
             broker qui n'a aucune raison d'être contraint : il ne désigne rien.",
        ),
    ];

    /// Types validés **à la construction**, admis comme paramètres de verbe.
    ///
    /// Un paramètre qui n'est ni une énumération fermée ni un texte admis doit
    /// figurer ici, et le test vérifie trois choses sur chacun : qu'il est un
    /// newtype, que son champ interne est **privé** — sans quoi la validation se
    /// contourne par construction directe — et qu'il porte `serde(try_from)`,
    /// sans quoi la désérialisation l'ignorerait complètement.
    ///
    /// C'est la différence de nature avec les deux autres catégories : une
    /// énumération rend l'état illégal **inconstructible**, un type validé le
    /// rend **refusé à la porte**. Le second est plus faible et suffit quand
    /// l'ensemble des valeurs légitimes est infini — un chemin, une date.
    ///
    /// # Indexée par couple, pour la raison écrite juste au-dessus
    ///
    /// Cette liste était indexée par **type seul**, quand sa voisine
    /// [`Self::CHAMPS_TEXTE_ADMIS`] l'est par couple `(variante, champ)` et
    /// explique pourquoi : « une liste indexée sur le seul nom de champ
    /// transforme une dette localisée en exemption générale ». Le raisonnement
    /// valait mot pour mot ici, et il n'y avait pas été appliqué : l'exemption
    /// accordée au chemin d'exclusion Defender bénissait **tout futur verbe**
    /// acceptant un `ExclusionPath`.
    ///
    /// **Mesuré, pas supposé.** `ApplyProfile { path: ExclusionPath }` — le
    /// `RunScript { path }` de l'angle mort, sous un autre nom — passait les
    /// treize tests, `fmt` et `clippy` compris, une fois faits les trois gestes
    /// mécaniques que les barrières exigent : la variante, le bras de
    /// [`nom_du_verbe`], l'échantillon. Aucun de ces trois gestes n'oblige à
    /// penser à ce que le verbe *fait*.
    ///
    /// Une revue adverse avait déjà fait ce constat pour la liste voisine, et le
    /// correctif n'avait couvert qu'une moitié du domaine. Une barrière peut
    /// appliquer un raisonnement juste à une moitié de son domaine et l'oublier
    /// sur l'autre ; c'est ce qui est corrigé ici.
    const TYPES_VALIDES_ADMIS: &[(&str, &str, &str, &str)] = &[
        (
            "RestoreSnapshot",
            "snapshot_id",
            "SnapshotId",
            "jeu de caractères clos et longueur bornée : ne peut désigner aucun \
             chemin, et n'est jamais concaténé à un",
        ),
        (
            "AddDefenderExclusion",
            "expires",
            "Expiry",
            "date RFC 3339, future, et bornée à un horizon d'un an : une \
             dérogation sans plafond est un angle mort permanent (D11-02)",
        ),
        (
            "AddDefenderExclusion",
            "path",
            "ExclusionPath",
            "absolu, sans joker, sans variable d'environnement, ni racine de \
             volume ni répertoire système entier. Le chemin est ici le SUJET de \
             l'opération — ce que Defender doit cesser d'inspecter — et non un \
             ordre à exécuter. Un chemin qui désignerait quoi faire, et non sur \
             quoi le faire, n'a rien à voir avec cette exemption.",
        ),
    ];

    /// « SetManagedSetting » → « set-managed-setting ».
    ///
    /// Doit reproduire **exactement** la règle de serde, puisque l'égalité du
    /// test en dépend : `serde_derive/src/internals/case.rs` insère un séparateur
    /// avant chaque majuscule sauf la première, puis remplace `_` par `-`. Donc
    /// `SetACL` donne « set-a-c-l » des deux côtés, et `TakeV2Snapshot`
    /// « take-v2-snapshot ». Si serde changeait cette règle, le test casserait
    /// sur du code légitime — et personne ne saurait pourquoi sans ce commentaire.
    fn en_kebab(ident: &str) -> String {
        mots_de_pascal_case(ident).join("-")
    }

    /// Les triplets (variante, champ, type) déclarés dans `enum Verb`.
    fn champs_par_variante(bloc: &str) -> Vec<(String, String, String)> {
        let mut variante = String::new();
        let mut champs = Vec::new();

        for ligne in bloc.lines() {
            let texte = ligne.trim();
            if texte.is_empty() || texte.starts_with('#') {
                continue;
            }

            let ident: String = texte
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            let suite = &texte[ident.len()..];
            if ident.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && (suite.starts_with(" {") || suite.starts_with(',') || suite.starts_with('('))
            {
                variante.clone_from(&ident);
            }

            for morceau in texte.split(',') {
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
                if nom.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && !type_champ.is_empty()
                {
                    champs.push((variante.clone(), nom.to_owned(), type_champ.to_owned()));
                }
            }
        }
        champs
    }

    /// **La lecture textuelle voit exactement ce que serde voit.**
    ///
    /// C'est l'ancrage qui ferme une famille entière d'attaques au lieu de ses
    /// membres, et il a fallu trois passes de revue adverse pour y arriver.
    ///
    /// Toutes les barrières précédentes lisaient le source avec `str::split`.
    /// Chaque correctif fermait une ruse et en laissait une autre du même
    /// genre : d'abord une accolade fermante dans un commentaire de ligne, puis
    /// la même dans un **littéral de chaîne** d'un attribut `#[doc = "…"]` —
    /// que `rustfmt` ne réindente pas, contrairement au commentaire de bloc.
    /// Un analyseur écrit en découpage de chaînes perdra toujours contre
    /// quelqu'un qui connaît la grammaire.
    ///
    /// D'où cet ancrage sur une source que le texte **ne contrôle pas** : la
    /// liste que le derive `Deserialize` produit à partir du type. Toute ruse de
    /// délimitation tronque la lecture textuelle et fait diverger les deux
    /// listes — sans qu'il ait fallu prévoir la ruse. Elle remplace au passage
    /// les deux seuils numériques, qui n'avaient sauvé la mise que par accident.
    #[test]
    fn la_lecture_textuelle_voit_tous_les_verbes() {
        use std::collections::BTreeSet;

        let lus: BTreeSet<String> = variantes_declarees(&bloc_apres(SOURCE, "pub enum Verb"))
            .iter()
            .map(|v| en_kebab(v))
            .collect();
        let vus_par_serde: BTreeSet<String> = noms_connus_de_serde().into_iter().collect();

        assert_eq!(
            lus, vus_par_serde,
            "SEC-02 : la lecture du source ne voit pas les mêmes verbes que serde. \
             Soit une variante est masquée par un renommage, soit le bloc analysé a \
             été tronqué — typiquement par une accolade fermante glissée dans un \
             commentaire ou dans un littéral de chaîne. Dans les deux cas, les \
             barrières qui suivent lisent un source incomplet et ne garantissent \
             plus rien."
        );
    }

    /// Les seules clefs d'attribut `serde` tolérées dans ce fichier.
    ///
    /// Liste blanche, et volontairement minuscule. Tout le reste — `skip`,
    /// `rename`, `alias`, `flatten`, `other`, `from` — **règle une projection**,
    /// c'est-à-dire découple ce que le client peut envoyer de ce que le source
    /// montre, et parfois de ce que le journal enregistre.
    ///
    /// `rename_all` et `tag` décrivent la forme du protocole, rien d'autre.
    ///
    /// # Pourquoi `try_from` est ici, et pourquoi ce n'est pas un assouplissement
    ///
    /// C'est la **seule clef de serde qui ajoute un contrôle au lieu d'en
    /// retirer un**. `skip_serializing` retire un champ de la sortie ;
    /// `try_from` impose que toute valeur reçue passe par un validateur avant
    /// d'exister. Le sens est exactement inverse.
    ///
    /// Sans elle, les types validés seraient une fiction : le client parle au
    /// broker par le réseau, pas par le constructeur, et serde construirait
    /// `SnapshotId` sans jamais appeler `TryFrom`. Le test
    /// `un_type_valide_ne_se_contourne_pas` **exige** d'ailleurs sa présence sur
    /// chacun des trois types — elle n'est donc pas seulement tolérée, elle est
    /// obligatoire là où elle sert.
    const CLEFS_SERDE_ADMISES: &[&str] = &["rename_all", "tag", "try_from"];

    /// **Personne ne règle la projection.**
    ///
    /// C'est la dernière marche d'une escalade de six revues, et elle porte sur
    /// une nature différente des précédentes. Les barrières d'avant ancraient la
    /// lecture sur une projection du type : les noms que serde dérive, puis les
    /// champs qu'il sérialise. Or **chaque projection est elle-même réglable par
    /// attribut**, donc chaque ancrage tombait d'un cran plus bas.
    ///
    /// `#[serde(skip_serializing)]` retire un champ du JSON sans rien retirer à
    /// la désérialisation : le client peut toujours l'envoyer. Combiné à un
    /// leurre, l'échantillon sérialise en `{"verb":"set-tuning"}`, les deux
    /// ensembles comparés sont vides, et l'égalité tient. Huit tests verts.
    ///
    /// # Et le contournement cachait pire que lui-même
    ///
    /// Un paramètre marqué `skip_serializing` est un paramètre que **le journal
    /// ne peut pas enregistrer**. SEC-03 exige « appelant, verbe, paramètres,
    /// diff, résultat » ; toute implémentation du journal fondée sur serde
    /// l'omettra en silence. Un contributeur qui ajoute `skip_serializing_if`
    /// pour ne pas journaliser une chaîne vide retire ce paramètre de la piste
    /// d'audit **sans aucun leurre et sans aucune malveillance**.
    ///
    /// Ce contrôle vaut donc autant pour SEC-03 que pour SEC-02.
    ///
    /// Il cherche `serde(` et non `#[serde(`, pour attraper l'enveloppe
    /// `cfg_attr(all(), serde(rename = "…"))` — que le contrôle voisin laissait
    /// passer, ne cherchant que la forme littérale. Et il porte sur le source
    /// **entier**, blanchi : il n'essaie pas d'identifier le bon bloc, donc aucun
    /// leurre ne le détourne.
    #[test]
    fn aucun_attribut_serde_ne_regle_la_projection() {
        let epure = sans_commentaires_ni_chaines(SOURCE);
        let mut examinees = 0usize;

        // L'aiguille est « serde » **puis** une parenthèse éventuellement
        // précédée d'espaces, et non le littéral « serde( ».
        //
        // La grammaire des attributs tolère `#[serde (skip_serializing)]`, que
        // la forme littérale ne voyait pas. Ce n'était pas exploitable — rustfmt
        // normalise l'espace et `cargo fmt --all --check` est bloquant à chaque
        // poussée — mais la défense était alors **externe et fortuite**, exactement
        // comme rustfmt l'avait été pour le commentaire de bloc. Une barrière
        // sauvée par un outil voisin cesse de protéger le jour où l'on assouplit
        // cet outil, et plus personne ne relie la brèche à sa cause.
        for (position, _) in epure.match_indices("serde") {
            let suite = epure[position + "serde".len()..].trim_start();
            let Some(arguments) = suite.strip_prefix('(') else {
                // `serde::Serialize`, `serde_json`, une mention en prose : rien
                // à contrôler, on passe.
                continue;
            };
            let fin = arguments
                .find(')')
                .expect("un attribut serde doit se refermer");
            let apres = arguments;

            for clef in apres[..fin].split(',') {
                let clef = clef.split('=').next().unwrap_or_default().trim();
                if clef.is_empty() {
                    continue;
                }
                examinees += 1;
                assert!(
                    CLEFS_SERDE_ADMISES.contains(&clef),
                    "SEC-02 / SEC-03 : l'attribut « serde({clef}) » règle une projection. \
                     Il découple ce que le client peut envoyer de ce que le source montre \
                     — et, s'il porte sur un paramètre, de ce que le journal enregistre. \
                     Si le besoin est réel, il passe par une ADR."
                );
            }
        }

        // Garde-fou de non-vacuité : un contrôle qui ne parcourt rien est vert
        // pour la pire des raisons. Six clefs sont attendues aujourd'hui — deux
        // sur `Verb`, une sur chacune des quatre énumérations de paramètres.
        assert!(
            examinees >= 6,
            "le contrôle n'a examiné que {examinees} clef(s) : l'extraction est cassée"
        );
    }

    /// **Le source lu porte les mêmes champs que le type compilé.**
    ///
    /// L'ancrage descend ici au niveau du **champ**, et c'est le dernier étage
    /// d'une escalade de cinq revues. Chaque étage précédent a été franchi :
    ///
    /// 1. la délimitation — accolade dans un commentaire, puis dans un littéral ;
    /// 2. l'ancre — un leurre `#[cfg(any())]` déclaré plus haut ;
    /// 3. l'unicité de l'ancre — qu'on croyait suffisante.
    ///
    /// La onzième attaque a montré qu'elle ne l'était pas. Un `pub use crate::
    /// Commande as Verb;` laisse le leurre être le **seul** `pub enum Verb` du
    /// fichier : l'unicité est satisfaite, l'égalité des noms aussi puisque serde
    /// interroge l'alias, et les champs lus sont ceux du leurre. Une barrière qui
    /// désigne son sujet par un nom écrit dans le texte peut toujours se faire
    /// présenter un autre sujet.
    ///
    /// D'où ce contrôle, qui ne fait plus confiance au texte pour dire *quels
    /// champs existent*. Les échantillons sont construits contre le **type**, pas
    /// contre le source : leur sérialisation dit la vérité gratuitement. Si les
    /// deux divergent, la lecture textuelle porte sur autre chose que
    /// l'énumération réellement exposée, et tout ce qui s'appuie dessus est nul.
    #[test]
    fn le_source_lu_porte_les_champs_du_type_compile() {
        use std::collections::BTreeSet;

        let par_variante = champs_par_variante(&bloc_apres(SOURCE, "pub enum Verb"));

        for verbe in echantillons() {
            let nom = nom_du_verbe(&verbe);
            let valeur: serde_json::Value =
                serde_json::to_value(&verbe).expect("un verbe est sérialisable");
            let objet = valeur.as_object().expect("un verbe sérialise en objet");

            // `verb` est la étiquette de la variante, pas un champ.
            let reels: BTreeSet<&str> = objet
                .keys()
                .map(String::as_str)
                .filter(|c| *c != "verb")
                .collect();

            let lus: BTreeSet<&str> = par_variante
                .iter()
                .filter(|(v, _, _)| en_kebab(v) == nom)
                .map(|(_, champ, _)| champ.as_str())
                .collect();

            assert_eq!(
                lus, reels,
                "SEC-02 / ADR-0006 : pour « {nom} », le source lu ne porte pas les \
                 mêmes champs que le type compilé. La lecture textuelle porte donc \
                 sur autre chose que l'énumération réellement exposée — typiquement \
                 un leurre, éventuellement rendu unique par un alias. Toutes les \
                 barrières textuelles sont nulles tant que cet écart existe."
            );
        }
    }

    /// Les types de [`TYPES_VALIDES_ADMIS`], sans doublon et dans l'ordre.
    ///
    /// La liste est indexée par couple `(variante, champ)` : un même type y
    /// figure donc plusieurs fois dès que deux verbes l'emploient. Les contrôles
    /// de **forme** — newtype, champ privé, `serde(try_from)` — portent sur le
    /// type, pas sur le couple, et n'ont aucune raison de se répéter.
    fn types_valides_distincts() -> Vec<&'static str> {
        let mut types: Vec<&'static str> =
            TYPES_VALIDES_ADMIS.iter().map(|(_, _, t, _)| *t).collect();
        types.sort_unstable();
        types.dedup();
        types
    }

    /// **Un type validé ne se contourne ni par construction ni par le réseau.**
    ///
    /// « Type validé » serait une affirmation creuse sans ces trois contrôles.
    /// Chacun ferme une porte réelle :
    ///
    /// 1. **Newtype** — un type à plusieurs champs se construirait champ par
    ///    champ, sans passer par le validateur.
    /// 2. **Champ interne privé** — `pub struct SnapshotId(pub String)` laisse
    ///    fabriquer n'importe quelle valeur directement, et la validation
    ///    devient décorative.
    /// 3. **`serde(try_from)`** — sans lui, la désérialisation construit le type
    ///    sans jamais appeler le validateur. C'est la porte qui compte le plus :
    ///    le client parle au broker **par le réseau**, pas par le constructeur.
    #[test]
    fn un_type_valide_ne_se_contourne_pas() {
        let epure = sans_commentaires_ni_chaines(SOURCE);

        // Le même type peut légitimement figurer sous plusieurs couples ; on
        // vérifie sa forme une fois, ce qui rend le message d'échec lisible
        // plutôt que répété.
        for nom in types_valides_distincts() {
            let declaration = format!("pub struct {nom}(");
            assert!(
                epure.contains(&declaration),
                "SEC-02 / ADR-0006 : « {nom} » est admis comme type validé mais n'est \
                 pas un newtype. Un type à plusieurs champs se construit champ par \
                 champ, donc sans passer par le validateur."
            );

            let position = epure
                .find(&declaration)
                .expect("la déclaration vient d'être trouvée");
            let apres = &epure[position + declaration.len()..];
            let interieur = apres
                .find(')')
                .map(|fin| &apres[..fin])
                .expect("un newtype se referme");
            assert!(
                !interieur.contains("pub"),
                "SEC-02 / ADR-0006 : le champ interne de « {nom} » est public. \
                 N'importe qui peut alors fabriquer la valeur sans validation, et \
                 le type ne garantit plus rien."
            );

            // La porte du réseau. Le client ne construit jamais le type : il
            // envoie du JSON, et serde le désérialise.
            let attendu = format!("pub struct {nom}");
            let debut_bloc = epure.find(&attendu).expect("déclaration présente");
            let entete = &epure[debut_bloc.saturating_sub(400)..debut_bloc];
            assert!(
                entete.contains("try_from"),
                "SEC-02 / ADR-0006 : « {nom} » ne porte pas `serde(try_from)`. La \
                 désérialisation le construirait donc sans appeler le validateur — \
                 et c'est par là que le client parle au broker."
            );
        }
    }

    /// Aucune variante ne cache sa charge utile dans un type enveloppé.
    ///
    /// `SetTuning(Tuning)` n'a aucun `nom: Type`, donc échappait à l'inspection
    /// des champs — et se désérialisait pourtant parfaitement depuis un client,
    /// vérifié par aller-retour. Seule `serde` interdisait les variantes *tuple*
    /// à plusieurs éléments sous `tag = "verb"` ; les *newtype* passaient.
    #[test]
    fn aucune_variante_nenveloppe_sa_charge_utile() {
        for ligne in bloc_apres(SOURCE, "pub enum Verb").lines().map(str::trim) {
            let ident: String = ligne
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            if !ident.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                continue;
            }
            assert!(
                !ligne[ident.len()..].starts_with('('),
                "SEC-02 / ADR-0006 : « {ligne} » enveloppe sa charge utile dans un \
                 type, qui échappe à l'inspection des champs. Déclare les champs \
                 nommément dans la variante."
            );
        }
    }

    /// **Tout paramètre de verbe désigne un ensemble fini, ou est un texte admis.**
    ///
    /// La liste des énumérations contrôlées est **dérivée des types de champs de
    /// `Verb`**, jamais écrite à la main : c'est ce qui la fait suivre l'ajout
    /// qu'on n'a pas anticipé. Un premier jet la nommait en dur, et
    /// `SettingValue::Raw(String)` passait donc au vert, rendant sa chaîne libre
    /// au verbe par la porte de service.
    ///
    /// Effet secondaire précieux : un alias de type ne trompe pas ce test, parce
    /// qu'il n'y cherche pas le mot « String » — il **exige** que tout type non
    /// textuel se résolve en `pub enum <Type>` dans ce fichier.
    #[test]
    fn tout_parametre_de_verbe_designe_un_ensemble_fini() {
        for (variante, nom, type_champ) in champs_par_variante(&bloc_apres(SOURCE, "pub enum Verb"))
        {
            if type_champ.contains("String") {
                assert!(
                    CHAMPS_TEXTE_ADMIS
                        .iter()
                        .any(|(v, c, _)| *v == variante && *c == nom),
                    "SEC-02 / ADR-0006 : « {variante}.{nom}: {type_champ} » est un texte \
                     libre non répertorié. Soit il désigne une cible, et il faut une \
                     énumération fermée ; soit il est légitimement libre, et il faut \
                     inscrire le couple dans CHAMPS_TEXTE_ADMIS avec sa justification — \
                     ce qui est précisément la revue qu'on veut provoquer."
                );
                continue;
            }

            // Un type validé à la construction est admis, à trois conditions
            // vérifiées ci-dessous par `un_type_valide_ne_se_contourne_pas`.
            // **Le couple, jamais le type seul.** Un type validé admis pour un
            // verbe ne l'est pas pour tous : l'exemption du chemin d'exclusion
            // Defender bénissait sinon `ApplyProfile { path: ExclusionPath }`,
            // mesuré vert sur les treize tests avant ce resserrement.
            if TYPES_VALIDES_ADMIS
                .iter()
                .any(|(v, c, t, _)| *v == variante && *c == nom && *t == type_champ)
            {
                continue;
            }

            // Le cas qui compte, et qui doit se dire clairement : le type est
            // admis AILLEURS. Sans ce message, la barrière tombait plus bas en
            // annonçant « déclaration `pub enum ExclusionPath` introuvable »,
            // c'est-à-dire en envoyant le contributeur déclarer une énumération
            // quand le geste attendu est de justifier une exemption. Un
            // garde-fou dont on ne lit pas le message finit neutralisé plutôt
            // que compris.
            assert!(
                !types_valides_distincts().contains(&type_champ.as_str()),
                "SEC-02 / ADR-0006 : « {variante}.{nom}: {type_champ} » emprunte un type \
                 validé admis pour un AUTRE verbe. Une exemption vaut pour le couple qui \
                 la porte, jamais pour le type : sans quoi l'exception accordée au chemin \
                 d'exclusion Defender bénirait tout futur verbe acceptant un chemin, \
                 c'est-à-dire le « RunScript {{ path }} » que ce fichier documente comme \
                 son angle mort. Inscrivez le couple dans TYPES_VALIDES_ADMIS avec sa \
                 justification, et ouvrez l'ADR que docs/08-CONVENTIONS.md exige."
            );

            // Sinon, le type doit être une énumération déclarée ICI. Si elle vit
            // ailleurs, l'extraction panique en le disant : c'est le seul
            // comportement acceptable, la barrière ne lisant qu'un fichier.
            for ligne in bloc_apres(SOURCE, &format!("pub enum {type_champ}"))
                .lines()
                .map(str::trim)
            {
                if ligne.is_empty() || ligne.starts_with('#') {
                    continue;
                }
                assert!(
                    !ligne.contains('{') && !ligne.contains('('),
                    "SEC-02 / ADR-0006 : « {ligne} », dans l'énumération « {type_champ} » \
                     du champ « {variante}.{nom} », porte une donnée. Une variante \
                     porteuse rend l'ensemble des cibles infini — le verbe redevient une \
                     écriture de registre arbitraire."
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
    /// Un exemplaire de chaque verbe, construit **contre le type**.
    ///
    /// C'est ce qui en fait une source de vérité indépendante du source : le
    /// compilateur refuse un échantillon qui ne correspondrait pas à
    /// l'énumération réellement exposée, et sa sérialisation donne les champs
    /// exacts. Aucune ruse textuelle ne l'atteint.
    fn echantillons() -> Vec<Verb> {
        vec![
            Verb::Scan { domain: None },
            Verb::TakeSnapshot {
                subject: SnapshotSubject::LabVirtualMachine,
            },
            Verb::RestoreSnapshot {
                snapshot_id: SnapshotId::try_from("snap-1".to_owned())
                    .expect("un identifiant d'échantillon doit être valide"),
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
                path: ExclusionPath::try_from(r"D:\src\target".to_owned())
                    .expect("un chemin d'échantillon doit être valide"),
                reason: "artefacts de build".into(),
                // L'horloge est injectée : un échantillon dont la validité
                // dépendrait de la date du jour deviendrait rouge tout seul.
                expires: Expiry::depuis(
                    "2026-12-01T00:00:00Z",
                    chrono::DateTime::parse_from_rfc3339("2026-08-02T00:00:00Z")
                        .expect("date de référence valide")
                        .with_timezone(&chrono::Utc),
                )
                .expect("une expiration d'échantillon doit être valide"),
            },
            Verb::Isolate,
        ]
    }

    #[test]
    fn aucun_verbe_ne_transporte_dexecution_arbitraire() {
        let interdits = INTERDITS;
        let echantillons = echantillons();

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

    /// Instant de référence des tests. **Jamais `Utc::now()`** : un test dont
    /// la validité dépend du jour où il tourne devient rouge tout seul.
    fn maintenant() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-08-02T00:00:00Z")
            .expect("date de référence valide")
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn une_exclusion_defender_exige_raison_et_expiration() {
        // Exigence D11-02 : pas de champ optionnel ici. Le typage impose la
        // justification — on ne peut pas construire l'appel sans elle.
        //
        // Ce test était tautologique : il vérifiait que le JSON porte deux
        // clefs, ce que la définition de la structure garantissait déjà, et
        // restait vert avec `reason: ""` et `expires: "hier"`. Il porte
        // désormais sur ce qui compte, la VALIDITÉ des valeurs.
        let v = Verb::AddDefenderExclusion {
            path: ExclusionPath::try_from(r"D:\src\target".to_owned()).expect("chemin valide"),
            reason: "400k fichiers, +6 min par build".to_owned(),
            expires: Expiry::depuis("2026-12-01T00:00:00Z", maintenant()).expect("date valide"),
        };
        let json = serde_json::to_string(&v).expect("un verbe est sérialisable");
        assert!(json.contains("reason"));
        assert!(json.contains("expires"));
    }

    #[test]
    fn une_expiration_passee_ou_sans_plafond_est_refusee() {
        // « hier », « » et « 2099-01-01 » compilaient tous les trois quand le
        // champ était une chaîne. D11-02 exige une expiration ; le type garantit
        // désormais sa validité, pas seulement sa présence.
        for (brut, pourquoi) in [
            ("2026-07-01T00:00:00Z", "déjà passée"),
            (
                "2026-08-02T00:00:00Z",
                "l'instant même, donc expirée à sa création",
            ),
            ("2030-01-01T00:00:00Z", "au-delà de l'horizon d'un an"),
            ("hier", "pas une date"),
            ("", "vide"),
            ("2026-12-01", "sans fuseau, donc ambiguë"),
        ] {
            assert!(
                Expiry::depuis(brut, maintenant()).is_err(),
                "« {brut} » devrait être refusée : {pourquoi}"
            );
        }

        // Et la borne haute, éprouvée des deux côtés.
        assert!(Expiry::depuis("2027-08-02T00:00:00Z", maintenant()).is_ok());
        assert!(Expiry::depuis("2027-08-04T00:00:00Z", maintenant()).is_err());
    }

    #[test]
    fn un_chemin_dexclusion_nouvre_jamais_tout_le_disque() {
        // Une exclusion de dossier couvre tous ses sous-dossiers, les jokers
        // sont acceptés, et les variables sont développées. « C:\ » et
        // « %SystemDrive%\* » sont donc « désactiver Defender » sous un autre
        // nom, que le §7 refuse comme verbe atomique.
        for (brut, pourquoi) in [
            (r"C:\", "racine de volume"),
            (r"C:\\", "racine de volume, écrite autrement"),
            (r"C:\*\*", "joker sur tout le disque"),
            (r"C:\Windows", "répertoire système entier"),
            (r"C:\Users", "tous les profils"),
            (
                r"%TEMP%\build",
                "variable développée en contexte LocalSystem",
            ),
            (r"$env:TEMP\build", "variable, autre syntaxe"),
            (r"D:\src\..\..\Windows", "remontée d'arborescence"),
            (
                r"src\target",
                "relatif : résolu contre un dossier courant inconnu",
            ),
            ("", "vide"),
        ] {
            assert!(
                ExclusionPath::try_from(brut.to_owned()).is_err(),
                "« {brut} » devrait être refusé : {pourquoi}"
            );
        }

        // Ce qui reste légitime doit passer : une barrière qui refuse le cas
        // d'usage réel se fait désarmer par le premier contributeur pressé.
        for legitime in [
            r"D:\src\target",
            r"C:\Windows\Temp\ks-build",
            r"C:\Users\pc\projets\keystone\target",
        ] {
            assert!(
                ExclusionPath::try_from(legitime.to_owned()).is_ok(),
                "« {legitime} » est une exclusion parfaitement normale"
            );
        }
    }

    #[test]
    fn un_identifiant_dinstantane_ne_peut_pas_designer_un_chemin() {
        // Restaurer, c'est appliquer en SYSTEM un contenu que l'appelant
        // désigne. L'identifiant ne doit donc jamais pouvoir sortir de l'index.
        for brut in [
            r"..\..\evil",
            "snap/../../etc",
            r"C:\evil",
            "snap\0null",
            "snap id",
            "",
            "a:b",
            "\\\\serveur\\partage",
        ] {
            assert!(
                SnapshotId::try_from(brut.to_owned()).is_err(),
                "« {brut} » devrait être refusé"
            );
        }

        for legitime in [
            "snap-1",
            "01J8ZQ4K7XY2M3N4P5Q6R7S8T9",
            "instantane_2026_08_02",
        ] {
            assert!(SnapshotId::try_from(legitime.to_owned()).is_ok());
        }

        // La borne de longueur, des deux côtés.
        assert!(SnapshotId::try_from("a".repeat(64)).is_ok());
        assert!(SnapshotId::try_from("a".repeat(65)).is_err());
    }

    #[test]
    fn la_validation_sapplique_aussi_a_la_deserialisation() {
        // **La porte qui compte.** Le client parle au broker par le réseau, pas
        // par le constructeur : une validation que serde contourne ne protège
        // de rien. C'est ce que `serde(try_from)` garantit, et ce test l'éprouve
        // plutôt que de le croire.
        let attaque = r#"{"verb":"restore-snapshot","snapshot_id":"..\\..\\evil"}"#;
        assert!(
            serde_json::from_str::<Verb>(attaque).is_err(),
            "un identifiant invalide doit être refusé À LA DÉSÉRIALISATION"
        );

        let exclusion = r#"{"verb":"add-defender-exclusion","path":"C:\\","reason":"x","expires":"2027-01-01T00:00:00Z"}"#;
        assert!(
            serde_json::from_str::<Verb>(exclusion).is_err(),
            "une racine de volume doit être refusée à la désérialisation"
        );

        // Et le cas légitime passe bien la même porte.
        let valide = r#"{"verb":"restore-snapshot","snapshot_id":"snap-1"}"#;
        assert!(serde_json::from_str::<Verb>(valide).is_ok());
    }
}

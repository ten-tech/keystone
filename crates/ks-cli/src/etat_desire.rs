//! Lecture de `workstation.yaml` — le fichier d'état désiré (ADR-0016).
//!
//! ## Lecture seule, et pas par timidité
//!
//! Ce module **lit**. Il n'écrit pas, et le crate YAML est configuré pour ne pas
//! le pouvoir : `default-features = false, features = ["deserialize"]`. Un
//! sérialiseur serde a été mesuré et écarté — il émet
//! `security.services.windefend.startup: automatique` sans guillemets, donc un
//! fichier qui se relit correctement **par chance**, et il n'émet aucun
//! commentaire, ce qui annulerait la contrepartie promise par l'ADR-0010 à la
//! forme plate. L'écriture passera par un émetteur maison, dans un lot
//! ultérieur.
//!
//! ## Ce que le document promet, et ce qu'il ne promet pas
//!
//! Le premier niveau est **fermé** : `deny_unknown_fields`. Une clé inattendue à
//! la racine est une faute de frappe ou un format plus récent ; dans les deux
//! cas, l'ignorer en silence est le mauvais choix. Les structures fermées des
//! niveaux suivants — `metadata`, une entrée d'`acceptedDrift` — le sont pour la
//! même raison, écrite une fois ici : ce sont des formes connues, un champ
//! inconnu y est une faute.
//!
//! `desired`, lui, est **délibérément ouvert** : n'importe quelle chaîne y est
//! une clé valide, parce qu'il n'existe pas de catalogue statique des chemins
//! d'items (ADR-0010). Le seul référentiel est ce qu'un scan a observé.
//!
//! ## La table `desired` sort d'ici non typée, et c'est voulu
//!
//! Elle est lue en [`ScalaireBrut`], c'est-à-dire « ce que YAML a donné ». Le
//! typage se fait ensuite, item par item, par [`ks_core::Desire::contraindre`], contre la
//! **forme de la valeur constatée**. C'est le seul moyen de neutraliser le
//! typage implicite de YAML dans un fichier édité à la main, où c'est l'humain
//! qui écrira `off`.
//!
//! Conséquence assumée, et qui n'est pas un défaut de ce module : `ks diff` a
//! besoin d'un scan pour typer le fichier. Un état désiré n'a de sens que
//! confronté à une machine.
//!
//! ## Ce que ce module ne fait pas
//!
//! Il ne lit pas le disque : `ks import` et `ks diff` apporteront la source et
//! le chemin à afficher. Il ne signale pas non plus les chemins « déclarés, non
//! observés » — cela demande l'inventaire, donc la commande, pas le lecteur.
//!
//! ## Le schéma JSON est une sortie de ce fichier, plus un document
//!
//! `schema/workstation.schema.json` est **produit** par [`schema_json`] depuis
//! les types ci-dessous (ADR-0010, décision n° 2). Il a été écrit à la main
//! pendant toute la Phase 0, et il décrivait alors une forme imbriquée que plus
//! aucun code ne lisait : cohérent avec son exemple, et avec rien d'autre.
//!
//! Deux garde-fous, et ils ne disent pas la même chose :
//!
//! * `le_schema_versionne_est_celui_que_les_types_produisent` compare le fichier
//!   commité à ce que les types produisent — la divergence casse `cargo test` ;
//! * le travail `schema` de la CI régénère et refuse tout écart, pour le cas où
//!   quelqu'un modifierait les deux du même geste.
//!
//! ### Le schéma décrit ce que le lecteur accepte, et rien de plus
//!
//! Aucune contrainte n'y est ajoutée que [`EtatDesire::lire`] ne tienne. Le
//! schéma écrit à la main imposait par exemple `^[A-Za-z0-9._-]{1,63}$` sur le
//! nom de machine, que rien ne vérifiait côté produit : un fichier refusé par le
//! validateur et accepté par Keystone, ou l'inverse, est exactement la
//! divergence que cette ADR ferme. Un jour où `lire` contraindra ce nom, la
//! contrainte s'écrira sur le type et le schéma la reprendra tout seul.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use ks_core::ScalaireBrut;

/// La seule version de format que ce lecteur sait relire.
///
/// Un document qui en annonce une autre est refusé plutôt que lu au mieux : un
/// champ dont la sémantique a changé se relit sans erreur et ment ensuite.
pub const VERSION_DE_FORMAT: &str = "keystone/v1";

/// Le seul genre de document attendu, aujourd'hui.
pub const GENRE_ATTENDU: &str = "Workstation";

/// Le fichier d'état désiré, tel qu'il est écrit et relu.
///
/// La forme est celle décidée par l'ADR-0010 : un en-tête, une table plate
/// indexée par chemin d'item, une liste d'écarts acceptés.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(
    title = "Keystone — workstation.yaml",
    extend("$id" = "https://keystone.invalid/schema/workstation.schema.json")
)]
pub struct EtatDesire {
    /// Version du format, `keystone/v1`.
    #[schemars(extend("const" = VERSION_DE_FORMAT))]
    pub api_version: String,
    /// Genre du document, `Workstation`.
    #[schemars(extend("const" = GENRE_ATTENDU))]
    pub kind: String,
    /// De quelle machine il s'agit, et de quoi elle hérite.
    pub metadata: Metadata,
    /// Ce qui est voulu, indexé par chemin d'item — **avant tout typage**.
    ///
    /// `BTreeMap` et non `HashMap` : l'ordre des clés est stable d'une lecture à
    /// l'autre, donc un diff affiché deux fois s'affiche deux fois pareil.
    ///
    /// Le schéma décrit la **valeur** par `ScalaireDeclare`, privé à ce module, et laisse la clé
    /// libre : il n'existe pas de catalogue statique des chemins d'items, et en
    /// inventer un ici en ferait une troisième source de vérité, à côté des
    /// collecteurs et du modèle (ADR-0010, dette assumée).
    ///
    /// La `description` du schéma est écrite à part, et c'est délibéré : elle
    /// s'affiche dans l'éditeur de celui qui écrit le yaml, pour qui le choix
    /// entre `BTreeMap` et `HashMap` ne veut rien dire. Deux lecteurs, deux
    /// textes ; la **forme**, elle, reste dérivée du type.
    #[serde(default)]
    #[schemars(
        with = "BTreeMap<String, ScalaireDeclare>",
        description = "Ce qui est voulu, une ligne par chemin d'item, tel que `ks scan` \
                       les nomme. Toute chaîne est une clé valide : le référentiel des \
                       chemins est ce qu'un scan a observé, jamais une liste figée."
    )]
    pub desired: BTreeMap<String, ScalaireBrut>,
    /// Les écarts volontairement tolérés, avec leur échéance.
    #[serde(default)]
    pub accepted_drift: Vec<EcartAccepte>,
}

/// De quelle machine parle le document.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    /// Nom de la machine.
    pub name: String,
    /// Surcouche de flotte dont ce fichier hérite (D2-10, D13-01).
    #[serde(default)]
    pub inherits: Option<String>,
    /// Qui tient ce poste.
    #[serde(default)]
    pub owner: Option<String>,
    /// À quoi il sert, en une phrase.
    #[serde(default)]
    pub description: Option<String>,
}

/// Un écart toléré volontairement, et jusqu'à quand.
///
/// Exigence D2-06 : la raison **et** la date d'expiration sont obligatoires, et
/// c'est le typage qui le tient — aucun des deux champs n'est optionnel. C'est
/// le seul mécanisme qui empêche un fichier d'état de pourrir sous une couche
/// d'exceptions dont personne ne se rappelle le motif.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcartAccepte {
    /// Le chemin de l'item toléré.
    pub item: String,
    /// Pourquoi cet écart est toléré. Ni vide, ni blanche.
    pub reason: Raison,
    /// Quand la tolérance expire, et l'écart redevient actif tout seul.
    pub expires: chrono::NaiveDate,
    /// Qui a décidé.
    pub decided_by: String,
    /// Quand la décision a été prise.
    pub decided_at: ks_core::Timestamp,
}

/// Une raison de tolérance, dont l'état vide est **inconstructible** depuis un fichier.
///
/// # Pourquoi un type plutôt qu'un contrôle
///
/// `reason: String` tenait la moitié de l'exigence D2-06 : le typage rendait le
/// champ obligatoire, donc `reason` ne pouvait pas manquer. Il ne disait rien de
/// son contenu, et `reason: ""` passait la lecture sans un mot. C'est très
/// exactement l'exception permanente silencieuse que l'exigence interdit
/// nommément, arrivée par la porte d'à côté.
///
/// Le refus est donc porté par le type et non par une fonction de validation
/// appelée quelque part : une fonction, on oublie de l'appeler ; un type, le
/// compilateur l'impose. C'est le même geste que `Vrai`, privé à ce module, pour
/// `absent: false`.
///
/// La blancheur compte autant que le vide : `reason: "   "` est un contournement
/// à un espace près, et il serait le premier trouvé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[schemars(
    inline,
    // `\S` exige au moins un caractère non blanc — la traduction exacte du refus
    // porté par `Deserialize` ci-dessous. `minLength: 1` laisserait passer
    // « reason: "   " », donc dirait au validateur externe autre chose que ce que
    // le produit fait, et une divergence entre les deux est précisément ce que
    // l'ADR-0010 ferme.
    extend("pattern" = r"\S"),
    description = "Pourquoi cet écart est toléré. Une phrase, pas un mot : ce texte est \
                   ce qu'on relira dans six mois pour décider de reconduire ou non."
)]
pub struct Raison(String);

impl Raison {
    /// Le texte de la raison.
    #[must_use]
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Raison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Raison {
    fn deserialize<D: serde::Deserializer<'de>>(deserialiseur: D) -> Result<Self, D::Error> {
        let texte = String::deserialize(deserialiseur)?;
        if texte.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "une tolérance sans raison est une exception permanente : \
                 écrivez pourquoi cet écart est accepté",
            ));
        }
        Ok(Self(texte))
    }
}

/// Ce qu'une entrée de `desired` a le droit d'être, **pour le seul schéma**.
///
/// C'est le décalque de [`ScalaireBrut`], qui vit dans `ks-core`. Le décalque
/// existe pour une raison de frontière, pas de confort : `ks-broker` dépend de
/// `ks-core`, et y déclarer `schemars` ajouterait six crates à l'arbre du seul
/// composant élevé du projet. Le modèle de menace l'interdit sans ADR dédiée
/// (§7, adversaire A4), et l'ADR-0010 l'exclut déjà en scopant la génération à
/// `ks-cli`.
///
/// **Ce n'est pas une seconde source de vérité, et un test le tient.**
/// `le_decalque_du_scalaire_decrit_ce_que_ks_core_ecrit` confronte, variante par
/// variante, ce que [`ScalaireBrut`] sérialise à ce que ce type relit, sous un
/// `match` exhaustif sans bras `_` : ajouter une variante à [`ScalaireBrut`]
/// casse la **compilation** de ce crate avant qu'un test s'exécute.
///
/// Le type reste privé : il ne décrit rien, il ne relit rien du produit, et
/// l'exposer inviterait à s'en servir comme d'un modèle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(
    description = "La valeur voulue pour un item : un booléen, un nombre, un texte, \
                   une liste de textes, ou `{ absent: true }` pour exiger que l'item \
                   n'existe pas. Un texte s'entoure de guillemets — sans eux, YAML lit \
                   `off`, `no` et `0x9` comme un booléen ou un nombre."
)]
enum ScalaireDeclare {
    /// `{ absent: true }` — l'item ne doit pas exister.
    Absent(DesirDabsence),
    /// `true` ou `false`, y compris ce que YAML retype depuis `off`, `no`, `on`.
    Bool(bool),
    /// Un nombre entier.
    Int(i64),
    /// Un texte : jeton, version, chemin, numéro de série.
    Text(String),
    /// Une liste de textes.
    List(Vec<String>),
}

/// La forme objet de l'absence, `{ absent: true }`.
///
/// `absent: false` est refusé par `ks-core` — « une absence ne se nie pas » — et
/// le `const` le dit au validateur externe. Une valeur que le produit rejette et
/// que le schéma accepterait est la moitié de la divergence qu'on ferme ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(description = "Le désir que cet item n'existe pas sur la machine.")]
struct DesirDabsence {
    /// Toujours `true`. Le champ porte le sens ; sa valeur ne fait que le confirmer.
    absent: Vrai,
}

/// Un booléen qui ne vaut que `true`, et le refuse à la relecture.
///
/// Un `bool` nu aurait suffi pour le schéma, puisque le `const` s'y ajoute par
/// attribut. Il ne suffisait pas pour le **test** : mesuré, le décalque relisait
/// alors `{ absent: false }` que `ks-core` refuse, et un décalque plus permissif
/// que la chose décalquée ne prouve plus rien de ce qu'il prétend décrire.
///
/// Le refus est donc porté par le type, comme dans `ks-core`, et le `const: true`
/// du schéma en découle plutôt que de l'affirmer tout seul.
///
/// `inline` : une définition nommée de plus dans le schéma, pour un booléen
/// contraint employé à un seul endroit, rendrait moins lisible un document qu'on
/// demande à des humains de lire.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[schemars(inline, extend("const" = true), description = "Toujours `true`.")]
struct Vrai(bool);

impl<'de> Deserialize<'de> for Vrai {
    fn deserialize<D: serde::Deserializer<'de>>(deserialiseur: D) -> Result<Self, D::Error> {
        // Aucun utilisateur ne verra jamais cette phrase — ce type ne relit rien
        // d'un fichier, seul le test le désérialise. Elle reprend mot pour mot
        // celle de `ks-core` pour que la comparaison des deux refus reste
        // immédiate à la lecture.
        if bool::deserialize(deserialiseur)? {
            Ok(Self(true))
        } else {
            Err(serde::de::Error::custom(
                "« absent: false » ne décrit rien : une absence ne se nie pas",
            ))
        }
    }
}

/// Le schéma JSON du fichier d'état désiré, tel qu'il est versionné.
///
/// La sortie est **déterministe et complète** : indentée de deux espaces,
/// terminée par un saut de ligne, sans horodatage ni chemin de machine. Deux
/// exécutions donnent les mêmes octets, faute de quoi le travail de CI qui
/// refuse tout écart signalerait un changement à chaque exécution et cesserait
/// d'être lu.
///
/// # Panique
///
/// Jamais : la structure rendue par `schemars` est un objet JSON, dont la
/// sérialisation ne peut échouer que sur un `f64` non fini ou une clé non
/// textuelle, et le schéma n'en porte aucun. L'`expect` dit cette impossibilité
/// plutôt que ce qui a échoué.
#[must_use]
pub fn schema_json() -> String {
    let schema = schemars::schema_for!(EtatDesire);
    let mut rendu = serde_json::to_string_pretty(&schema)
        .expect("un schéma ne porte ni flottant non fini ni clé non textuelle");
    rendu.push('\n');
    rendu
}

/// Ce qui peut empêcher de relire un fichier d'état désiré.
///
/// Le message destiné à l'utilisateur et le détail technique sont **deux champs
/// distincts**, jamais une chaîne concaténée : c'est ce qui permet d'afficher
/// l'un et de journaliser l'autre.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurDeLecture {
    /// Le document n'a pas pu être relu en entier.
    ///
    /// Trois causes se rejoignent ici, et le détail seul les distingue : une clé
    /// inconnue à la racine, une clé écrite deux fois, ou un YAML invalide
    /// (tabulation, indentation irrégulière, guillemet non fermé).
    ///
    /// Le détail vient du crate de lecture, **en anglais**. Il ne suit donc pas
    /// la règle de voix du projet, et c'est une limite consignée par
    /// l'ADR-0016 : on préfère un message étranger qui nomme la ligne fautive à
    /// un message français qui ne la nomme pas.
    #[error(
        "Le fichier d'état désiré n'a pas pu être relu en entier : une clé y est \
         inconnue, écrite deux fois, ou le document n'est pas un YAML valide. \
         Tant que la lecture échoue, rien n'est comparé. Le détail technique \
         nomme la ligne en cause."
    )]
    Document {
        /// Ce que le lecteur YAML a répondu, mot pour mot.
        detail: String,
    },

    /// L'en-tête annonce un format que ce lecteur ne sait pas relire.
    #[error(
        "Le fichier d'état désiré annonce « {trouve} », et ce lecteur relit \
         « {attendu} ». Rien n'a été comparé. Vérifiez l'en-tête du fichier."
    )]
    Format {
        /// Ce que le fichier annonce.
        trouve: String,
        /// Ce que ce lecteur sait relire.
        attendu: &'static str,
    },

    /// Deux tolérances visent le même chemin d'item.
    ///
    /// `acceptedDrift` est une liste, pas une table : rien dans la forme
    /// n'empêche d'y écrire deux fois le même chemin, et rien nulle part ne
    /// disait laquelle l'emporte. Les deux réponses possibles sont mauvaises :
    /// prendre la première ignore en silence une décision plus récente, prendre
    /// la dernière ignore en silence une échéance plus courte. On refuse le
    /// document, comme pour la clé écrite deux fois dans `desired`.
    #[error(
        "Deux tolérances visent « {item} », avec des raisons ou des échéances qui \
         peuvent différer. Rien n'a été comparé : Keystone ne choisit pas à votre \
         place laquelle vaut. Gardez celle qui a cours et retirez l'autre."
    )]
    AcceptationEnDouble {
        /// Le chemin visé deux fois.
        item: String,
    },
}

impl EtatDesire {
    /// Relit un fichier d'état désiré depuis sa source.
    ///
    /// La source arrive en mémoire plutôt qu'en chemin : le lecteur reste pur,
    /// donc testable sans disque, et c'est la commande qui décide d'où vient le
    /// texte et quel chemin afficher.
    ///
    /// # Erreurs
    ///
    /// * [`ErreurDeLecture::Document`] — clé inconnue à la racine, clé en
    ///   double, ou YAML invalide.
    /// * [`ErreurDeLecture::Format`] — `apiVersion` ou `kind` inattendu.
    pub fn lire(source: &str) -> Result<Self, ErreurDeLecture> {
        let document: Self =
            serde_saphyr::from_str(source).map_err(|e| ErreurDeLecture::Document {
                detail: e.to_string(),
            })?;

        // Validation au boundary : ce qui entre depuis un fichier est aussi peu
        // fiable qu'un input réseau, et `workstation.yaml` est inscriptible par
        // l'utilisateur — donc par l'adversaire A1.
        for (trouve, attendu) in [
            (&document.api_version, VERSION_DE_FORMAT),
            (&document.kind, GENRE_ATTENDU),
        ] {
            if trouve != attendu {
                return Err(ErreurDeLecture::Format {
                    trouve: trouve.clone(),
                    attendu,
                });
            }
        }

        // Le doublon se cherche après la lecture, et non pendant : `serde` voit
        // une liste, et une liste n'a pas de notion de clé en double. C'est le
        // sens métier qui en fait une, et c'est donc ici qu'il se contrôle.
        let mut vus: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for accepte in &document.accepted_drift {
            if !vus.insert(accepte.item.as_str()) {
                return Err(ErreurDeLecture::AcceptationEnDouble {
                    item: accepte.item.clone(),
                });
            }
        }

        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use ks_core::{Desire, FormeAttendue, ItemValue};

    use super::*;

    /// Un document minimal, auquel le corps du test ajoute ce qu'il éprouve.
    const EN_TETE: &str = "\
apiVersion: keystone/v1
kind: Workstation
metadata:
  name: WKS-EXEMPLE-01
";

    /// Relit un document fait d'un seul scalaire déclaré.
    fn lit_un_scalaire(ecrit: &str) -> Result<ScalaireBrut, ErreurDeLecture> {
        let source = format!("{EN_TETE}desired:\n  un.chemin: {ecrit}\n");
        let document = EtatDesire::lire(&source)?;
        Ok(document
            .desired
            .get("un.chemin")
            .cloned()
            .expect("la clé vient d'être écrite"))
    }

    // ── La batterie des vingt et un scalaires ──────────────────────────────
    //
    // Vingt et une valeurs réellement relevées sur la machine de référence ou
    // plausibles dans un fichier écrit à la main, telles que l'ADR-0016 les a
    // mesurées. La table ci-dessous est la mesure, pas une intention : chaque
    // ligne dit ce que YAML **donne**, y compris quand c'est absurde.

    /// Ce que le lecteur rend d'un scalaire écrit tel quel.
    #[derive(Debug, PartialEq, Eq)]
    enum CeQueYamlDonne {
        /// Un booléen, que l'humain ait écrit `false` ou `off`.
        Booleen(bool),
        /// Un entier, que l'humain ait écrit `9` ou `0x9`.
        Entier(i64),
        /// Un texte, seul cas où le fichier rend ce qu'on y a écrit.
        Texte,
        /// Le document entier est refusé : `08`, `~`, `null`.
        Refus,
    }

    /// Les vingt et un scalaires, et ce que YAML en fait.
    ///
    /// L'ordre suit le tableau de l'ADR-0016. `false` et `False` complètent la
    /// famille de sa première ligne, qui n'en citait que les quatre membres
    /// surprenants — ce sont eux qui portent le compte à vingt et un.
    const BATTERIE: &[(&str, CeQueYamlDonne)] = &[
        // Ce que YAML lit comme faux.
        ("no", CeQueYamlDonne::Booleen(false)),
        ("n", CeQueYamlDonne::Booleen(false)),
        ("off", CeQueYamlDonne::Booleen(false)),
        ("Off", CeQueYamlDonne::Booleen(false)),
        ("false", CeQueYamlDonne::Booleen(false)),
        ("False", CeQueYamlDonne::Booleen(false)),
        // Ce que YAML lit comme vrai.
        ("yes", CeQueYamlDonne::Booleen(true)),
        ("y", CeQueYamlDonne::Booleen(true)),
        ("on", CeQueYamlDonne::Booleen(true)),
        ("true", CeQueYamlDonne::Booleen(true)),
        ("True", CeQueYamlDonne::Booleen(true)),
        // Les deux items réellement exposés sur la machine de référence :
        // `inventory.os.kernel` vaut Text("26200") et
        // `security.firmware.microcode_revision` vaut Text("23410000").
        // Déclarés sans guillemets, ils reviendraient en entier et produiraient
        // un écart PERMANENT sur une machine qui n'a jamais changé.
        ("23410000", CeQueYamlDonne::Entier(23_410_000)),
        ("26200", CeQueYamlDonne::Entier(26_200)),
        // L'hexadécimal, que personne n'a demandé.
        ("0x9", CeQueYamlDonne::Entier(9)),
        // Le zéro en tête ne donne pas 8 : il fait échouer la lecture.
        ("08", CeQueYamlDonne::Refus),
        // Les deux écritures du vide. Un désir d'absence s'écrit
        // `{ absent: true }`, jamais `~` ni `null`.
        ("~", CeQueYamlDonne::Refus),
        ("null", CeQueYamlDonne::Refus),
        // Et les quatre qui se comportent : un jeton (ADR-0015), un numéro de
        // série, une version, une date.
        ("automatique", CeQueYamlDonne::Texte),
        ("P0CN20WW", CeQueYamlDonne::Texte),
        ("25.3.0", CeQueYamlDonne::Texte),
        ("2024-10-21", CeQueYamlDonne::Texte),
    ];

    #[test]
    fn les_vingt_et_un_scalaires_donnent_ce_que_yaml_a_decide() {
        assert_eq!(BATTERIE.len(), 21, "la batterie a changé de taille");

        for (ecrit, attendu) in BATTERIE {
            let obtenu = lit_un_scalaire(ecrit);
            match (attendu, &obtenu) {
                (CeQueYamlDonne::Booleen(b), Ok(ScalaireBrut::Bool(lu))) => {
                    assert_eq!(lu, b, "« {ecrit} »");
                }
                (CeQueYamlDonne::Entier(n), Ok(ScalaireBrut::Int(lu))) => {
                    assert_eq!(lu, n, "« {ecrit} »");
                }
                (CeQueYamlDonne::Texte, Ok(ScalaireBrut::Text(lu))) => {
                    assert_eq!(lu, ecrit, "« {ecrit} » : le texte a été transformé");
                }
                (CeQueYamlDonne::Refus, Err(ErreurDeLecture::Document { .. })) => {}
                _ => panic!("« {ecrit} » : attendu {attendu:?}, obtenu {obtenu:?}"),
            }
        }
    }

    #[test]
    fn huit_des_vingt_et_un_scalaires_sont_retypes_en_silence() {
        // **Le décompte est calculé, jamais recopié.** Est « piégeux » un
        // scalaire que YAML retype SANS BRUIT vers une valeur dont l'écriture
        // littérale n'est pas celle que l'humain a tapée :
        //
        //   `off` devient false, et « off » ne s'écrit pas « false » → piégeux ;
        //   `false` devient false, et « false » s'écrit « false »    → non ;
        //   `0x9` devient 9, et « 0x9 » ne s'écrit pas « 9 »         → piégeux ;
        //   `26200` devient 26200, écrit à l'identique               → non ;
        //   `08` et `~` sont REFUSÉS, donc bruyants                  → non.
        //
        // Ce critère est mécanique : il ne dépend d'aucune liste écrite à la
        // main, donc il ne peut pas dériver quand la batterie s'allonge. Il
        // isole exactement ce qui est dangereux — la conversion muette — et
        // laisse de côté le refus, qui se voit.
        let piegeux: Vec<&str> = BATTERIE
            .iter()
            .filter(|(ecrit, donne)| match donne {
                CeQueYamlDonne::Booleen(b) => ecrit.to_lowercase() != b.to_string(),
                CeQueYamlDonne::Entier(n) => *ecrit != n.to_string(),
                CeQueYamlDonne::Texte | CeQueYamlDonne::Refus => false,
            })
            .map(|(ecrit, _)| *ecrit)
            .collect();

        assert_eq!(
            piegeux,
            ["no", "n", "off", "Off", "yes", "y", "on", "0x9"],
            "la famille des conversions muettes a changé"
        );
        assert_eq!(
            piegeux.len(),
            8,
            "l'ADR-0016 en compte huit sur vingt et un"
        );
    }

    #[test]
    fn face_a_un_item_texte_seuls_les_textes_passent() {
        // Le contrôle de l'ADR-0016 appliqué à toute la batterie : sur un item
        // dont la valeur constatée est un texte — le cas de
        // `security.services.windefend.startup`, qui vaut le jeton
        // « automatique » —, tout ce que YAML n'a pas rendu en texte est
        // refusé. Les huit conversions muettes deviennent ainsi huit refus
        // nommés, ce qui est précisément le but de la décision.
        let observe = ItemValue::Text("automatique".into());
        let mut acceptes = 0;
        let mut refuses = 0;

        for (ecrit, attendu) in BATTERIE {
            let Ok(brut) = lit_un_scalaire(ecrit) else {
                // `08`, `~` et `null` n'arrivent jamais au contrôle : le
                // document entier est refusé avant.
                assert_eq!(*attendu, CeQueYamlDonne::Refus, "« {ecrit} »");
                refuses += 1;
                continue;
            };

            match Desire::contraindre(brut, "security.services.windefend.startup", &observe) {
                Ok(desire) => {
                    assert_eq!(
                        *attendu,
                        CeQueYamlDonne::Texte,
                        "« {ecrit} » accepté à tort"
                    );
                    assert_eq!(desire, Desire::Text((*ecrit).to_owned()));
                    acceptes += 1;
                }
                Err(e) => {
                    assert_ne!(*attendu, CeQueYamlDonne::Texte, "« {ecrit} » refusé à tort");
                    assert_eq!(e.attendu, FormeAttendue::Texte);
                    assert!(
                        e.to_string().contains("Entourez"),
                        "« {ecrit} » : le message ne dit pas quoi faire — {e}"
                    );
                    refuses += 1;
                }
            }
        }

        assert_eq!(acceptes, 4, "les quatre textes de la batterie");
        assert_eq!(refuses, 17, "tout le reste");
    }

    #[test]
    fn le_guillemetage_neutralise_le_typage_a_lecriture() {
        // Ce que l'ADR a mesuré, et pourquoi ça ne suffit pas : le guillemetage
        // règle le cas du fichier que Keystone ÉCRIT. Il ne règle pas celui que
        // l'humain édite pendant sept jours, et c'est l'humain qui écrira `off`.
        for ecrit in ["true", "off", "23410000", "08", "0x9", "2024-10-21"] {
            let cite = format!("\"{ecrit}\"");
            let brut = lit_un_scalaire(&cite).expect("un scalaire cité reste lisible");
            assert_eq!(
                brut,
                ScalaireBrut::Text((*ecrit).to_owned()),
                "« {cite} » n'est pas revenu en texte"
            );
        }
    }

    // ── La politique de champs, et les deux refus qu'elle porte ────────────

    #[test]
    fn une_cle_racine_inconnue_est_refusee() {
        // Une clé inattendue à la racine est une faute de frappe ou un format
        // plus récent. Dans les deux cas, l'ignorer en silence laisserait
        // croire que la déclaration a été prise en compte.
        let source = format!("{EN_TETE}desiredd:\n  un.chemin: true\n");
        let e = EtatDesire::lire(&source).expect_err("clé racine inconnue acceptée");

        let ErreurDeLecture::Document { detail } = &e else {
            panic!("mauvaise erreur : {e:?}");
        };
        assert!(
            detail.contains("desiredd"),
            "le détail ne nomme pas la clé : {detail}"
        );
        // La phrase destinée à l'utilisateur ne porte pas le détail technique :
        // ce sont deux champs, et c'est ce qui permet d'afficher l'un sans
        // l'autre.
        assert!(!e.to_string().contains("desiredd"), "{e}");
    }

    #[test]
    fn une_cle_en_double_est_refusee() {
        // C'est le mode de pourrissement classique d'un fichier de
        // configuration : une exception ajoutée en haut, oubliée, écrasée en
        // bas. La dernière gagnerait en silence.
        let source = format!(
            "{EN_TETE}desired:\n  \
             security.defender.realtime: true\n  \
             security.defender.realtime: false\n"
        );
        let e = EtatDesire::lire(&source).expect_err("clé en double acceptée");

        let ErreurDeLecture::Document { detail } = &e else {
            panic!("mauvaise erreur : {e:?}");
        };
        assert!(
            detail.contains("security.defender.realtime"),
            "le détail ne nomme pas la clé en double : {detail}"
        );
    }

    #[test]
    fn un_entete_inattendu_est_refuse_avant_toute_comparaison() {
        // Un champ dont la sémantique a changé se relit sans erreur et ment
        // ensuite. On refuse la version qu'on ne sait pas relire.
        for (entete, trouve) in [
            (
                "apiVersion: keystone/v2\nkind: Workstation\n",
                "keystone/v2",
            ),
            ("apiVersion: keystone/v1\nkind: Serveur\n", "Serveur"),
        ] {
            let source = format!("{entete}metadata:\n  name: WKS-EXEMPLE-01\n");
            let e = EtatDesire::lire(&source).expect_err("en-tête inattendu accepté");
            assert_eq!(
                e,
                ErreurDeLecture::Format {
                    trouve: trouve.to_owned(),
                    attendu: if trouve.starts_with("keystone") {
                        VERSION_DE_FORMAT
                    } else {
                        GENRE_ATTENDU
                    },
                }
            );
        }
    }

    // ── Le document dans son entier ────────────────────────────────────────

    #[test]
    fn un_document_complet_se_relit_dans_ses_trois_formes_de_desir() {
        let source = "\
apiVersion: keystone/v1
kind: Workstation

metadata:
  name: WKS-EXEMPLE-01
  inherits: ./base.yaml

desired:
  # security — posture de la plateforme
  security.defender.realtime: true
  security.services.windefend.startup: \"automatique\"
  security.platform.hvci_policy: true
  inventory.os.kernel: \"26200\"
  security.defender.asr_rules:
    - BlockOfficeChildProcess
    - BlockCredentialStealing
  security.defender.exclusions.paths:
    absent: true

acceptedDrift:
  - item: security.services.fax.startup
    reason: \"requis temporairement par le pilote du scanner du labo\"
    expires: 2026-10-15
    decidedBy: exemple
    decidedAt: \"2026-07-18T09:12:00+02:00\"
";
        let d = EtatDesire::lire(source).expect("document valide refusé");

        assert_eq!(d.metadata.name, "WKS-EXEMPLE-01");
        assert_eq!(d.metadata.inherits.as_deref(), Some("./base.yaml"));
        assert_eq!(d.desired.len(), 6);

        // Un booléen reste un booléen.
        assert_eq!(
            d.desired["security.defender.realtime"],
            ScalaireBrut::Bool(true)
        );
        // Un jeton cité reste un texte — et `26200` aussi, grâce aux guillemets.
        assert_eq!(
            d.desired["security.services.windefend.startup"],
            ScalaireBrut::Text("automatique".into())
        );
        assert_eq!(
            d.desired["inventory.os.kernel"],
            ScalaireBrut::Text("26200".into())
        );
        // Une liste reste une liste.
        assert_eq!(
            d.desired["security.defender.asr_rules"],
            ScalaireBrut::List(vec![
                "BlockOfficeChildProcess".into(),
                "BlockCredentialStealing".into(),
            ])
        );
        // Et le désir d'absence porte la forme objet décidée par l'ADR-0016,
        // pas l'étiquette `!absent` que l'ADR-0010 proposait : mesuré,
        // celle-ci n'est pas atteignable par un enum `untagged`.
        assert_eq!(
            d.desired["security.defender.exclusions.paths"],
            ScalaireBrut::Absent
        );

        assert_eq!(d.accepted_drift.len(), 1);
        assert_eq!(d.accepted_drift[0].expires.to_string(), "2026-10-15");
    }

    #[test]
    fn letiquette_absent_de_ladr_0010_nest_pas_atteignable() {
        // La mesure qui a fait corriger la décision n° 3 de l'ADR-0010 : une
        // étiquette YAML personnalisée ne traverse pas un enum `untagged`. Ce
        // test la garde, pour qu'on ne réintroduise pas la syntaxe en croyant
        // qu'elle marche.
        let source = format!("{EN_TETE}desired:\n  un.chemin: !absent\n");
        assert!(
            EtatDesire::lire(&source).is_err(),
            "« !absent » a été accepté : la syntaxe de l'ADR-0010 est revenue"
        );
    }

    #[test]
    fn un_ecart_accepte_exige_sa_raison_et_son_echeance() {
        // Exigence D2-06, tenue par le typage : ni `reason` ni `expires` n'est
        // optionnel. Sans elles, un fichier d'état pourrit en trois ans sous
        // une couche d'exceptions dont personne ne se rappelle le motif.
        for manquant in ["reason", "expires"] {
            let mut lignes = vec![
                "  - item: security.services.fax.startup",
                "    reason: \"pilote du scanner du labo\"",
                "    expires: 2026-10-15",
                "    decidedBy: exemple",
                "    decidedAt: \"2026-07-18T09:12:00+02:00\"",
            ];
            lignes.retain(|l| !l.trim_start().starts_with(manquant));
            let source = format!("{EN_TETE}acceptedDrift:\n{}\n", lignes.join("\n"));

            assert!(
                EtatDesire::lire(&source).is_err(),
                "un écart accepté sans « {manquant} » a été accepté"
            );
        }
    }

    #[test]
    fn une_raison_vide_ou_blanche_est_refusee() {
        // **Le défaut que le typage seul ne voyait pas.** `reason: String`
        // rendait le champ obligatoire, jamais son contenu : `reason: ""`
        // passait la lecture sans un mot, et l'exception permanente silencieuse
        // que D2-06 interdit revenait par la porte d'à côté.
        //
        // La blancheur compte autant que le vide : à un espace près, le
        // contournement serait le premier trouvé.
        for vide in ["\"\"", "\"   \"", "\"\\t\""] {
            let source = format!(
                "{EN_TETE}acceptedDrift:\n  \
                 - item: security.services.fax.startup\n    \
                 reason: {vide}\n    \
                 expires: 2026-10-15\n    \
                 decidedBy: exemple\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"\n"
            );
            let e =
                EtatDesire::lire(&source).expect_err(&format!("une raison {vide} a été acceptée"));
            let ErreurDeLecture::Document { detail } = &e else {
                panic!("mauvaise erreur : {e:?}");
            };
            assert!(
                detail.contains("exception permanente"),
                "le détail ne dit pas ce qui manque : {detail}"
            );
        }
    }

    #[test]
    fn une_raison_renseignee_est_acceptee() {
        // Le pendant du refus : sans lui, un contrôle qui refuserait tout
        // passerait le test précédent en trompant sur toute la ligne.
        let source = format!(
            "{EN_TETE}acceptedDrift:\n  \
             - item: security.services.fax.startup\n    \
             reason: \"pilote du scanner du labo\"\n    \
             expires: 2026-10-15\n    \
             decidedBy: exemple\n    \
             decidedAt: \"2026-07-18T09:12:00+02:00\"\n"
        );
        let d = EtatDesire::lire(&source).expect("une raison renseignée est refusée");
        assert_eq!(
            d.accepted_drift[0].reason.texte(),
            "pilote du scanner du labo"
        );
    }

    #[test]
    fn deux_acceptations_du_meme_chemin_sont_refusees() {
        // `acceptedDrift` est une liste : rien dans la forme n'empêche d'y
        // écrire deux fois le même chemin, et les deux résolutions possibles
        // sont mauvaises — la première ignore une décision plus récente, la
        // dernière ignore une échéance plus courte. On refuse le document.
        let une = |echeance: &str| {
            format!(
                "  - item: security.services.fax.startup\n    \
                 reason: \"pilote du scanner du labo\"\n    \
                 expires: {echeance}\n    \
                 decidedBy: exemple\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"\n"
            )
        };
        let source = format!(
            "{EN_TETE}acceptedDrift:\n{}{}",
            une("2026-10-15"),
            une("2036-10-15")
        );
        let e = EtatDesire::lire(&source).expect_err("un doublon de chemin a été accepté");
        assert_eq!(
            e,
            ErreurDeLecture::AcceptationEnDouble {
                item: "security.services.fax.startup".into(),
            }
        );
        // Le message nomme le chemin, sans quoi il faudrait chercher à l'œil
        // dans une liste qui peut être longue.
        assert!(
            e.to_string().contains("security.services.fax.startup"),
            "{e}"
        );
    }

    #[test]
    fn deux_acceptations_de_chemins_differents_passent() {
        // La contre-épreuve : un contrôle de doublon trop large refuserait tout
        // fichier portant plus d'une tolérance, ce qui est l'usage normal.
        let une = |chemin: &str| {
            format!(
                "  - item: {chemin}\n    \
                 reason: \"pilote du scanner du labo\"\n    \
                 expires: 2026-10-15\n    \
                 decidedBy: exemple\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"\n"
            )
        };
        let source = format!(
            "{EN_TETE}acceptedDrift:\n{}{}",
            une("security.services.fax.startup"),
            une("security.defender.realtime")
        );
        let d = EtatDesire::lire(&source).expect("deux chemins distincts refusés");
        assert_eq!(d.accepted_drift.len(), 2);
    }

    // ── Le schéma, et les deux fichiers qu'il engage ───────────────────────

    /// Le schéma tel qu'il est commité, lu à la compilation.
    ///
    /// `include_str!` et non une lecture de disque : le test compare ce que le
    /// dépôt porte, pas ce qu'un `cargo run` vient d'écrire à côté.
    const SCHEMA_COMMITE: &str = include_str!("../../../schema/workstation.schema.json");

    /// L'exemple versionné, lu de la même façon et pour la même raison.
    const EXEMPLE_COMMITE: &str = include_str!("../../../schema/examples/workstation.yaml");

    #[test]
    fn le_schema_versionne_est_celui_que_les_types_produisent() {
        // La décision n° 2 de l'ADR-0010, éprouvée là où elle coûte le moins
        // cher à découvrir. Le travail de CI régénère et refuse tout écart ; ce
        // test-ci fait la même chose sans réseau ni git, donc dès `cargo test`.
        //
        // Le fichier n'est pas normalisé avant comparaison, et c'est délibéré :
        // `.gitattributes` impose LF à la sortie de dépôt, et tolérer CRLF ici
        // ferait passer un fichier que le `git diff` de la CI refuserait. Deux
        // garde-fous qui ne disent pas la même chose valent moins qu'un seul.
        let produit = schema_json();
        if produit == SCHEMA_COMMITE {
            return;
        }

        // `assert_eq!` sur deux documents de cinq kilo-octets recrache les deux
        // en entier, et la ligne qui diffère se cherche à l'œil. On la nomme.
        // Un garde-fou dont on ne lit pas le message finit par être neutralisé
        // plutôt que compris.
        let ecart = produit
            .lines()
            .zip(SCHEMA_COMMITE.lines())
            .position(|(a, b)| a != b);
        let detail = match ecart {
            Some(n) => format!(
                "ligne {} — les types produisent « {} », le fichier porte « {} »",
                n + 1,
                produit.lines().nth(n).unwrap_or_default().trim(),
                SCHEMA_COMMITE.lines().nth(n).unwrap_or_default().trim(),
            ),
            None => format!(
                "les {} premières lignes coïncident, mais le fichier en compte {} \
                 et les types en produisent {}",
                produit.lines().count().min(SCHEMA_COMMITE.lines().count()),
                SCHEMA_COMMITE.lines().count(),
                produit.lines().count(),
            ),
        };
        panic!(
            "schema/workstation.schema.json ne correspond plus aux types : {detail}.\n\
             Régénérez-le : cargo run -q -p ks-cli --example generer-schema \
             > schema/workstation.schema.json"
        );
    }

    #[test]
    fn le_schema_genere_est_stable_dune_execution_a_lautre() {
        // Un générateur qui produirait deux sorties différentes rendrait le
        // contrôle de CI rouge sans qu'aucun type ait bougé — et un garde-fou
        // qui crie sans raison finit par être neutralisé.
        assert_eq!(schema_json(), schema_json());
        assert!(
            schema_json().ends_with("}\n"),
            "le schéma ne se termine pas par un saut de ligne"
        );
    }

    #[test]
    fn lexemple_versionne_est_relu_par_keystone_lui_meme() {
        // **Le point de la décision n° 4.** Un exemple validé par le seul
        // validateur JSON Schema peut très bien être refusé par le produit :
        // c'est exactement l'état d'avant ce lot, où l'exemple portait la forme
        // imbriquée et n'aurait pas passé une seule ligne de ce lecteur.
        //
        // Les deux validations disent des choses différentes, et il faut les
        // deux : le schéma voit la forme des valeurs, le lecteur voit en plus la
        // clé écrite deux fois et l'en-tête qu'il ne sait pas relire.
        let document = EtatDesire::lire(EXEMPLE_COMMITE)
            .expect("l'exemple versionné est refusé par le lecteur de Keystone");

        assert_eq!(document.metadata.name, "WKS-EXEMPLE-01");
        assert!(
            !document.desired.is_empty(),
            "un exemple sans déclaration ne montre rien"
        );

        // Et il exerce réellement les formes qu'il prétend montrer : sans ce
        // contrôle, l'exemple pourrait se réduire à trois booléens sans que rien
        // ne le signale.
        let formes: Vec<Option<ks_core::FormeAttendue>> =
            document.desired.values().map(ScalaireBrut::forme).collect();
        assert!(
            formes.contains(&None),
            "l'exemple ne montre pas le désir d'absence"
        );
        for attendue in [
            ks_core::FormeAttendue::Booleen,
            ks_core::FormeAttendue::Texte,
            ks_core::FormeAttendue::Liste,
        ] {
            assert!(
                formes.contains(&Some(attendue)),
                "l'exemple ne montre aucune valeur en {}",
                attendue.avec_article()
            );
        }

        // L'écart accepté porte sa raison et son échéance, comme D2-06 l'exige.
        assert_eq!(document.accepted_drift.len(), 1);
        assert!(!document.accepted_drift[0].reason.texte().is_empty());
    }

    #[test]
    fn le_decalque_du_scalaire_decrit_ce_que_ks_core_ecrit() {
        // `ScalaireDeclare` est le décalque de `ScalaireBrut` que le schéma
        // décrit, et il vit dans un autre crate que lui. Une liste
        // d'échantillons écrite à la main ne détecterait jamais ce qu'on a
        // oublié d'y mettre : le `match` exhaustif sans bras `_` ci-dessous
        // casse la COMPILATION le jour où `ScalaireBrut` gagne une variante.
        let echantillons = vec![
            ScalaireBrut::Absent,
            ScalaireBrut::Bool(true),
            ScalaireBrut::Bool(false),
            ScalaireBrut::Int(0),
            ScalaireBrut::Int(23_410_000),
            ScalaireBrut::Text(String::new()),
            ScalaireBrut::Text("automatique".into()),
            ScalaireBrut::List(Vec::new()),
            ScalaireBrut::List(vec!["a".into(), "b".into()]),
        ];
        for brut in &echantillons {
            match brut {
                ScalaireBrut::Absent
                | ScalaireBrut::Bool(_)
                | ScalaireBrut::Int(_)
                | ScalaireBrut::Text(_)
                | ScalaireBrut::List(_) => {}
            }
        }

        // Ce que `ks-core` écrit, le décalque le relit — et le réécrit à
        // l'identique. C'est la seule chose qui empêche le schéma de décrire un
        // format que le produit n'emploie pas.
        for brut in echantillons {
            let ecrit = serde_json::to_value(&brut).expect("sérialisation");
            let relu: ScalaireDeclare = serde_json::from_value(ecrit.clone())
                .unwrap_or_else(|e| panic!("{brut:?} → {ecrit} refusé par le décalque : {e}"));
            assert_eq!(
                serde_json::to_value(&relu).expect("sérialisation"),
                ecrit,
                "{brut:?} : le décalque n'écrit pas la même chose que ks-core"
            );
        }

        // Et le refus se décalque aussi : « une absence ne se nie pas ». Le
        // schéma porte `const: true` sur ce champ pour la même raison — une
        // valeur que le produit rejette et que le validateur accepterait est la
        // moitié de la divergence que ce lot ferme.
        let nie = serde_json::json!({ "absent": false });
        assert!(
            serde_json::from_value::<ScalaireBrut>(nie.clone()).is_err(),
            "ks-core accepte « absent: false »"
        );
        assert!(
            serde_json::from_value::<ScalaireDeclare>(nie).is_err(),
            "le décalque accepte « absent: false » que ks-core refuse"
        );

        // Et le schéma le refuse aussi, sans quoi le validateur externe et le
        // produit ne diraient pas la même chose de la même ligne.
        let schema: serde_json::Value =
            serde_json::from_str(&schema_json()).expect("le schéma est du JSON");
        assert_eq!(
            schema
                .pointer("/$defs/DesirDabsence/properties/absent/const")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "le schéma n'impose plus « absent: true »"
        );
    }
}

//! Tâches planifiées, en lecture seule (D2-03, D5-02, Phase 0.4).
//!
//! ## Ce qui est lisible, et par quelle porte
//!
//! Les trois portes évidentes sont fermées à une session non élevée, mesuré sur
//! la machine de référence le 2026-08-24 :
//!
//! | Porte | Réponse |
//! |---|---|
//! | `HKLM\…\Schedule\TaskCache\Tree` | accès refusé |
//! | `HKLM\…\Schedule\TaskCache\Tasks` | accès refusé |
//! | `C:\Windows\System32\Tasks` | accès refusé |
//! | `MSFT_ScheduledTask`, par WMI | 194 lignes |
//!
//! La quatrième répond donc, et c'est la seule. Le module lit `MSFT_ScheduledTask`
//! dans `root\Microsoft\Windows\TaskScheduler`, exactement comme
//! [`crate::etat_effectif`] lit `Win32_DeviceGuard` : une requête, aucune méthode.
//!
//! ## La difficulté réelle : `Actions` est un tableau d'objets hétérogènes
//!
//! Une tâche porte des actions de deux classes — `MSFT_TaskExecAction`, qui
//! lance un binaire, et `MSFT_TaskComHandlerAction`, qui appelle un composant en
//! processus. Mesuré : 82 tâches de la première, 112 de la seconde, aucune des
//! deux à la fois.
//!
//! Le désérialiseur de la bibliothèque demande **chaque champ déclaré** par
//! `get_property` et propage l'erreur avant que serde voie le champ
//! (`de/wbem_class_de.rs`, `next_value_seed`, version verrouillée). Déclarer
//! `Execute` dans une structure imbriquée fait donc échouer la requête **entière**
//! dès la première action COM : mesuré, `0x80041002` et zéro tâche lue, pas 82.
//! `Option<Execute>` n'y change rien — la propriété est cherchée d'abord.
//!
//! La sortie tient en une ligne de serde. Le désérialiseur rend le **nom de
//! classe de l'objet** comme identifiant d'énumération (`deserialize_identifier`),
//! donc une énumération à balise externe dont les variantes portent les noms des
//! classes WMI se désérialise sans qu'aucune propriété absente soit demandée :
//! une action COM ne se voit demander aucun champ, une action exécutable se voit
//! demander `Execute`, et rien de plus. Mesuré : 194 tâches, 84 valeurs
//! `Execute`, en 1,9 s — soit le coût de la requête sans les actions.
//!
//! **Ce que cela préserve vaut plus que la commodité.** La voie alternative
//! (`exec_query` puis `get_property` objet par objet) donne le même résultat pour
//! le même prix, mais elle fait passer `IWbemClassWrapper` dans notre code — et
//! ce type expose `put_property`, `spawn_instance` et `get_method`. La lecture
//! seule cesserait alors d'être portée par la signature pour ne plus tenir qu'à
//! la discipline. `query::<T>()` ne rend que des structures inertes.
//!
//! Le voisinage justifie l'insistance : `PS_ScheduledTask`, dans **le même
//! espace de noms**, porte `RegisterByXml`, `StartByPath`, `SetByObject` et
//! `DisableByName`. `MSFT_ScheduledTask`, la classe lue ici, n'a aucune méthode.
//!
//! ### La limite de cette voie, nommée plutôt que découverte
//!
//! Une classe d'action qui ne figure pas dans l'énumération fait échouer la
//! requête entière — `unknown variant`, mesuré. Le risque est borné : le schéma
//! de cet espace de noms ne contient que **deux** classes concrètes d'action,
//! `MSFT_TaskExecAction` et `MSFT_TaskComHandlerAction`, plus la classe abstraite
//! `MSFT_TaskAction` dont elles héritent (relevé par `meta_class` le 2026-08-24).
//! Et l'échec, s'il survenait, est honnête : six items illisibles, jamais un
//! décompte faux.
//!
//! ## Ce qui est publié, et ce qui ne l'est pas
//!
//! **194 tâches deviennent six items.** Aucun n'est déclarable, et ce n'est pas
//! une limite de phase : écrire une tâche, c'est `RegisterByXml`, c'est-à-dire un
//! verbe que la doctrine du projet refuse définitivement. Classer un de ces items
//! `Reglage` promettrait une convergence qui n'arrivera pas.
//!
//! Ne sont **pas** publiés, et chaque refus repose sur une mesure du 2026-08-24 :
//!
//! * `SecurityDescriptor` — 7 566 caractères cumulés de SDDL, illisibles sans
//!   décodeur d'ACL, donc contraires au principe P6, et porteurs de SID ;
//! * `Triggers` — 9 classes distinctes observées, 57 tâches sans aucun
//!   déclencheur, et aucun item ne demande « quand » ;
//! * `Author` — vide 71 fois sur 194, et les valeurs non vides sont des chaînes
//!   MUI indirectes. Ce n'est pas une attestation d'auteur (ADR-0011) ;
//! * `Arguments` — présents sur 55 des 84 actions exécutables. C'est là que vit
//!   une ligne de commande, et la publier inviterait à lire un verdict dans une
//!   chaîne ;
//! * **le chemin du binaire lui-même** — mesuré, il porte le nom du compte
//!   (`C:\Users\…\PowerToys.exe`). L'item nomme la **tâche**, pas le fichier,
//!   comme [`crate::lettre_de_volume`] ne publie que la lettre d'un volume ;
//! * les CLSID des gestionnaires COM — 94 identifiants opaques, P6 les interdit ;
//! * `running_as_system` (100 tâches) et `RunLevel = 1` (79) — lisibles, mesurés,
//!   et sans lecture possible : dominés par l'arborescence de l'éditeur. Tout ce
//!   qui est lisible n'est pas publiable.
//!
//! ## Où sont les choses, jamais ce qu'elles sont
//!
//! Aucun classement, aucun rang, aucun verdict. `outside_microsoft_root` nomme
//! **l'endroit qu'on a regardé**, et surtout pas un auteur : mesuré, la
//! fonctionnalité `\SoftLanding\…` est de Microsoft et vit hors de `\Microsoft\`,
//! et rien n'interdit à quiconque d'enregistrer une tâche en dedans. C'est la
//! même discipline que « disponible » contre « activé ».
//!
//! Aucune évaluation d'inscriptibilité non plus. « Répertoire inscriptible par
//! l'utilisateur » serait **faux ici** : 4 des 16 tâches hors répertoire système
//! lancent `MpCmdRun.exe` depuis `C:\ProgramData\…`, qu'un compte standard ne
//! possède pas. L'emplacement se vérifie ; l'inscriptibilité se calcule, et un
//! calcul faux devient une accusation.
//!
//! ## Une identité se publie telle que Windows l'a enregistrée
//!
//! Cinq des identités hors `\Microsoft\` portent l'identifiant de sécurité du
//! compte (`\OneDrive Startup Task-S-1-5-21-…`). Ce sont les premiers items de
//! Keystone à en porter un. Les réécrire inventerait une identité que Windows n'a
//! jamais enregistrée, et rendrait le relevé inutilisable pour retrouver la
//! tâche. Un identifiant de sécurité n'est pas un secret ; la distinction méritait
//! d'être écrite plutôt que supposée.

use ks_core::{Domain, Item, ItemValue, Nature, Provenance};

use crate::posture::Lecture;

/// Racine sous laquelle Windows range ses propres tâches.
///
/// **Une convention d'emplacement, pas une attestation d'auteur.** Voir la
/// section « Où sont les choses » en tête de module.
const RACINE_MICROSOFT: &str = r"\Microsoft\";

/// `TASK_STATE_DISABLED`, la seule valeur de `State` que ce module interprète.
///
/// Les cinq valeurs documentées sont 0 inconnu, 1 désactivée, 2 en file,
/// 3 prête, 4 en cours. Mesuré le 2026-08-24 : 31 désactivées, 157 prêtes,
/// 6 en cours, et **aucune** sans état. Les trois autres ne se traduisent en
/// aucun item, donc elles ne se nomment pas ici.
const ETAT_DESACTIVEE: u32 = 1;

/// Ce qu'on dit quand la lecture entière a échoué.
const REFUS_WMI: &str = "WMI n'a pas répondu";

/// Ce qu'on dit quand une tâche ne rapporte pas ses actions.
///
/// Mesuré à 0 sur 194 : le cas est structurel, pas observé. Il ferme les trois
/// items qui dépendent des actions, et ne touche pas les trois autres.
const ACTIONS_NON_RAPPORTEES: &str =
    "le planificateur n'a pas rapporté les actions d'au moins une tâche";

/// Ce qu'on dit quand une action exécutable ne nomme pas son exécutable.
///
/// Mesuré à 0 sur 84. Publier les listes sans elle reviendrait à affirmer que
/// cette tâche lance un binaire sous le répertoire système, ce que rien n'atteste.
const EXECUTABLE_NON_RAPPORTE: &str = "une action ne nomme pas l'exécutable qu'elle lance";

/// Ce qu'on dit quand l'environnement ne nomme pas le répertoire système.
///
/// Sans lui, « hors du répertoire système » n'a pas de sens : il n'y a pas de
/// dedans. C'est le cas de toute plateforme non Windows, et d'un environnement
/// amputé de `SystemRoot`.
const RACINE_SYSTEME_INCONNUE: &str = "le répertoire système n'est pas nommé dans l'environnement";

/// Délai au-delà duquel on cesse d'attendre l'énumération des tâches.
///
/// **Quinze secondes, et non les cinq de [`crate::etat_effectif`]**, parce que
/// les deux lectures ne sont pas du même ordre. Mesuré sur la machine de
/// référence le 2026-08-24, dépôt WMI chaud :
///
/// | Classe | Lignes | Durée |
/// |---|---|---|
/// | `Win32_DeviceGuard` | 1 | 15 à 320 ms |
/// | `Win32_Service` | 303 | 275 à 846 ms |
/// | `MSFT_ScheduledTask` | 194 | 1 777 à 2 279 ms |
///
/// Le fournisseur de tâches est dix fois plus lent que celui des services pour
/// deux fois moins de lignes, et rien n'y change : ni la projection (3 champs ou
/// tous, même durée), ni le filtrage WQL (`WHERE NOT TaskPath LIKE …` rend
/// 14 lignes en 1 855 ms, et `WHERE Enabled = TRUE` est refusé). Il énumère tout,
/// puis filtre.
///
/// **La marge n'est pas une garantie, et une mesure le dit.** Sur 36 lectures
/// chronométrées, 35 tiennent entre 1,78 et 2,43 s, et **une a demandé 5,74 s** —
/// au-delà des cinq secondes du module voisin. Quinze secondes valent 7,5 fois la
/// médiane et 2,6 fois ce pire cas observé. Ce délai n'existe que pour borner une
/// attente **infinie** : la bibliothèque énumère avec `WBEM_INFINITE`, et un fil
/// Rust bloqué ne se tue pas.
///
/// Relever le délai partagé aurait été l'autre issue, et elle est mauvaise :
/// elle allonge le pire cas des deux lectures qui sont rapides aujourd'hui, pour
/// le confort de celle qui ne l'est pas.
#[cfg(windows)]
const DELAI_TACHES: std::time::Duration = std::time::Duration::from_secs(15);

/// Ce qu'une action de tâche fait, réduit à ce que les items publient.
///
/// **Volontairement pauvre.** Une action COM ne porte aucune donnée ici : son
/// CLSID est opaque pour l'utilisateur, donc impubliable (P6), et le compter ne
/// demande pas de le lire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionRelevee {
    /// L'action lance un exécutable. `None` quand le planificateur ne l'a pas
    /// nommé — ce qui n'est pas « aucun exécutable », mais « on ne sait pas ».
    Executable(Option<String>),
    /// L'action appelle un composant en processus. Aucun binaire n'est lancé.
    GestionnaireCom,
}

/// Une tâche planifiée, réduite à ce que les items publient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacheRelevee {
    /// `TaskPath` — le dossier, `\Microsoft\Windows\Bluetooth\` par exemple.
    ///
    /// Mesuré sur 194 tâches : il commence **et** finit toujours par une barre
    /// oblique inverse.
    pub dossier: String,
    /// `TaskName` — le nom seul, sans dossier.
    ///
    /// Mesuré sur 194 tâches : il ne contient jamais de `:` ni de barre oblique
    /// inverse. C'est ce qui rend la barrière
    /// `un_item_de_tache_ne_publie_quune_identite_de_tache` sans faux positif.
    pub nom: String,
    /// `State`, tel que rapporté. `None` = non rapporté, et jamais « prête ».
    pub etat: Option<u32>,
    /// Les actions de la tâche. `None` = le planificateur ne les a pas rapportées.
    pub actions: Option<Vec<ActionRelevee>>,
}

impl TacheRelevee {
    /// L'identité publiée : le dossier suivi du nom, sans séparateur ajouté.
    ///
    /// C'est aussi ce que rend `URI`, mesuré identique sur 194 tâches sur 194 —
    /// raison pour laquelle cette propriété n'est pas lue.
    #[must_use]
    pub fn identite(&self) -> String {
        format!("{}{}", self.dossier, self.nom)
    }

    /// La tâche est-elle enregistrée ailleurs que sous la racine `\Microsoft` ?
    ///
    /// La comparaison est **insensible à la casse** : sur Windows, `\microsoft\`
    /// et `\Microsoft\` désignent le même dossier du planificateur, et prétendre
    /// le contraire produirait un faux positif. Cela n'a pas été mesuré, et ne
    /// pouvait pas l'être : créer un dossier au planificateur est une écriture.
    #[must_use]
    pub fn hors_racine_microsoft(&self) -> bool {
        !commence_par(&self.dossier, RACINE_MICROSOFT)
    }

    /// Le planificateur rapporte-t-il cette tâche à l'arrêt ?
    ///
    /// Un état **non rapporté** ne l'est pas : une lacune de lecture n'est pas
    /// une désactivation, exactement comme un service absent n'est pas un service
    /// arrêté ([`crate::etat_effectif::execution_service`]).
    #[must_use]
    pub fn desactivee(&self) -> bool {
        self.etat == Some(ETAT_DESACTIVEE)
    }
}

/// Ce dont le classement d'un exécutable a besoin, et qui vient du dehors.
///
/// **Injecté plutôt que lu**, pour que le classement soit une fonction pure
/// qu'un test éprouve sans Windows, sans WMI et sans dépendre des variables
/// d'environnement de la machine qui joue les tests. C'est le geste déjà
/// appliqué à l'exécution effective des services.
pub struct Environnement<'a> {
    /// Le répertoire système développé, `C:\WINDOWS` sur la machine de référence.
    ///
    /// `None` quand l'environnement ne le nomme pas — hors Windows, notamment.
    /// Les deux items d'emplacement deviennent alors illisibles, jamais vides.
    pub racine_systeme: Option<&'a str>,
    /// Comment développer une variable d'environnement, par son nom sans `%`.
    ///
    /// Sur Windows, la recherche est insensible à la casse, et le relevé
    /// l'exige : les 84 actions exécutables écrivent `%SystemRoot%`,
    /// `%systemroot%`, `%windir%` et `%localappdata%`.
    pub variable: &'a dyn Fn(&str) -> Option<String>,
}

/// Où se trouve le binaire que lance une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emplacement {
    /// Sous le répertoire système.
    SousLaRacineSysteme,
    /// Nommé avec un répertoire, mais pas sous le répertoire système.
    HorsDeLaRacineSysteme,
    /// Nommé sans aucun répertoire. Ce qui s'exécutera est décidé par l'ordre de
    /// recherche au lancement, pas par la tâche.
    SansRepertoire,
    /// On n'a pas pu le placer. Ni l'un, ni l'autre, ni un constat.
    Indeterminable,
}

/// Développe les variables d'environnement d'une valeur, sans en inventer.
///
/// Une variable que le résolveur ne connaît pas est **laissée telle quelle**,
/// `%` compris, comme le fait `ExpandEnvironmentStrings` : la remplacer par du
/// vide fabriquerait un chemin qui n'a jamais existé.
///
/// # Pourquoi cette fonction n'est pas une commodité
///
/// Sans elle, le classement se trompe sur la majorité du relevé. Mesuré le
/// 2026-08-24 : **69 des 84** actions exécutables écrivent une variable, et la
/// même règle appliquée à la lettre classe **80** tâches hors du répertoire
/// système au lieu de 16. Le piège reste ouvert d'un cran : `%localappdata%` se
/// développe contre le compte **qui scanne**, pas contre le principal de la
/// tâche. Non observé sur une machine à un seul utilisateur interactif, mais
/// structurel, donc écrit ici.
#[must_use]
pub fn developper(valeur: &str, variable: &dyn Fn(&str) -> Option<String>) -> String {
    let mut sortie = String::with_capacity(valeur.len());
    let mut reste = valeur;
    while let Some(ouverture) = reste.find('%') {
        sortie.push_str(&reste[..ouverture]);
        let apres = &reste[ouverture + 1..];
        let Some(fermeture) = apres.find('%') else {
            // Un `%` esseulé n'ouvre rien : il appartient au chemin.
            sortie.push('%');
            sortie.push_str(apres);
            return sortie;
        };
        let nom = &apres[..fermeture];
        match variable(nom) {
            Some(valeur) => sortie.push_str(&valeur),
            None => {
                sortie.push('%');
                sortie.push_str(nom);
                sortie.push('%');
            }
        }
        reste = &apres[fermeture + 1..];
    }
    sortie.push_str(reste);
    sortie
}

/// Retire les guillemets qui encadrent un exécutable, et les espaces de bord.
///
/// Mesuré : 4 des 84 valeurs `Execute` sont encadrées de guillemets. Sans ce
/// retrait, `"%SystemRoot%\…"` ne commencerait pas par le répertoire système et
/// se classerait dehors — un faux constat pour un caractère.
fn commande_nue(execute: &str) -> &str {
    let sans_bord = execute.trim();
    sans_bord
        .strip_prefix('"')
        .and_then(|reste| reste.strip_suffix('"'))
        .unwrap_or(sans_bord)
        .trim()
}

/// Un chemin en commence-t-il un autre, à la façon de Windows ?
///
/// Insensible à la casse **ASCII**, et sur les seuls caractères ASCII : c'est
/// une approximation de la comparaison de Windows, suffisante pour des noms de
/// dossiers système et pour `\Microsoft\`, et volontairement pauvre plutôt que
/// faussement savante.
fn commence_par(chaine: &str, prefixe: &str) -> bool {
    chaine.len() >= prefixe.len()
        && chaine
            .as_bytes()
            .iter()
            .zip(prefixe.as_bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// Ramène les barres obliques d'un chemin à celles de Windows.
fn normaliser(chemin: &str) -> String {
    chemin.replace('/', r"\")
}

/// Ce chemin est-il **sous** ce répertoire, et non simplement préfixé par lui ?
///
/// La frontière compte : `C:\WINDOWSXYZ\quelquechose.exe` commence par
/// `C:\WINDOWS` sans être dedans. Sans le contrôle de séparateur, un répertoire
/// voisin passerait pour le répertoire système, ce qui est exactement l'erreur
/// qu'un adversaire fabriquerait.
fn sous_le_repertoire(chemin: &str, repertoire: &str) -> bool {
    let chemin = normaliser(chemin);
    let repertoire = normaliser(repertoire);
    let repertoire = repertoire.trim_end_matches('\\');
    if repertoire.is_empty() {
        return false;
    }
    if !commence_par(&chemin, repertoire) {
        return false;
    }
    match chemin.as_bytes().get(repertoire.len()) {
        None => true,
        Some(b'\\') => true,
        Some(_) => false,
    }
}

/// Où se trouve le binaire d'une action, après développement des variables.
///
/// **Pure, et c'est ce qui la rend éprouvable** : elle ne lit ni WMI, ni le
/// registre, ni l'environnement du processus. Tout ce dont elle a besoin arrive
/// par [`Environnement`].
#[must_use]
pub fn emplacement(execute: Option<&str>, env: &Environnement<'_>) -> Emplacement {
    let Some(execute) = execute else {
        return Emplacement::Indeterminable;
    };
    let developpe = developper(commande_nue(execute), env.variable);
    let developpe = commande_nue(&developpe);
    if developpe.is_empty() {
        return Emplacement::Indeterminable;
    }
    if !developpe.contains('\\') && !developpe.contains('/') {
        return Emplacement::SansRepertoire;
    }
    let Some(racine) = env.racine_systeme else {
        // Sans dedans, il n'y a pas de dehors. On ne classe pas au hasard.
        return Emplacement::Indeterminable;
    };
    if sous_le_repertoire(developpe, racine) {
        Emplacement::SousLaRacineSysteme
    } else {
        Emplacement::HorsDeLaRacineSysteme
    }
}

/// Collecteur des tâches planifiées.
///
/// **Ne déclare aucune écriture, et sa signature le porte** : [`crate::Collector`]
/// prend `&self` et ne reçoit aucun canal d'effet.
pub struct TachesCollector;

impl TachesCollector {
    /// Identifiant, pour le journal.
    #[must_use]
    pub const fn nom() -> &'static str {
        "taches"
    }

    /// Les tâches planifiées déclarées, ou l'aveu qu'on n'a pas su les lire.
    ///
    /// **Trois états, jamais deux.** Un `Vec` vide ne saurait pas dire lequel des
    /// deux il décrit, et c'est la confusion que ce dépôt a déjà payée sur les
    /// exclusions de Defender, où « 0 élément » s'affichait sur une clé jamais
    /// ouverte.
    #[must_use]
    pub fn taches() -> Lecture<Vec<TacheRelevee>> {
        #[cfg(windows)]
        {
            windows_impl::taches()
        }
        #[cfg(not(windows))]
        {
            // Le planificateur de Windows n'existe pas ici. La classe n'est pas
            // refusée, elle n'a aucune raison d'exister : c'est une absence, et
            // surtout pas un zéro, qui prétendrait qu'on a compté.
            Lecture::Absente
        }
    }

    /// Les items du domaine D2 produits par ce collecteur.
    #[must_use]
    pub fn items() -> Vec<Item> {
        let racine = std::env::var("SystemRoot").ok();
        let variable = |nom: &str| std::env::var(nom).ok();
        let env = Environnement {
            racine_systeme: racine.as_deref(),
            variable: &variable,
        };
        items_depuis(&Self::taches(), &env)
    }
}

/// Ce que les listes d'emplacement ont pu établir, ou pourquoi elles n'ont pas pu.
///
/// Un type plutôt que deux `Vec` et un booléen : les trois cas s'excluent, et
/// les rendre inconstructibles ensemble vaut mieux que les tenir cohérents à la
/// main.
enum Emplacements {
    /// Les deux listes, chacune triée.
    Etablis {
        /// Les tâches dont au moins un binaire est hors du répertoire système.
        hors_racine: Vec<String>,
        /// Les tâches dont au moins un binaire est nommé sans répertoire.
        sans_repertoire: Vec<String>,
    },
    /// On n'a pas su placer au moins un binaire, et voici pourquoi.
    Indeterminables(&'static str),
}

/// Classe les tâches par l'emplacement de leurs binaires. **Pure.**
fn emplacements(taches: &[TacheRelevee], env: &Environnement<'_>) -> Emplacements {
    let mut hors_racine = Vec::new();
    let mut sans_repertoire = Vec::new();

    for tache in taches {
        let Some(actions) = &tache.actions else {
            return Emplacements::Indeterminables(ACTIONS_NON_RAPPORTEES);
        };
        for action in actions {
            let ActionRelevee::Executable(execute) = action else {
                continue;
            };
            match emplacement(execute.as_deref(), env) {
                Emplacement::SousLaRacineSysteme => {}
                Emplacement::HorsDeLaRacineSysteme => hors_racine.push(tache.identite()),
                Emplacement::SansRepertoire => sans_repertoire.push(tache.identite()),
                // Une tâche qu'on ne sait pas placer rend les deux listes
                // incomplètes. Les publier quand même reviendrait à affirmer
                // qu'elle lance un binaire sous le répertoire système.
                Emplacement::Indeterminable => {
                    return Emplacements::Indeterminables(if env.racine_systeme.is_none() {
                        RACINE_SYSTEME_INCONNUE
                    } else {
                        EXECUTABLE_NON_RAPPORTE
                    });
                }
            }
        }
    }

    // Une tâche à plusieurs actions du même bord ne se compte qu'une fois : la
    // liste porte des tâches, pas des actions.
    trier(&mut hors_racine);
    trier(&mut sans_repertoire);
    Emplacements::Etablis {
        hors_racine,
        sans_repertoire,
    }
}

/// Trie et déduplique une liste d'identités.
///
/// **Le tri n'est pas cosmétique.** WMI ne contractualise aucun ordre
/// d'énumération, et le magasin d'observations compare des valeurs : deux
/// relevés identiques rendus dans un ordre différent ouvriraient un intervalle
/// à chaque scan, sur une machine où rien n'a bougé.
fn trier(identites: &mut Vec<String>) {
    identites.sort_unstable();
    identites.dedup();
}

/// Traduit une lecture en items. **Pure** : c'est ce qui la rend éprouvable.
///
/// Séparée de [`TachesCollector::items`] pour que les cas qui comptent — le
/// refus, l'absence, l'exécutable non rapporté, l'action non rapportée —
/// s'éprouvent sur toute plateforme, sans WMI et sans machine Windows.
fn items_depuis(lecture: &Lecture<Vec<TacheRelevee>>, env: &Environnement<'_>) -> Vec<Item> {
    let maintenant = chrono::Utc::now();
    let mut items = Vec::new();
    let mut item = |chemin: &str, nature: Nature, valeur: ItemValue, but: &str, risque: &str| {
        items.push(Item {
            path: chemin.to_owned(),
            domain: Domain::Configuration,
            nature,
            desired: None,
            observed: valeur,
            observed_at: maintenant,
            // `Observed`, jamais `Keystone` : on relève, on ne produit pas.
            provenance: Provenance::Observed,
            purpose: but.to_owned(),
            risk: risque.to_owned(),
            reference: None,
        });
    };

    // Ce que chaque famille de valeurs vaut selon l'état de la lecture. Une
    // absence et un refus ne se ressemblent pas, et aucun des deux ne devient
    // `Int(0)` ni `List([])`.
    let scalaire = |compte: Option<usize>| match (lecture, compte) {
        (Lecture::Trouvee(_), Some(n)) => ItemValue::Int(i64::try_from(n).unwrap_or(i64::MAX)),
        (Lecture::Trouvee(_) | Lecture::Refusee, _) => ItemValue::illisible(REFUS_WMI),
        (Lecture::Absente, _) => ItemValue::Absent,
    };
    let liste = |identites: Option<&[String]>, raison: &'static str| match (lecture, identites) {
        (Lecture::Trouvee(_), Some(l)) => ItemValue::List(l.to_vec()),
        (Lecture::Trouvee(_), None) => ItemValue::illisible(raison),
        (Lecture::Refusee, _) => ItemValue::illisible(REFUS_WMI),
        (Lecture::Absente, _) => ItemValue::Absent,
    };

    let taches = match lecture {
        Lecture::Trouvee(t) => t.as_slice(),
        Lecture::Absente | Lecture::Refusee => &[],
    };

    let mut hors_microsoft: Vec<String> = taches
        .iter()
        .filter(|t| t.hors_racine_microsoft())
        .map(TacheRelevee::identite)
        .collect();
    trier(&mut hors_microsoft);
    let mut arretees: Vec<String> = taches
        .iter()
        .filter(|t| t.hors_racine_microsoft() && t.desactivee())
        .map(TacheRelevee::identite)
        .collect();
    trier(&mut arretees);

    let places = emplacements(taches, env);
    let com = taches
        .iter()
        .map(|t| t.actions.as_ref())
        .try_fold(0_usize, |compte, actions| {
            let actions = actions?;
            Some(compte + usize::from(actions.contains(&ActionRelevee::GestionnaireCom)))
        });

    item(
        "configuration.tasks.total",
        // Un décompte d'une population qui bouge seule : chaque mise à jour de
        // Windows en ajoute et en retire, sans que personne l'ait voulu de ce
        // côté-ci. Exactement `inventory.software.total`.
        Nature::Mesure,
        scalaire(Some(taches.len())),
        "Nombre de tâches planifiées enregistrées sur la machine, toutes racines \
         confondues. C'est le dénominateur des quatre listes : sans lui, « 14 » \
         et « 16 » ne se lisent pas.",
        "Aucun — cet item est un constat.",
    );

    item(
        "configuration.tasks.outside_microsoft_root",
        // Constat : une tâche enregistrée est un fait de la machine. Aucun verbe
        // n'écrira une tâche — ce serait `RegisterByXml`, refusé définitivement —
        // donc rien ici ne se déclare ni ne converge.
        Nature::Constat,
        liste(Some(&hors_microsoft), REFUS_WMI),
        "Tâches enregistrées ailleurs que sous la racine « \\Microsoft » du \
         planificateur, quel que soit leur état. « \\Microsoft » est une \
         convention d'emplacement et pas une attestation d'auteur : une \
         fonctionnalité de Microsoft peut vivre en dehors, et rien n'interdit \
         d'enregistrer en dedans. Cet item nomme l'endroit où l'on a regardé. \
         L'identité est publiée telle que Windows l'a enregistrée, identifiant \
         de compte compris.",
        "Aucun, c'est un relevé. Une entrée qui apparaît est un fait daté ; ce \
         qu'il faut en conclure ne se lit pas ici.",
    );

    item(
        "configuration.tasks.disabled_outside_microsoft_root",
        Nature::Constat,
        liste(Some(&arretees), REFUS_WMI),
        "Parmi les tâches ci-dessus, celles que le planificateur rapporte à \
         l'arrêt. Elles restent enregistrées, et la liste précédente les porte \
         aussi : une désactivation se lit ainsi comme un changement, jamais \
         comme une disparition.",
        "Aucun, c'est un relevé. Une tâche à l'arrêt se réactive sans être \
         réenregistrée.",
    );

    let (hors_racine, sans_repertoire, raison) = match &places {
        Emplacements::Etablis {
            hors_racine,
            sans_repertoire,
        } => (
            Some(hors_racine.as_slice()),
            Some(sans_repertoire.as_slice()),
            REFUS_WMI,
        ),
        Emplacements::Indeterminables(raison) => (None, None, *raison),
    };

    item(
        "configuration.tasks.executables_outside_system_root",
        Nature::Constat,
        liste(hors_racine, raison),
        "Tâches dont au moins une action lance un binaire situé hors du \
         répertoire système. Une tâche lance un binaire, et l'endroit où ce \
         binaire est écrit décide de qui peut le remplacer. Cette liste dit où \
         regarder, et le chemin du binaire n'y figure pas : l'item nomme la \
         tâche.",
        "Aucun, c'est un relevé. Aucune inscriptibilité n'est évaluée ici — elle \
         se calcule, et un calcul faux deviendrait une accusation.",
    );

    item(
        "configuration.tasks.executables_without_directory",
        Nature::Constat,
        liste(sans_repertoire, raison),
        "Tâches qui nomment leur exécutable sans aucun répertoire. Ce qui \
         s'exécutera est alors décidé par l'ordre de recherche au lancement, et \
         non par la tâche. C'est un fait mécanique, sans jugement : c'est le \
         mouvement de ce nombre qui informe.",
        "Aucun, c'est un relevé.",
    );

    item(
        "configuration.tasks.com_handler",
        // Un décompte, comme le total, et pour la même raison.
        Nature::Mesure,
        scalaire(com),
        "Nombre de tâches dont au moins une action passe par un gestionnaire \
         COM, donc sans lancer de binaire. C'est la part de la population dont \
         les deux listes d'exécutables ne disent rien. Les identifiants de ces \
         composants ne sont pas publiés : un CLSID ne se lit pas.",
        "Aucun — cet item est un constat.",
    );

    items
}

#[cfg(windows)]
mod windows_impl {
    use super::{ActionRelevee, TacheRelevee};
    use crate::posture::Lecture;
    use serde::Deserialize;

    /// Là où le planificateur publie ses tâches.
    const ESPACE_DE_NOMS: &str = r"root\Microsoft\Windows\TaskScheduler";

    /// Une action qui lance un exécutable.
    ///
    /// **Un seul champ.** Chacun est demandé par `get_property` sur chaque objet
    /// imbriqué ; `Arguments` et `WorkingDirectory` seraient donc deux lectures
    /// de plus par action pour des valeurs qu'aucun item ne publie.
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ActionExecutable {
        execute: Option<String>,
    }

    /// Une action qui appelle un composant en processus.
    ///
    /// **Aucun champ, et c'est le propos.** Le désérialiseur demande exactement
    /// les champs déclarés : n'en déclarer aucun signifie qu'aucune propriété
    /// d'un gestionnaire COM n'est lue, CLSID compris.
    #[derive(Deserialize)]
    struct GestionnaireCom {}

    /// Les classes concrètes d'action du schéma, et rien d'autre.
    ///
    /// **La balise est externe et les variantes portent les noms des classes
    /// WMI** : c'est ce qui fait qu'une action COM ne se voit jamais demander
    /// `Execute`. Voir la section « La difficulté réelle » en tête de module.
    #[derive(Deserialize)]
    enum ActionLue {
        /// `MSFT_TaskExecAction` — 84 actions mesurées sur la machine de référence.
        #[serde(rename = "MSFT_TaskExecAction")]
        Executable(ActionExecutable),
        /// `MSFT_TaskComHandlerAction` — 112 tâches mesurées.
        #[serde(rename = "MSFT_TaskComHandlerAction")]
        Com(GestionnaireCom),
    }

    /// Projection de `MSFT_ScheduledTask`.
    ///
    /// Le `rename` n'est pas décoratif : la bibliothèque construit
    /// `SELECT TaskName,TaskPath,State,Actions FROM MSFT_ScheduledTask` à partir
    /// du nom et des champs de cette structure.
    #[derive(Deserialize)]
    #[serde(rename = "MSFT_ScheduledTask", rename_all = "PascalCase")]
    struct TacheLue {
        task_name: Option<String>,
        task_path: Option<String>,
        state: Option<u32>,
        actions: Option<Vec<ActionLue>>,
    }

    /// Lit les tâches sur un fil dédié — voir [`crate::etat_effectif::sur_un_fil_dedie`],
    /// qui porte le raisonnement sur COM, sur le délai, et sur ce que le repli
    /// ne couvre pas.
    pub(super) fn taches() -> Lecture<Vec<TacheRelevee>> {
        crate::etat_effectif::sur_un_fil_dedie(super::DELAI_TACHES, interroger, || Lecture::Refusee)
    }

    fn interroger() -> Lecture<Vec<TacheRelevee>> {
        use wmi::{COMLibrary, WMIConnection};

        // Un échec ici est un refus de lecture, jamais un constat : c'est la
        // doctrine du module voisin, et elle vaut d'autant plus ici qu'une liste
        // vide affirmerait qu'aucune tâche n'est enregistrée hors de la racine
        // de l'éditeur.
        let Ok(com) = COMLibrary::new() else {
            return Lecture::Refusee;
        };
        let Ok(connexion) = WMIConnection::with_namespace_path(ESPACE_DE_NOMS, com) else {
            return Lecture::Refusee;
        };
        let Ok(lignes) = connexion.query::<TacheLue>() else {
            return Lecture::Refusee;
        };

        let mut relevees = Vec::with_capacity(lignes.len());
        for ligne in lignes {
            // Une tâche sans dossier ni nom ne peut être rapprochée d'aucune
            // racine : la garder fausserait les listes, l'écarter fausserait le
            // total. On refuse l'ensemble plutôt que de publier un décompte dont
            // on sait qu'il manque quelqu'un — c'est le geste du collecteur de
            // virtualisation sur une sous-clé illisible, mot pour mot.
            let (Some(dossier), Some(nom)) = (ligne.task_path, ligne.task_name) else {
                return Lecture::Refusee;
            };
            relevees.push(TacheRelevee {
                dossier,
                nom,
                etat: ligne.state,
                actions: ligne.actions.map(|actions| {
                    actions
                        .into_iter()
                        .map(|action| match action {
                            ActionLue::Executable(e) => ActionRelevee::Executable(e.execute),
                            ActionLue::Com(GestionnaireCom {}) => ActionRelevee::GestionnaireCom,
                        })
                        .collect()
                }),
            });
        }
        Lecture::Trouvee(relevees)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le répertoire système de la machine de référence, tel que mesuré.
    const RACINE: &str = r"C:\WINDOWS";

    /// Les variables réellement rencontrées dans les 84 actions exécutables.
    ///
    /// La casse est celle que le planificateur rapporte — `%SystemRoot%` et
    /// `%systemroot%` coexistent dans le même relevé —, et la table est donc
    /// interrogée sans égard à la casse, comme le fait Windows.
    fn variable(nom: &str) -> Option<String> {
        const TABLE: &[(&str, &str)] = &[
            ("SystemRoot", RACINE),
            ("windir", RACINE),
            ("ProgramFiles", r"C:\Program Files"),
            ("localappdata", r"C:\Users\essai\AppData\Local"),
        ];
        TABLE
            .iter()
            .find(|(clef, _)| clef.eq_ignore_ascii_case(nom))
            .map(|(_, valeur)| (*valeur).to_owned())
    }

    fn env() -> Environnement<'static> {
        Environnement {
            racine_systeme: Some(RACINE),
            variable: &variable,
        }
    }

    fn tache(dossier: &str, nom: &str, actions: &[ActionRelevee]) -> TacheRelevee {
        TacheRelevee {
            dossier: dossier.to_owned(),
            nom: nom.to_owned(),
            etat: Some(3),
            actions: Some(actions.to_vec()),
        }
    }

    fn lance(commande: &str) -> ActionRelevee {
        ActionRelevee::Executable(Some(commande.to_owned()))
    }

    /// Un relevé fidèle à la machine de référence, réduit à ce qui s'éprouve.
    ///
    /// Les valeurs `Execute` sont **recopiées du relevé du 2026-08-24**, casse et
    /// guillemets compris : ce sont elles qui rendent les barrières non triviales.
    fn releve_de_reference() -> Vec<TacheRelevee> {
        vec![
            // Sous la racine de l'éditeur, binaire sous le répertoire système.
            tache(
                r"\Microsoft\Windows\Chkdsk\",
                "ProactiveScan",
                &[lance(r"%SystemRoot%\System32\drvinst.exe")],
            ),
            // Même racine, casse minuscule de la variable.
            tache(
                r"\Microsoft\Windows\Clip\",
                "License Validation",
                &[lance(r"%systemroot%\system32\ClipRenew.exe")],
            ),
            // Sous la racine de l'éditeur, et pourtant hors du répertoire
            // système : les quatre tâches de Defender sont exactement ce cas.
            tache(
                r"\Microsoft\Windows\Windows Defender\",
                "Windows Defender Cleanup",
                &[lance(
                    r"C:\ProgramData\Microsoft\Windows Defender\Platform\MpCmdRun.exe",
                )],
            ),
            // Encadré de guillemets, et derrière une variable.
            tache(
                r"\Microsoft\Windows\Windows Media Sharing\",
                "UpdateLibrary",
                &[lance(
                    r#""%ProgramFiles%\Windows Media Player\wmpnscfg.exe""#,
                )],
            ),
            // Sans aucun répertoire.
            tache(
                r"\Microsoft\Windows\Bluetooth\",
                "UninstallDeviceTask",
                &[lance("BthUdTask.exe")],
            ),
            // Hors racine de l'éditeur, avec l'identifiant du compte.
            tache(
                r"\",
                "OneDrive Startup Task-S-1-5-21-707948682-1113435736-4095543961-1001",
                &[lance(
                    r"%localappdata%\Microsoft\OneDrive\OneDriveStandaloneUpdater.exe",
                )],
            ),
            // Hors racine de l'éditeur, et à l'arrêt.
            TacheRelevee {
                dossier: r"\Lenovo\UDC\".to_owned(),
                nom: "Lenovo UDC Diagnostic Scan".to_owned(),
                etat: Some(ETAT_DESACTIVEE),
                actions: Some(vec![lance(r"C:\Program Files\Lenovo\udc.exe")]),
            },
            // Un gestionnaire COM : aucun binaire, donc aucune des deux listes.
            tache(
                r"\Microsoft\Windows\WCM\",
                "WiFiTask",
                &[ActionRelevee::GestionnaireCom],
            ),
        ]
    }

    fn valeur(items: &[Item], suffixe: &str) -> ItemValue {
        let chemin = format!("configuration.tasks.{suffixe}");
        items
            .iter()
            .find(|i| i.path == chemin)
            .unwrap_or_else(|| panic!("« {chemin} » n'est pas produit"))
            .observed
            .clone()
    }

    #[test]
    fn le_releve_de_reference_se_traduit_en_six_items() {
        let items = items_depuis(&Lecture::Trouvee(releve_de_reference()), &env());
        assert_eq!(items.len(), 6, "six items, et rien de plus");
        assert_eq!(valeur(&items, "total"), ItemValue::Int(8));
        assert_eq!(valeur(&items, "com_handler"), ItemValue::Int(1));
        assert_eq!(
            valeur(&items, "outside_microsoft_root"),
            ItemValue::List(vec![
                r"\Lenovo\UDC\Lenovo UDC Diagnostic Scan".to_owned(),
                r"\OneDrive Startup Task-S-1-5-21-707948682-1113435736-4095543961-1001".to_owned(),
            ]),
            "les identités sont triées, sans quoi un ordre d'énumération \
             différent ouvrirait un intervalle dans le magasin"
        );
        assert_eq!(
            valeur(&items, "disabled_outside_microsoft_root"),
            ItemValue::List(vec![r"\Lenovo\UDC\Lenovo UDC Diagnostic Scan".to_owned()])
        );
        assert_eq!(
            valeur(&items, "executables_outside_system_root"),
            ItemValue::List(vec![
                r"\Lenovo\UDC\Lenovo UDC Diagnostic Scan".to_owned(),
                r"\Microsoft\Windows\Windows Defender\Windows Defender Cleanup".to_owned(),
                r"\Microsoft\Windows\Windows Media Sharing\UpdateLibrary".to_owned(),
                r"\OneDrive Startup Task-S-1-5-21-707948682-1113435736-4095543961-1001".to_owned(),
            ]),
            "une tâche sous « \\Microsoft\\ » peut lancer un binaire du dehors : \
             les quatre tâches de Defender sont exactement ce cas"
        );
        assert_eq!(
            valeur(&items, "executables_without_directory"),
            ItemValue::List(vec![
                r"\Microsoft\Windows\Bluetooth\UninstallDeviceTask".to_owned()
            ])
        );
    }

    /// **La barrière du développement des variables** (B6).
    ///
    /// Éprouvée par falsification sur la machine réelle : la même règle appliquée
    /// à la lettre, sans développer `%windir%`, `%SystemRoot%`, `%ProgramFiles%`
    /// ni `%localappdata%`, classe **80** tâches hors du répertoire système au
    /// lieu de 16 — 69 des 84 actions exécutables portant une variable. Ici, le
    /// relevé de référence rejoue le même défaut en petit.
    #[test]
    fn le_classement_dun_binaire_developpe_les_variables_denvironnement() {
        // Ce que le développement change, valeur par valeur.
        for (commande, attendu) in [
            (
                r"%SystemRoot%\System32\drvinst.exe",
                Emplacement::SousLaRacineSysteme,
            ),
            (
                r"%systemroot%\system32\ClipRenew.exe",
                Emplacement::SousLaRacineSysteme,
            ),
            (
                r"%windir%\System32\XblGameSaveTask.exe",
                Emplacement::SousLaRacineSysteme,
            ),
            (
                r#""%ProgramFiles%\Windows Media Player\wmpnscfg.exe""#,
                Emplacement::HorsDeLaRacineSysteme,
            ),
            (
                r"%localappdata%\Microsoft\OneDrive\OneDriveStandaloneUpdater.exe",
                Emplacement::HorsDeLaRacineSysteme,
            ),
        ] {
            assert_eq!(
                emplacement(Some(commande), &env()),
                attendu,
                "« {commande} » mal placé"
            );
        }

        // Et sans développement, trois de ces cinq basculent : c'est le défaut
        // que la barrière interdit, reproduit ici avec un résolveur muet.
        let muet = |_: &str| None;
        let aveugle = Environnement {
            racine_systeme: Some(RACINE),
            variable: &muet,
        };
        assert_eq!(
            emplacement(Some(r"%SystemRoot%\System32\drvinst.exe"), &aveugle),
            Emplacement::HorsDeLaRacineSysteme,
            "sans développement, un binaire du répertoire système passe dehors"
        );

        // Une variable inconnue reste littérale : la remplacer par du vide
        // fabriquerait « \System32\drvinst.exe », un chemin qui n'existe pas.
        assert_eq!(developper(r"%Inconnue%\a.exe", &muet), r"%Inconnue%\a.exe");
        // Un pour-cent esseulé appartient au chemin.
        assert_eq!(developper(r"C:\100%\a.exe", &variable), r"C:\100%\a.exe");
        assert_eq!(developper("", &variable), "");
    }

    /// **La barrière de la frontière de répertoire.**
    ///
    /// `C:\WINDOWSXYZ` commence par `C:\WINDOWS` sans être dedans. Sans le
    /// contrôle de séparateur, un répertoire voisin passerait pour le répertoire
    /// système — l'erreur exacte qu'un adversaire fabriquerait.
    #[test]
    fn un_repertoire_voisin_ne_passe_pas_pour_le_repertoire_systeme() {
        for dedans in [
            r"C:\WINDOWS\System32\a.exe",
            r"C:\windows\system32\a.exe",
            r"c:/WINDOWS/System32/a.exe",
            r"C:\WINDOWS",
        ] {
            assert!(
                sous_le_repertoire(dedans, RACINE),
                "« {dedans} » devrait être dedans"
            );
        }
        for dehors in [
            r"C:\WINDOWSXYZ\a.exe",
            r"C:\WINDOWS.old\a.exe",
            r"D:\WINDOWS\System32\a.exe",
            r"C:\Program Files\a.exe",
            "",
        ] {
            assert!(
                !sous_le_repertoire(dehors, RACINE),
                "« {dehors} » devrait être dehors"
            );
        }
        // Une racine vide ne met rien dedans : sans dedans, pas de dehors.
        assert!(!sous_le_repertoire(r"C:\WINDOWS\a.exe", ""));
    }

    /// **La barrière de l'identité** (B2).
    ///
    /// Un item de tâche ne publie qu'une identité de tâche. Invariant vérifié
    /// sur les 194 tâches de la machine de référence : tout `TaskPath` commence
    /// et finit par une barre oblique inverse, et **aucun** `TaskName` ne
    /// contient `:` ni de barre oblique inverse. Un chemin de binaire, une ligne
    /// de commande ou un SDDL échouent donc à l'un des deux contrôles.
    ///
    /// Éprouvée par falsification : remplacer `tache.identite()` par la commande
    /// dans [`emplacements`] fait échouer ce test en nommant la valeur fautive.
    #[test]
    fn un_item_de_tache_ne_publie_quune_identite_de_tache() {
        // Un relevé volontairement hostile : chaque valeur qu'on refuse de
        // publier est présente dans le relevé, à sa place légitime.
        let hostile = vec![
            tache(
                r"\Piege\",
                "Fuite",
                &[lance(
                    r"C:\Users\essai\AppData\Local\secret.exe --jeton=abc",
                )],
            ),
            tache(r"\Piege\", "Sddl", &[lance("D:AI(A;;FA;;;BA)(A;;FA;;;SY)")]),
        ];
        let items = items_depuis(&Lecture::Trouvee(hostile), &env());

        let mut controlees = 0;
        for item in &items {
            let ItemValue::List(entrees) = &item.observed else {
                continue;
            };
            for entree in entrees {
                controlees += 1;
                assert!(
                    entree.starts_with('\\'),
                    "« {} » publie « {entree} », qui n'est pas une identité de tâche",
                    item.path
                );
                assert!(
                    !entree.contains(':'),
                    "« {} » publie « {entree} » : ni chemin de binaire, ni SDDL",
                    item.path
                );
            }
        }
        assert!(
            controlees >= 3,
            "seulement {controlees} entrées contrôlées : la barrière ne barre plus rien"
        );

        // Et ce qu'on refuse ne se retrouve nulle part, pas même dans un texte.
        let tout = format!("{items:?}");
        for interdit in ["secret.exe", "--jeton=abc", "D:AI(A;;FA"] {
            assert!(
                !tout.contains(interdit),
                "« {interdit} » a fui dans les items"
            );
        }
    }

    /// **La barrière du gestionnaire COM** (B3).
    ///
    /// Les 112 actions COM de la machine de référence ne sont pas des lectures
    /// refusées : ce sont des actions qui ne lancent aucun binaire. Les confondre
    /// publierait 112 illisibles, ou un décompte faux.
    ///
    /// Éprouvée par falsification, et sur la machine réelle plutôt qu'en
    /// laboratoire : déclarer `Execute` dans une structure imbriquée plutôt que
    /// dans une variante d'énumération fait répondre `0x80041002` à WMI et rend
    /// **zéro** tâche au lieu de 194.
    #[test]
    fn un_gestionnaire_com_nest_pas_une_lecture_refusee() {
        let que_du_com: Vec<TacheRelevee> = (0..112)
            .map(|n| {
                tache(
                    r"\Microsoft\Windows\WCM\",
                    &format!("Handler{n}"),
                    &[ActionRelevee::GestionnaireCom],
                )
            })
            .collect();
        let items = items_depuis(&Lecture::Trouvee(que_du_com), &env());

        assert_eq!(valeur(&items, "total"), ItemValue::Int(112));
        assert_eq!(valeur(&items, "com_handler"), ItemValue::Int(112));
        // Lues et vides : on a regardé, aucun binaire n'est en cause. Ce n'est
        // pas un refus, et une liste vide est ici le constat exact.
        assert_eq!(
            valeur(&items, "executables_outside_system_root"),
            ItemValue::List(vec![])
        );
        assert_eq!(
            valeur(&items, "executables_without_directory"),
            ItemValue::List(vec![])
        );
        for suffixe in [
            "executables_outside_system_root",
            "executables_without_directory",
        ] {
            assert!(
                valeur(&items, suffixe).est_constat(),
                "« {suffixe} » : une action COM n'est pas une lacune de lecture"
            );
        }
    }

    /// **La barrière du refus** (B5).
    ///
    /// La cicatrice « 0 exclusion sur une clé jamais ouverte », transposée. Un
    /// refus ne publie ni zéro, ni liste vide ; une absence de planificateur ne
    /// publie pas davantage un zéro.
    #[test]
    fn une_lecture_refusee_ne_publie_ni_zero_ni_liste_vide() {
        let refus = items_depuis(&Lecture::Refusee, &env());
        assert_eq!(refus.len(), 6);
        for item in &refus {
            assert!(
                !item.observed.est_constat(),
                "« {} » vaut {} après un refus",
                item.path,
                item.observed
            );
            assert_ne!(item.observed, ItemValue::Int(0));
            assert_ne!(item.observed, ItemValue::List(vec![]));
            assert_ne!(item.observed, ItemValue::Absent);
        }

        // Hors Windows, le planificateur n'existe pas. C'est une absence, et
        // toujours pas un zéro : « aucune tâche » est un constat qu'on n'a pas
        // fait.
        let absence = items_depuis(&Lecture::Absente, &env());
        assert_eq!(absence.len(), 6);
        for item in &absence {
            assert_eq!(item.observed, ItemValue::Absent, "« {} »", item.path);
            assert_ne!(item.observed, ItemValue::Int(0));
        }
    }

    /// Une lacune sur une action ferme les items qui en dépendent, et eux seuls.
    ///
    /// Publier les listes sans la tâche qu'on n'a pas su placer reviendrait à
    /// affirmer que son binaire est sous le répertoire système — ce que rien
    /// n'atteste. Le total et les deux listes d'emplacement, eux, restent lus.
    #[test]
    fn un_executable_non_rapporte_nillisibilise_que_les_listes_demplacement() {
        let mut releve = releve_de_reference();
        releve.push(tache(
            r"\Piege\",
            "SansExecutable",
            &[ActionRelevee::Executable(None)],
        ));
        let items = items_depuis(&Lecture::Trouvee(releve), &env());

        assert_eq!(valeur(&items, "total"), ItemValue::Int(9));
        assert!(valeur(&items, "outside_microsoft_root").est_constat());
        assert!(valeur(&items, "com_handler").est_constat());
        for suffixe in [
            "executables_outside_system_root",
            "executables_without_directory",
        ] {
            assert_eq!(
                valeur(&items, suffixe),
                ItemValue::illisible(EXECUTABLE_NON_RAPPORTE),
                "« {suffixe} » : une tâche qu'on ne sait pas placer rend la liste \
                 incomplète, et une liste incomplète se dit"
            );
        }

        // Une tâche dont les actions ne sont pas rapportées ferme en plus le
        // décompte des gestionnaires COM : on ne sait pas ce qu'elle contient.
        let mut sans_actions = releve_de_reference();
        sans_actions.push(TacheRelevee {
            dossier: r"\Piege\".to_owned(),
            nom: "SansActions".to_owned(),
            etat: Some(3),
            actions: None,
        });
        let items = items_depuis(&Lecture::Trouvee(sans_actions), &env());
        assert_eq!(valeur(&items, "total"), ItemValue::Int(9));
        assert_eq!(
            valeur(&items, "com_handler"),
            ItemValue::illisible(REFUS_WMI)
        );
        assert_eq!(
            valeur(&items, "executables_outside_system_root"),
            ItemValue::illisible(ACTIONS_NON_RAPPORTEES)
        );
    }

    /// Sans répertoire système nommé, on ne place rien — et on ne l'invente pas.
    #[test]
    fn sans_racine_systeme_les_emplacements_sont_illisibles_et_non_vides() {
        let sans_racine = Environnement {
            racine_systeme: None,
            variable: &variable,
        };
        let items = items_depuis(&Lecture::Trouvee(releve_de_reference()), &sans_racine);

        assert!(valeur(&items, "total").est_constat());
        assert!(valeur(&items, "outside_microsoft_root").est_constat());
        assert_eq!(
            valeur(&items, "executables_outside_system_root"),
            ItemValue::illisible(RACINE_SYSTEME_INCONNUE)
        );
        // Celui-là ne dépend d'aucune racine, mais il ne peut plus se dire
        // complet : la tâche qu'on n'a pas placée aurait pu en faire partie.
        assert_eq!(
            valeur(&items, "executables_without_directory"),
            ItemValue::illisible(RACINE_SYSTEME_INCONNUE)
        );
    }

    /// **Une tâche absente n'est jamais une tâche désactivée**, et l'inverse.
    ///
    /// C'est le geste de [`crate::etat_effectif::execution_service`] sur le
    /// service `Sense` : publier « à l'arrêt » sur ce qui n'existe pas
    /// fabriquerait un écart de toutes pièces. Ici la confusion se prendrait par
    /// l'autre bout — filtrer les tâches à l'arrêt hors de la liste principale
    /// ferait lire une désactivation comme une disparition.
    #[test]
    fn une_tache_a_larret_reste_enregistree_et_un_etat_non_rapporte_nest_pas_un_arret() {
        let releve = vec![
            TacheRelevee {
                dossier: r"\PowerToys\".to_owned(),
                nom: "Autorun".to_owned(),
                etat: Some(ETAT_DESACTIVEE),
                actions: Some(vec![]),
            },
            TacheRelevee {
                dossier: r"\Mozilla\".to_owned(),
                nom: "Default Browser Agent".to_owned(),
                // Non rapporté. Ni prête, ni à l'arrêt : on ne sait pas.
                etat: None,
                actions: Some(vec![]),
            },
        ];
        let items = items_depuis(&Lecture::Trouvee(releve), &env());

        // La tâche à l'arrêt figure dans les DEUX listes : la désactiver a
        // ajouté une entrée quelque part, elle n'en a retiré aucune.
        assert_eq!(
            valeur(&items, "outside_microsoft_root"),
            ItemValue::List(vec![
                r"\Mozilla\Default Browser Agent".to_owned(),
                r"\PowerToys\Autorun".to_owned(),
            ])
        );
        assert_eq!(
            valeur(&items, "disabled_outside_microsoft_root"),
            ItemValue::List(vec![r"\PowerToys\Autorun".to_owned()]),
            "un état non rapporté n'est pas un arrêt"
        );
    }

    /// La racine de l'éditeur est un emplacement, et la casse n'en décide pas.
    #[test]
    fn la_racine_microsoft_est_un_emplacement_pas_un_auteur() {
        // Hors racine, alors que l'éditeur en est l'auteur : mesuré.
        assert!(tache(r"\SoftLanding\", "Deferral", &[]).hors_racine_microsoft());
        // Dedans, quelle que soit la casse : les deux écritures désignent le
        // même dossier du planificateur.
        for dedans in [
            r"\Microsoft\Windows\",
            r"\microsoft\windows\",
            r"\Microsoft\",
        ] {
            assert!(
                !tache(dedans, "T", &[]).hors_racine_microsoft(),
                "« {dedans} » est la racine de l'éditeur"
            );
        }
        // Et un dossier qui commence pareil sans en être un : la barre oblique
        // finale de la constante l'exclut.
        assert!(tache(r"\MicrosoftBis\", "T", &[]).hors_racine_microsoft());
        assert!(tache(r"\", "T", &[]).hors_racine_microsoft());
    }

    /// **Aucun item de tâche n'est déclarable** (B4).
    ///
    /// Et ce n'est pas une limite de phase : écrire une tâche, c'est
    /// `RegisterByXml` ou `SetByObject`, c'est-à-dire un verbe refusé
    /// définitivement. La classer `Reglage` promettrait une convergence qui
    /// n'arrivera pas ; la classer `Objectif` la ferait entrer dans
    /// `workstation.yaml`, où elle passerait en écart à la première mise à jour
    /// d'application.
    ///
    /// Le `match` est exhaustif **sans bras `_`** : ajouter une variante à
    /// `Nature` casse ici la compilation, donc la CI, avant qu'un test s'exécute.
    #[test]
    fn aucun_item_de_tache_nest_declarable() {
        let items = items_depuis(&Lecture::Trouvee(releve_de_reference()), &env());
        let (mut mesures, mut constats) = (0, 0);
        for item in &items {
            match item.nature {
                Nature::Mesure => mesures += 1,
                Nature::Constat => constats += 1,
                Nature::Reglage | Nature::Objectif => panic!(
                    "« {} » se dit déclarable : aucun verbe n'écrira une tâche \
                     planifiée, et `RegisterByXml` restera refusé",
                    item.path
                ),
            }
            assert!(!item.nature.est_declarable(), "« {}  »", item.path);
            assert!(!item.nature.est_convergeable(), "« {} »", item.path);
            assert!(item.desired.is_none());
        }
        assert_eq!(
            (mesures, constats),
            (2, 4),
            "répartition des natures des tâches : deux décomptes qui bougent \
             seuls, quatre listes qui décrivent la machine"
        );
    }

    /// Tout item porte sa finalité et son risque (P6).
    #[test]
    fn tout_item_de_tache_porte_son_explication() {
        for lecture in [
            Lecture::Trouvee(releve_de_reference()),
            Lecture::Absente,
            Lecture::Refusee,
        ] {
            for item in items_depuis(&lecture, &env()) {
                assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
                assert!(!item.risk.is_empty(), "« {} » sans risque", item.path);
                assert_eq!(item.domain, Domain::Configuration);
            }
        }
    }

    /// **La barrière de la surface d'écriture** (B1).
    ///
    /// Ce module lit par `query::<T>()`, qui ne rend que des structures inertes.
    /// La voie alternative — `exec_query` puis `get_property` — ferait passer
    /// `IWbemClassWrapper` dans notre code, et ce type expose `put_property`,
    /// `spawn_instance` et `get_method`. La lecture seule cesserait d'être portée
    /// par la signature.
    ///
    /// Le balayage refuse aussi `PS_ScheduledTask`, qui vit dans **le même espace
    /// de noms** et porte `RegisterByXml`, `StartByPath` et `DisableByName`.
    ///
    /// **Sa limite est assumée**, comme celle de la barrière SEC-02 : les motifs
    /// sont assemblés par `concat!` pour que ce fichier ne contienne pas
    /// lui-même ce qu'il interdit, et un `concat!` de l'autre côté la
    /// contournerait. Un balayage de source ne remplace ni la revue ni l'ADR.
    #[test]
    fn le_collecteur_de_taches_ninvoque_aucune_methode_wmi() {
        const SOURCE: &str = include_str!("taches.rs");
        let interdits = [
            concat!("exec", "_query"),
            concat!("raw", "_query"),
            concat!("exec", "_method"),
            concat!("exec_class", "_method"),
            concat!("exec_instance", "_method"),
            concat!("put", "_property"),
            concat!("spawn", "_instance"),
            concat!("get", "_method"),
            concat!("PS_Sched", "uledTask"),
        ];
        // Le corps du module, sans les commentaires : ils nomment volontairement
        // ce qu'ils interdisent, et un contrôle qui les lit ne pourrait plus
        // rien interdire sans se contredire.
        let code: String = SOURCE
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            code.contains("query::<TacheLue>"),
            "la voie de lecture a changé de nom : la barrière ne garde plus rien"
        );
        for motif in interdits {
            assert!(
                !code.contains(motif),
                "« {motif} » atteint la surface d'écriture de WMI, ou une classe \
                 qui la porte — le verbe passe par une ADR, pas par ce module"
            );
        }
    }
}

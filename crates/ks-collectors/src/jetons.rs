//! Le vocabulaire fermé des valeurs qui sortent d'une table de codes (ADR-0015).
//!
//! ## Pourquoi ce module existe
//!
//! Douze items relevés sur la machine de référence se comparaient sur une
//! **phrase française** écrite par le collecteur — `activée, sans verrou UEFI`,
//! `en cours d'exécution`, `automatique`. Rien ne protégeait ces chaînes :
//! retirer une virgule, corriger une coquille, remplacer « à l'arrêt » par
//! « arrêté » faisait basculer d'un coup tous les items déclarés du collecteur
//! en écart, sur une machine où rien n'avait bougé. Le compilateur ne disait
//! rien, aucun test ne disait rien, et la revue voyait une amélioration de
//! style.
//!
//! Pire, l'effet était **rétroactif** : le magasin d'observations conserve les
//! valeurs telles qu'écrites, donc une reformulation transformait la série des
//! sept jours passés en une série où tout avait changé le jour de la
//! recompilation.
//!
//! ## La règle
//!
//! **Ces jetons sont un format.** Ils ne se renomment jamais, au même titre que
//! [`ks_core::Actor::etiquette_stable`] — minuscules, sans accent, sans
//! ponctuation, sans espace. Ils partent dans le relevé, dans le magasin
//! d'observations et, demain, dans `workstation.yaml`, où l'utilisateur les
//! écrit à la main.
//!
//! Le libellé français, lui, vit dans `ks_cli::lisible`, avec la conversion
//! d'octets et pour exactement la même raison : un item porte ce que la machine
//! a mesuré, la traduction appartient à l'affichage.
//!
//! ## Ce qui garde le format
//!
//! Chaque table implémente [`TableDeCodes`], dont `variantes()` porte un
//! `match` **exhaustif sans bras `_`** : ajouter un code casse la compilation
//! avant qu'un test s'exécute. [`tous`] agrège ces tables, et le test
//! `toute_table_de_codes_figure_dans_le_recensement` confronte ce recensement
//! aux `impl TableDeCodes for` réellement présents dans ce fichier — une table
//! oubliée ne peut donc pas échapper au contrôle des libellés.
//!
//! ## Ce que ce module ne couvre pas
//!
//! Les modes des règles ASR (`mode_asr`) restent en français : ils vivent à
//! l'intérieur d'un [`ks_core::ItemValue::List`], accolés au GUID de la règle
//! (`<guid> = audit`), et non dans un `Text`. Les déplacer suppose de décider
//! ce qu'est le jeton d'une *liste* de couples, ce que l'ADR-0015 n'a pas
//! tranché. La limite est nommée ici plutôt que laissée à la découverte.

/// Préfixe du jeton d'un code que la table ne nomme pas.
///
/// Le suffixe conserve la nuance à laquelle le type de démarrage tenait déjà :
/// un octet parasite se **nomme**, il ne tombe pas dans « désactivé ». Sans
/// lui, un code inattendu se confondrait avec un état réellement mesuré.
pub const PREFIXE_CODE_INCONNU: &str = "code-inconnu:";

/// Le jeton d'un code hors table.
fn code_inconnu(n: u32) -> String {
    format!("{PREFIXE_CODE_INCONNU}{n}")
}

/// Ce que toute table de codes sait faire.
///
/// L'intérêt du trait n'est pas le polymorphisme — personne n'appelle ces
/// tables derrière un `dyn` — mais le **recensement** : il donne à [`tous`] un
/// moyen uniforme d'énumérer le vocabulaire complet, donc à la CLI un moyen
/// d'exiger que chaque jeton porte son libellé.
pub trait TableDeCodes: Copy + Sized {
    /// Toutes les variantes, un code hors table compris.
    ///
    /// La liste est écrite à la main, mais **pas laissée à la vigilance** : le
    /// `match` exhaustif sans bras `_` de chaque implémentation casse la
    /// compilation le jour où une variante s'ajoute sans rejoindre la liste.
    fn variantes() -> Vec<Self>;

    /// Le jeton, tel qu'il part dans le relevé. **Il ne se renomme jamais.**
    fn jeton(self) -> String;
}

/// Type de démarrage d'un service Windows, tel que le registre l'encode.
///
/// Les valeurs viennent de `HKLM\SYSTEM\CurrentControlSet\Services\<nom>\Start`
/// et sont documentées par Microsoft : 0 au démarrage du noyau, 1 au démarrage
/// du système, 2 automatique, 3 manuel, 4 désactivé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemarrageService {
    /// `0` — chargé par le chargeur d'amorçage.
    Noyau,
    /// `1` — initialisé pendant le démarrage du noyau.
    Systeme,
    /// `2` — démarré automatiquement par le gestionnaire de services.
    Automatique,
    /// `3` — démarré à la demande.
    Manuel,
    /// `4` — désactivé.
    Desactive,
    /// Un octet que Microsoft ne documente pas. Il se nomme.
    CodeInconnu(u32),
}

impl DemarrageService {
    /// Range un octet du registre dans la table.
    #[must_use]
    pub const fn depuis_code(valeur: u32) -> Self {
        match valeur {
            0 => Self::Noyau,
            1 => Self::Systeme,
            2 => Self::Automatique,
            3 => Self::Manuel,
            4 => Self::Desactive,
            n => Self::CodeInconnu(n),
        }
    }
}

impl TableDeCodes for DemarrageService {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![
            Self::Noyau,
            Self::Systeme,
            Self::Automatique,
            Self::Manuel,
            Self::Desactive,
            Self::CodeInconnu(7),
        ];
        for v in &echantillons {
            match v {
                Self::Noyau
                | Self::Systeme
                | Self::Automatique
                | Self::Manuel
                | Self::Desactive
                | Self::CodeInconnu(_) => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Noyau => "demarrage-noyau".to_owned(),
            Self::Systeme => "demarrage-systeme".to_owned(),
            Self::Automatique => "automatique".to_owned(),
            Self::Manuel => "manuel".to_owned(),
            Self::Desactive => "desactive".to_owned(),
            Self::CodeInconnu(n) => code_inconnu(n),
        }
    }
}

/// Une protection dont le verrou UEFI est optionnel, telle que le registre l'encode.
///
/// Même table pour `RunAsPPL` (protection LSA) et `LsaCfgFlags` (Credential
/// Guard), toutes deux documentées par Microsoft :
///
/// > To configure the feature **with** a UEFI variable, use […] `00000001`.
/// > To configure the feature **without** a UEFI variable, use […] `00000002`.
///
/// Le premier jet inversait les deux, et c'était le pire sens : sur la machine
/// de référence `RunAsPPL` vaut 2, donc Keystone annonçait « verrouillée par
/// UEFI » alors que la protection cède à un `reg add` suivi d'un redémarrage.
///
/// C'est **la seule** transcription de ces trois codes : la protection LSA,
/// qui se déplie en deux items, dérive la sienne d'ici plutôt que de recopier
/// la table (voir [`EtatProtectionLsa::depuis`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionVerrouillable {
    /// `0` — la protection n'est pas configurée.
    Inactive,
    /// `1` — active, et verrouillée par une variable UEFI.
    ActiveVerrouUefi,
    /// `2` — active, sans variable UEFI : elle cède à une écriture du registre.
    ActiveSansVerrouUefi,
    /// Un octet que Microsoft ne documente pas.
    CodeInconnu(u32),
}

impl ProtectionVerrouillable {
    /// Range un octet du registre dans la table.
    #[must_use]
    pub const fn depuis_code(valeur: u32) -> Self {
        match valeur {
            0 => Self::Inactive,
            1 => Self::ActiveVerrouUefi,
            2 => Self::ActiveSansVerrouUefi,
            n => Self::CodeInconnu(n),
        }
    }

    /// Le verrou UEFI est-il posé ? `None` quand le code ne permet pas de le dire.
    ///
    /// Un code hors table ne répond **ni oui ni non** : prétendre « pas de
    /// verrou » sur un octet qu'on ne sait pas lire serait un faux positif sur
    /// la contre-mesure qui garde les identifiants.
    #[must_use]
    pub const fn verrou_uefi(self) -> Option<bool> {
        match self {
            Self::ActiveVerrouUefi => Some(true),
            Self::Inactive | Self::ActiveSansVerrouUefi => Some(false),
            Self::CodeInconnu(_) => None,
        }
    }
}

impl TableDeCodes for ProtectionVerrouillable {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![
            Self::Inactive,
            Self::ActiveVerrouUefi,
            Self::ActiveSansVerrouUefi,
            Self::CodeInconnu(7),
        ];
        for v in &echantillons {
            match v {
                Self::Inactive
                | Self::ActiveVerrouUefi
                | Self::ActiveSansVerrouUefi
                | Self::CodeInconnu(_) => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Inactive => "inactive".to_owned(),
            Self::ActiveVerrouUefi => "active-verrou-uefi".to_owned(),
            Self::ActiveSansVerrouUefi => "active-sans-verrou-uefi".to_owned(),
            Self::CodeInconnu(n) => code_inconnu(n),
        }
    }
}

/// L'**état** de la protection LSA, son verrou mis à part (ADR-0015, point 4).
///
/// `activée, sans verrou UEFI` portait deux faits dans une seule chaîne, ce que
/// le principe P6 refuse : un indicateur composite est toujours dépliable en
/// ses composantes exactes. La protection LSA donne donc deux items — l'état
/// ici, le verrou dans `security.platform.lsa_protection_uefi_lock` — et l'on
/// peut enfin déclarer « je veux la protection LSA active » sans se prononcer
/// sur le verrou.
///
/// Credential Guard, lui, n'est **pas** déplié : il garde le jeton composite de
/// [`ProtectionVerrouillable`]. La différence est assumée — l'ADR-0015 ne
/// déplie qu'un item, et son verrou n'a pas la même portée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatProtectionLsa {
    /// La protection n'est pas configurée.
    Inactive,
    /// La protection est active, verrouillée ou non.
    Active,
    /// Un octet que Microsoft ne documente pas.
    CodeInconnu(u32),
}

impl EtatProtectionLsa {
    /// Dérive l'état du code lu, sans recopier la table.
    ///
    /// Une seconde transcription des octets 0, 1 et 2 divergerait de la
    /// première au premier changement ; celle-ci ne peut pas.
    #[must_use]
    pub const fn depuis(protection: ProtectionVerrouillable) -> Self {
        match protection {
            ProtectionVerrouillable::Inactive => Self::Inactive,
            ProtectionVerrouillable::ActiveVerrouUefi
            | ProtectionVerrouillable::ActiveSansVerrouUefi => Self::Active,
            ProtectionVerrouillable::CodeInconnu(n) => Self::CodeInconnu(n),
        }
    }
}

impl TableDeCodes for EtatProtectionLsa {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![Self::Inactive, Self::Active, Self::CodeInconnu(7)];
        for v in &echantillons {
            match v {
                Self::Inactive | Self::Active | Self::CodeInconnu(_) => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Inactive => "inactive".to_owned(),
            Self::Active => "active".to_owned(),
            Self::CodeInconnu(n) => code_inconnu(n),
        }
    }
}

/// `VirtualizationBasedSecurityStatus` de `Win32_DeviceGuard`.
///
/// Table documentée par Microsoft, page « Validate enabled VBS and memory
/// integrity features » :
///
/// > **0** VBS isn't enabled. **1** VBS is enabled but not running.
/// > **2** VBS is enabled and running.
///
/// **C'est le 1 qui justifie toute cette lecture.** Une machine où VBS est
/// configuré sans tourner porte exactement la même configuration au registre
/// qu'une machine protégée. Le registre les confond ; cette table les sépare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatVbs {
    /// `0` — VBS n'est pas activé.
    Eteint,
    /// `1` — activé, mais pas en cours d'exécution.
    ConfigureNonDemarre,
    /// `2` — activé et en cours d'exécution.
    EnExecution,
    /// Un code que Microsoft ne documente pas.
    CodeInconnu(u32),
}

impl EtatVbs {
    /// Range un code WMI dans la table.
    #[must_use]
    pub const fn depuis_code(valeur: u32) -> Self {
        match valeur {
            0 => Self::Eteint,
            1 => Self::ConfigureNonDemarre,
            2 => Self::EnExecution,
            n => Self::CodeInconnu(n),
        }
    }
}

impl TableDeCodes for EtatVbs {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![
            Self::Eteint,
            Self::ConfigureNonDemarre,
            Self::EnExecution,
            Self::CodeInconnu(9),
        ];
        for v in &echantillons {
            match v {
                Self::Eteint
                | Self::ConfigureNonDemarre
                | Self::EnExecution
                | Self::CodeInconnu(_) => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Eteint => "eteint".to_owned(),
            Self::ConfigureNonDemarre => "configure-non-demarre".to_owned(),
            Self::EnExecution => SERVICE_EN_EXECUTION.to_owned(),
            Self::CodeInconnu(n) => code_inconnu(n),
        }
    }
}

/// `CodeIntegrityPolicyEnforcementStatus` de `Win32_DeviceGuard`.
///
/// Documenté : **0** Off, **1** Audit, **2** Enforced. Le mode audit journalise
/// sans bloquer — même nuance que pour les règles ASR, et même piège : le
/// compter comme une protection serait un faux positif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegriteCode {
    /// `0` — aucune stratégie appliquée.
    Eteinte,
    /// `1` — la stratégie journalise sans bloquer.
    Audit,
    /// `2` — la stratégie est imposée.
    Imposee,
    /// Un code que Microsoft ne documente pas.
    CodeInconnu(u32),
}

impl IntegriteCode {
    /// Range un code WMI dans la table.
    #[must_use]
    pub const fn depuis_code(valeur: u32) -> Self {
        match valeur {
            0 => Self::Eteinte,
            1 => Self::Audit,
            2 => Self::Imposee,
            n => Self::CodeInconnu(n),
        }
    }
}

impl TableDeCodes for IntegriteCode {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![
            Self::Eteinte,
            Self::Audit,
            Self::Imposee,
            Self::CodeInconnu(9),
        ];
        for v in &echantillons {
            match v {
                Self::Eteinte | Self::Audit | Self::Imposee | Self::CodeInconnu(_) => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Eteinte => "eteinte".to_owned(),
            Self::Audit => "audit".to_owned(),
            Self::Imposee => "imposee".to_owned(),
            Self::CodeInconnu(n) => code_inconnu(n),
        }
    }
}

/// Jeton d'un service qui tourne réellement.
///
/// Partagé avec [`EtatVbs::EnExecution`] : c'est le même fait, il ne s'écrit
/// donc pas de deux façons.
pub const SERVICE_EN_EXECUTION: &str = "en-execution";

/// Un service s'exécute-t-il réellement ?
///
/// Deux valeurs et pas un booléen, délibérément : un booléen s'affiche
/// « activé », le vocabulaire d'un interrupteur. Or la question posée est
/// « est-ce que ça tourne ? », à laquelle « activé » répond de travers — c'est
/// précisément la confusion entre configuration et exécution que la lecture WMI
/// existe pour lever.
///
/// # Deux familles d'items s'en servent, et c'est le même fait
///
/// Les services protégés par l'hyperviseur, dont l'exécution se lit dans
/// `SecurityServicesRunning` (`security.platform.hvci_running`), et les services
/// Windows surveillés, dont l'exécution se lit dans `Win32_Service`
/// (`security.services.<nom>.running`). Une seconde table portant les mêmes
/// jetons dirait deux fois la même chose et divergerait au premier changement ;
/// c'est la raison pour laquelle cette table ne nomme plus sa source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionService {
    /// Le service s'exécute.
    EnExecution,
    /// On a regardé, et il ne s'exécute pas. C'est un constat, pas une lacune.
    Arrete,
}

impl ExecutionService {
    /// Range un constat d'exécution.
    ///
    /// Le booléen attendu vient d'une lecture **réussie** : appeler cette
    /// fonction avec `false` parce qu'on n'a rien pu lire produirait le faux
    /// négatif que tout ce crate existe pour éviter.
    #[must_use]
    pub const fn depuis_execution(en_execution: bool) -> Self {
        if en_execution {
            Self::EnExecution
        } else {
            Self::Arrete
        }
    }
}

impl TableDeCodes for ExecutionService {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![Self::EnExecution, Self::Arrete];
        for v in &echantillons {
            match v {
                Self::EnExecution | Self::Arrete => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::EnExecution => SERVICE_EN_EXECUTION.to_owned(),
            Self::Arrete => "arrete".to_owned(),
        }
    }
}

/// Ce que le matériel sait faire, d'après `AvailableSecurityProperties`.
///
/// Réponse textuelle, pas booléenne, pour la même raison que
/// [`ExecutionService`] : la question est « ce matériel en est-il capable »,
/// jamais « est-ce en service ». Sur une machine où la protection DMA est
/// disponible mais non activée, « activé » serait un faux positif de sécurité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProprieteMaterielle {
    /// Le code figure dans la liste lue.
    Disponible,
    /// La liste a été lue, et le code n'y est pas.
    Absente,
}

impl ProprieteMaterielle {
    /// Range une appartenance à la liste lue.
    #[must_use]
    pub const fn depuis_presence(present: bool) -> Self {
        if present {
            Self::Disponible
        } else {
            Self::Absente
        }
    }
}

impl TableDeCodes for ProprieteMaterielle {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![Self::Disponible, Self::Absente];
        for v in &echantillons {
            match v {
                Self::Disponible | Self::Absente => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Disponible => "disponible-sur-ce-materiel".to_owned(),
            Self::Absente => "absente-de-ce-materiel".to_owned(),
        }
    }
}

/// L'attribution de l'inventaire logiciel couvre-t-elle tous les gestionnaires ?
///
/// Texte plutôt que booléen : « activé / désactivé » est le vocabulaire d'un
/// interrupteur, pas d'une question de complétude. Un item qu'on lit de travers
/// est un item mal conçu, même si sa valeur est juste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    /// Tous les gestionnaires détectés ont pu être interrogés.
    Complete,
    /// Au moins un gestionnaire reste aveugle : « non attribué » est un majorant.
    Partielle,
}

impl Attribution {
    /// Range le constat de complétude.
    #[must_use]
    pub const fn depuis_completude(complete: bool) -> Self {
        if complete {
            Self::Complete
        } else {
            Self::Partielle
        }
    }
}

impl TableDeCodes for Attribution {
    fn variantes() -> Vec<Self> {
        let echantillons = vec![Self::Complete, Self::Partielle];
        for v in &echantillons {
            match v {
                Self::Complete | Self::Partielle => {}
            }
        }
        echantillons
    }

    fn jeton(self) -> String {
        match self {
            Self::Complete => "complete".to_owned(),
            Self::Partielle => "partielle".to_owned(),
        }
    }
}

/// Le vocabulaire complet, table par table.
///
/// C'est le crochet dont la CLI se sert pour exiger que **chaque** jeton porte
/// son libellé français. Une table absente de ce recensement échapperait au
/// contrôle : le test `toute_table_de_codes_figure_dans_le_recensement` l'exige
/// donc en confrontant cette fonction aux `impl TableDeCodes for` du fichier.
#[must_use]
pub fn tous() -> Vec<String> {
    fn ajouter<T: TableDeCodes>(vocabulaire: &mut Vec<String>) {
        vocabulaire.extend(T::variantes().into_iter().map(T::jeton));
    }

    let mut vocabulaire = Vec::new();
    ajouter::<DemarrageService>(&mut vocabulaire);
    ajouter::<ProtectionVerrouillable>(&mut vocabulaire);
    ajouter::<EtatProtectionLsa>(&mut vocabulaire);
    ajouter::<EtatVbs>(&mut vocabulaire);
    ajouter::<IntegriteCode>(&mut vocabulaire);
    ajouter::<ExecutionService>(&mut vocabulaire);
    ajouter::<ProprieteMaterielle>(&mut vocabulaire);
    ajouter::<Attribution>(&mut vocabulaire);
    vocabulaire
}

/// Un jeton est-il écrit dans le format ?
///
/// Minuscules, sans espace, sans accent — ni aucun caractère hors ASCII. La
/// fonction dit *si*, jamais *pourquoi* : c'est au contrôle mécanique de
/// l'inventaire de nommer l'item fautif.
#[must_use]
pub fn est_bien_forme(jeton: &str) -> bool {
    !jeton.is_empty()
        && jeton
            .chars()
            .all(|c| c.is_ascii() && !c.is_ascii_uppercase() && !c.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn un_jeton_ne_porte_ni_espace_ni_majuscule_ni_accent() {
        // Le format, éprouvé sur le vocabulaire entier plutôt que sur les
        // quelques jetons auxquels on aurait pensé.
        for jeton in tous() {
            assert!(
                est_bien_forme(&jeton),
                "« {jeton} » n'est pas écrit dans le format des jetons"
            );
        }

        // Et la fonction de forme refuse bien ce qu'elle prétend refuser :
        // sans ces quatre lignes, `est_bien_forme` pourrait renvoyer `true`
        // sans rien regarder, et le contrôle ci-dessus passerait à vide.
        for refuse in ["activée, sans verrou UEFI", "en cours d'exécution", "A", ""] {
            assert!(!est_bien_forme(refuse), "« {refuse} » devrait être refusé");
        }
    }

    /// La table figée, jeton par jeton. **Ces valeurs ne se renomment jamais.**
    ///
    /// Écrite à la main, mais gardée par les `match` exhaustifs sans bras `_`
    /// de [`TableDeCodes::variantes`] : ajouter un code casse la compilation,
    /// donc la CI, avant même que ce test s'exécute. Le contributeur est forcé
    /// de venir ici décider du jeton de son nouveau code — et, dans la CLI, de
    /// son libellé français.
    #[test]
    fn les_jetons_sont_un_format_qui_ne_se_renomme_jamais() {
        let attendu = |table: Vec<String>, jetons: &[&str]| {
            assert_eq!(
                table,
                jetons.iter().map(|j| (*j).to_owned()).collect::<Vec<_>>()
            );
        };

        attendu(
            DemarrageService::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &[
                "demarrage-noyau",
                "demarrage-systeme",
                "automatique",
                "manuel",
                "desactive",
                "code-inconnu:7",
            ],
        );
        attendu(
            ProtectionVerrouillable::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &[
                "inactive",
                "active-verrou-uefi",
                "active-sans-verrou-uefi",
                "code-inconnu:7",
            ],
        );
        attendu(
            EtatProtectionLsa::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &["inactive", "active", "code-inconnu:7"],
        );
        attendu(
            EtatVbs::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &[
                "eteint",
                "configure-non-demarre",
                "en-execution",
                "code-inconnu:9",
            ],
        );
        attendu(
            IntegriteCode::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &["eteinte", "audit", "imposee", "code-inconnu:9"],
        );
        attendu(
            ExecutionService::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &["en-execution", "arrete"],
        );
        attendu(
            ProprieteMaterielle::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &["disponible-sur-ce-materiel", "absente-de-ce-materiel"],
        );
        attendu(
            Attribution::variantes()
                .into_iter()
                .map(TableDeCodes::jeton)
                .collect(),
            &["complete", "partielle"],
        );
    }

    #[test]
    fn un_code_hors_table_se_nomme_au_lieu_de_tomber_dans_un_etat_reel() {
        // La nuance à laquelle le type de démarrage tenait déjà : un octet
        // parasite ne se confond pas avec « désactivé ». Le faire tomber dans
        // un état réel ferait passer une lecture aberrante pour un constat.
        assert_eq!(DemarrageService::depuis_code(42).jeton(), "code-inconnu:42");
        assert_ne!(DemarrageService::depuis_code(42).jeton(), "desactive");
        assert_eq!(
            ProtectionVerrouillable::depuis_code(7).jeton(),
            "code-inconnu:7"
        );
        assert_ne!(ProtectionVerrouillable::depuis_code(7).jeton(), "inactive");
        assert_eq!(EtatVbs::depuis_code(9).jeton(), "code-inconnu:9");
        assert_eq!(IntegriteCode::depuis_code(9).jeton(), "code-inconnu:9");
    }

    #[test]
    fn le_verrou_uefi_nest_pas_deduit_dun_code_quon_ne_sait_pas_lire() {
        // Microsoft documente : 1 AVEC variable UEFI, 2 SANS. Le premier jet du
        // collecteur inversait les deux, et annonçait une protection
        // verrouillée qui cède en réalité à un `reg add` suivi d'un
        // redémarrage — faux positif sur la garde des identifiants.
        assert_eq!(
            ProtectionVerrouillable::depuis_code(1).verrou_uefi(),
            Some(true)
        );
        assert_eq!(
            ProtectionVerrouillable::depuis_code(2).verrou_uefi(),
            Some(false)
        );
        assert_eq!(
            ProtectionVerrouillable::depuis_code(0).verrou_uefi(),
            Some(false)
        );
        // Et un code hors table ne répond ni oui ni non.
        assert_eq!(ProtectionVerrouillable::depuis_code(7).verrou_uefi(), None);
    }

    #[test]
    fn letat_de_la_protection_lsa_ne_se_prononce_plus_sur_le_verrou() {
        // Le dépliage de l'ADR-0015 : déclarer « je veux la protection LSA
        // active » ne doit plus obliger à se prononcer sur le verrou. Les codes
        // 1 et 2 donnent donc le MÊME état, et leur différence vit dans l'item
        // voisin `lsa_protection_uefi_lock`.
        let verrouille = EtatProtectionLsa::depuis(ProtectionVerrouillable::depuis_code(1));
        let sans_verrou = EtatProtectionLsa::depuis(ProtectionVerrouillable::depuis_code(2));
        assert_eq!(verrouille, sans_verrou);
        assert_eq!(verrouille.jeton(), "active");

        assert_eq!(
            EtatProtectionLsa::depuis(ProtectionVerrouillable::depuis_code(0)).jeton(),
            "inactive"
        );
        // Un code hors table reste nommé de bout en bout de la dérivation.
        assert_eq!(
            EtatProtectionLsa::depuis(ProtectionVerrouillable::depuis_code(7)).jeton(),
            "code-inconnu:7"
        );
    }

    /// Le recensement de [`tous`] couvre-t-il toutes les tables du fichier ?
    ///
    /// La faiblesse d'un recensement écrit à la main est qu'on l'oublie. La
    /// barrière est donc textuelle et autonome, comme
    /// `la_liste_des_collecteurs_est_a_jour` : les `impl TableDeCodes for`
    /// vivent tous ici, `include_str!` suffit à confronter la fonction à son
    /// propre fichier. Une table non recensée n'aurait pas de libellé exigé
    /// côté CLI, donc s'afficherait en jeton brut à l'écran.
    #[test]
    fn toute_table_de_codes_figure_dans_le_recensement() {
        const SOURCE: &str = include_str!("jetons.rs");

        let implementees: BTreeSet<&str> = SOURCE
            .lines()
            .filter_map(|l| l.trim().strip_prefix("impl TableDeCodes for "))
            .filter_map(|reste| reste.split_whitespace().next())
            .collect();

        let recensees: BTreeSet<&str> = SOURCE
            .lines()
            .filter_map(|l| l.trim().strip_prefix("ajouter::<"))
            .filter_map(|reste| reste.split('>').next())
            .collect();

        assert!(
            !implementees.is_empty(),
            "aucun `impl TableDeCodes for` trouvé — la barrière ne barre plus rien"
        );
        assert_eq!(
            implementees, recensees,
            "une table de codes n'est pas recensée par `tous()` :
   implémentées {implementees:?}
   recensées    {recensees:?}"
        );

        // Et le recensement produit bien quelque chose : un `tous()` vide
        // rendrait le contrôle des libellés de la CLI vert sans rien vérifier.
        assert!(tous().len() >= implementees.len());
    }
}

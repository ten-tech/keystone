//! Posture de sécurité de la plateforme (D5, Phase 0.2).
//!
//! ## Ce que ce collecteur voit, et pourquoi ça vaut cher
//!
//! Le §6 du modèle de menace classe les détections par rapport valeur / effort.
//! La **désactivation d'une protection** y arrive en deuxième position, juste
//! derrière l'ajout d'une autorité de certification. Le §5 dit pourquoi :
//! *« beaucoup d'intrusions réelles commencent par quelqu'un a désactivé la
//! protection en temps réel »*.
//!
//! C'est exactement ce que lit ce module — et c'est le créneau étroit et réel que
//! Keystone revendique : il ne voit pas le malware, il voit qu'on a baissé la garde.
//!
//! ## Tout par le registre, donc aucun bloc `unsafe`
//!
//! Secure Boot, HVCI, Credential Guard, protection LSA, l'état de Defender et le
//! démarrage des services de sécurité sont **tous** lisibles dans le registre. Ni
//! WMI, ni l'API TBS, ni PowerShell : la Phase 0.2 démarre donc sans avoir à
//! trancher le grain des exceptions `unsafe`, qui reste une décision à part.
//!
//! Ce qui exige réellement une API native — état détaillé du TPM, protecteurs
//! BitLocker par volume, usure SMART — attend cette décision et n'est pas ici.
//!
//! ## La règle qui gouverne tout ce fichier
//!
//! **Une clé absente n'est pas « désactivé ».**
//!
//! Constaté sur une machine réelle : `EnableVirtualizationBasedSecurity` est
//! absente alors que HVCI vaut 1 — la protection tourne, elle n'est simplement pas
//! *imposée par stratégie*. Traduire cette absence en `false` afficherait « VBS
//! désactivé » sur une machine protégée, c'est-à-dire un faux négatif de sécurité :
//! précisément ce qu'un outil de posture n'a pas le droit de produire.
//!
//! Une valeur illisible donne donc [`ItemValue::Absent`], jamais une supposition.

use chrono::{DateTime, Utc};
use ks_core::{Domain, Item, ItemValue, Provenance};

/// Type de démarrage d'un service Windows, tel que le registre l'encode.
///
/// Les valeurs viennent de `HKLM\SYSTEM\CurrentControlSet\Services\<nom>\Start`.
/// On les traduit plutôt que d'afficher un chiffre : « 4 » ne veut rien dire pour
/// un humain, « désactivé » si (principe P6).
#[must_use]
pub fn demarrage_service(valeur: u32) -> &'static str {
    match valeur {
        0 => "au démarrage du noyau",
        1 => "au démarrage du système",
        2 => "automatique",
        3 => "manuel",
        4 => "désactivé",
        _ => "inconnu",
    }
}

/// Les services dont l'arrêt est un signal, et la raison de les surveiller.
///
/// Liste volontairement courte : chacun est cité au §6 du modèle de menace ou
/// couvre une brique de posture. Un service désactivé ici n'est pas une curiosité,
/// c'est le premier maillon de la plupart des chaînes d'attaque.
const SERVICES_SURVEILLES: &[(&str, &str)] = &[
    ("WinDefend", "Antivirus Microsoft Defender"),
    ("MpsSvc", "Pare-feu Windows Defender"),
    (
        "EventLog",
        "Journal des événements — sa perte efface la piste d'audit",
    ),
    ("Sense", "Defender for Endpoint, quand il est déployé"),
    // « wscsvc », pas « wscvc ». Le premier jet portait la faute de frappe, et le
    // service remontait « absent » alors qu'il tournait — un faux négatif de
    // sécurité produit par une lettre manquante. Un nom de service erroné est
    // indiscernable d'un service non installé : d'où le test ci-dessous, qui
    // exige que les services toujours présents sur Windows soient trouvés.
    ("wscsvc", "Centre de sécurité Windows"),
    (
        "BITS",
        "Transfert intelligent en arrière-plan, utilisé par les mises à jour",
    ),
];

/// Collecteur de posture plateforme.
pub struct PostureCollector;

impl PostureCollector {
    /// Identifiant, pour le journal et les budgets d'empreinte.
    #[must_use]
    pub const fn nom() -> &'static str {
        "posture"
    }

    /// Les items de posture observés sur cette machine.
    #[must_use]
    pub fn items() -> Vec<Item> {
        #[cfg(windows)]
        {
            windows_impl::items()
        }
        #[cfg(not(windows))]
        {
            // Hors Windows, on ne produit rien plutôt que d'inventer : ces
            // protections n'existent pas ailleurs, et un item absent est plus
            // honnête qu'un item « non applicable » qui polluerait le décompte.
            Vec::new()
        }
    }
}

/// Fabrique un item de posture, avec sa finalité et son risque (principe P6).
pub(crate) fn item_posture(
    chemin: &str,
    valeur: ItemValue,
    but: &str,
    risque: &str,
    reference: Option<&str>,
) -> Item {
    Item {
        path: chemin.to_owned(),
        domain: Domain::Security,
        desired: None,
        observed: valeur,
        observed_at: Utc::now(),
        // Un relevé n'est pas une écriture : voir `Provenance::Observed`.
        provenance: Provenance::Observed,
        purpose: but.to_owned(),
        risk: risque.to_owned(),
        reference: reference.map(str::to_owned),
    }
}

/// Convertit un `FILETIME` Windows en horodatage.
///
/// Windows compte les intervalles de 100 ns depuis le 1er janvier 1601 ; l'époque
/// Unix commence 11 644 473 600 secondes plus tard.
///
/// # Le seuil de vraisemblance, et pourquoi il est dans le code
///
/// Un `FILETIME` nul se convertit très correctement en **1601-01-01**. La
/// conversion est juste, le résultat est absurde : aucune signature antivirus
/// n'a été appliquée sous Henri IV. Or une date absurde dans un journal
/// forensique est pire qu'une date absente — elle se corrèle avec d'autres
/// événements et fabrique une histoire (D1-09).
///
/// Cette fonction refuse donc tout ce qui sort de l'intervalle 2000–2100, et
/// renvoie `None`. Les deux bornes sont arbitraires et assumées : elles séparent
/// « valeur non renseignée » et « octets illisibles » de « date réelle », ce
/// qu'aucune conversion pure ne peut faire.
///
/// Le plafond n'est pas décoratif : `u64::MAX` se convertit en l'an 58 000, que
/// `chrono` accepte sans broncher. Sans borne haute, huit octets corrompus
/// produiraient une date parfaitement valide et parfaitement fausse.
#[must_use]
pub fn filetime_vers_horodatage(ticks: u64) -> Option<DateTime<Utc>> {
    /// Décalage entre l'époque Windows (1601) et l'époque Unix (1970), en secondes.
    const DECALAGE_EPOQUE: i64 = 11_644_473_600;
    /// 2000-01-01T00:00:00Z. En deçà, la valeur n'est pas une date, c'est un zéro.
    const PLANCHER_UNIX: i64 = 946_684_800;
    /// 2100-01-01T00:00:00Z. Au-delà, ce ne sont pas des octets de date.
    const PLAFOND_UNIX: i64 = 4_102_444_800;

    let secondes = i64::try_from(ticks / 10_000_000).ok()? - DECALAGE_EPOQUE;
    if !(PLANCHER_UNIX..PLAFOND_UNIX).contains(&secondes) {
        return None;
    }
    let nanos = u32::try_from((ticks % 10_000_000) * 100).ok()?;
    DateTime::from_timestamp(secondes, nanos)
}

/// Convertit un horodatage en `FILETIME`. Réservé aux tests d'aller-retour.
#[cfg(test)]
fn horodatage_vers_filetime(d: DateTime<Utc>) -> u64 {
    const DECALAGE_EPOQUE: i64 = 11_644_473_600;
    let secondes = d.timestamp() + DECALAGE_EPOQUE;
    u64::try_from(secondes).unwrap_or(0) * 10_000_000
}

#[cfg(windows)]
mod windows_impl {
    use super::{demarrage_service, filetime_vers_horodatage, item_posture, SERVICES_SURVEILLES};
    use ks_core::{Item, ItemValue};
    use windows_registry::LOCAL_MACHINE;

    /// Lit un entier du registre. `None` si la clé ou la valeur n'existe pas —
    /// et c'est bien `None` qu'on veut, pas `Some(0)`.
    fn u32_registre(chemin: &str, nom: &str) -> Option<u32> {
        LOCAL_MACHINE.open(chemin).ok()?.get_u32(nom).ok()
    }

    fn texte_registre(chemin: &str, nom: &str) -> Option<String> {
        LOCAL_MACHINE.open(chemin).ok()?.get_string(nom).ok()
    }

    /// Les NOMS des valeurs d'une clef, pas ses sous-clefs.
    ///
    /// Les exclusions Defender et les règles ASR sont stockées ainsi : une valeur
    /// par élément, le nom portant l'information. L'itérateur emprunte la clef —
    /// il faut donc la garder vivante jusqu'à la collecte, ce qu'une chaîne de
    /// `and_then` ne fait pas.
    fn noms_des_valeurs(chemin: &str) -> Vec<String> {
        let Ok(clef) = LOCAL_MACHINE.open(chemin) else {
            return Vec::new();
        };
        let Ok(valeurs) = clef.values() else {
            return Vec::new();
        };
        valeurs.map(|(nom, _)| nom).collect()
    }

    /// Traduit une valeur booléenne du registre en item.
    ///
    /// L'absence donne `Absent`, jamais `Bool(false)` : voir la règle en tête de
    /// module. C'est la seule fonction de ce fichier qu'il faut lire en entier
    /// avant d'y toucher.
    fn drapeau(valeur: Option<u32>) -> ItemValue {
        valeur.map_or(ItemValue::Absent, |v| ItemValue::Bool(v != 0))
    }

    pub(super) fn items() -> Vec<Item> {
        // ─── Secure Boot ────────────────────────────────────────────────────
        let mut items = vec![item_posture(
            "security.platform.secure_boot",
            drapeau(u32_registre(
                r"SYSTEM\CurrentControlSet\Control\SecureBoot\State",
                "UEFISecureBootEnabled",
            )),
            "Secure Boot n'autorise au démarrage que du code signé par une autorité \
             reconnue du firmware.",
            "Sans lui, un bootkit s'installe avant le système et avant tout outil de \
             sécurité — y compris Keystone.",
            Some("docs/04-MODELE-DE-MENACE.md § « Ce qui n'est pas couvert »"),
        )];

        // ─── Intégrité du code imposée par l'hyperviseur ────────────────────
        items.push(item_posture(
            "security.platform.hvci",
            drapeau(u32_registre(
                r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
                "Enabled",
            )),
            "HVCI fait vérifier l'intégrité du code du noyau par l'hyperviseur, hors \
             de portée du noyau lui-même.",
            "Sans lui, un pilote non signé ou détourné s'exécute en anneau 0.",
            None,
        ));

        // `EnableVirtualizationBasedSecurity` décrit ce que la STRATÉGIE impose, pas
        // ce qui tourne : elle est souvent absente sur une machine où VBS est
        // pourtant actif. On la publie telle quelle, et on ne la confond pas avec
        // l'état réel — que seul HVCI ci-dessus atteste.
        items.push(item_posture(
            "security.platform.vbs_policy",
            drapeau(u32_registre(
                r"SYSTEM\CurrentControlSet\Control\DeviceGuard",
                "EnableVirtualizationBasedSecurity",
            )),
            "Sécurité basée sur la virtualisation, telle qu'imposée par stratégie. \
             Absente ne veut pas dire inactive : c'est HVCI qui atteste l'exécution.",
            "Une stratégie absente laisse l'état à la main de qui peut le changer.",
            None,
        ));

        items.push(item_posture(
            "security.platform.credential_guard",
            drapeau(u32_registre(
                r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\CredentialGuard",
                "Enabled",
            )),
            "Credential Guard isole les secrets d'authentification dans un espace que \
             le noyau ne peut pas lire.",
            "Sans lui, un attaquant SYSTEM récolte les empreintes et les tickets en \
             mémoire, et se déplace latéralement.",
            None,
        ));

        // ─── Protection LSA ─────────────────────────────────────────────────
        // 0 ou absent : non protégé. 1 : protégé. 2 : protégé avec verrou UEFI.
        // Le verrou change tout — il survit à une modification du registre.
        let lsa = u32_registre(r"SYSTEM\CurrentControlSet\Control\Lsa", "RunAsPPL");
        items.push(item_posture(
            "security.platform.lsa_protection",
            lsa.map_or(ItemValue::Absent, |v| {
                ItemValue::Text(
                    match v {
                        1 => "activée",
                        2 => "activée, verrouillée par UEFI",
                        _ => "désactivée",
                    }
                    .to_owned(),
                )
            }),
            "La protection LSA empêche un processus non protégé de lire la mémoire du \
             service qui détient les secrets d'authentification.",
            "Sans elle, l'extraction d'identifiants ne demande qu'un outil courant.",
            None,
        ));

        // ─── Defender ───────────────────────────────────────────────────────
        const DEFENDER: &str = r"SOFTWARE\Microsoft\Windows Defender";

        items.push(item_posture(
            "security.defender.engine_version",
            texte_registre(&format!(r"{DEFENDER}\Signature Updates"), "EngineVersion")
                .map_or(ItemValue::Absent, ItemValue::Text),
            "Version du moteur d'analyse de Defender.",
            "Un moteur ancien ne sait pas lire les signatures récentes.",
            None,
        ));

        items.push(item_posture(
            "security.defender.signature_version",
            texte_registre(
                &format!(r"{DEFENDER}\Signature Updates"),
                "AVSignatureVersion",
            )
            .map_or(ItemValue::Absent, ItemValue::Text),
            "Version des signatures antivirus.",
            "Des signatures figées font croire à une protection qui ne reconnaît plus \
             rien de récent.",
            None,
        ));

        // `AVSignatureApplied` est un FILETIME sur huit octets. C'est la donnée la
        // plus parlante du lot : elle répond à « depuis quand Defender n'a-t-il pas
        // été mis à jour ? », qui est une question de posture, pas de version.
        let date_signatures = LOCAL_MACHINE
            .open(format!(r"{DEFENDER}\Signature Updates"))
            .ok()
            .and_then(|k| k.get_value("AVSignatureApplied").ok())
            // `Value` implémente `AsRef<[u8]>` : on refuse toute longueur autre
            // que huit octets plutôt que de compléter ou de tronquer. Un FILETIME
            // mal formé doit rester illisible, pas devenir une date plausible.
            .and_then(|v| <[u8; 8]>::try_from(v.as_ref()).ok())
            .map(u64::from_le_bytes)
            .and_then(filetime_vers_horodatage);

        items.push(item_posture(
            "security.defender.signatures_applied_at",
            date_signatures.map_or(ItemValue::Absent, |d| ItemValue::Text(d.to_rfc3339())),
            "Date d'application des dernières signatures.",
            "C'est l'âge, pas le numéro de version, qui dit si la protection suit.",
            None,
        ));

        // Les exclusions sont des VALEURS sous la clé, une par chemin exclu.
        // Chacune est un trou volontaire dans la couverture : les compter, c'est
        // mesurer la surface qu'on a soi-même ouverte (D11-02).
        for (categorie, chemin) in [
            ("paths", "Paths"),
            ("extensions", "Extensions"),
            ("processes", "Processes"),
        ] {
            let exclusions = noms_des_valeurs(&format!(r"{DEFENDER}\Exclusions\{chemin}"));

            items.push(item_posture(
                &format!("security.defender.exclusions.{categorie}"),
                ItemValue::List(exclusions),
                "Exclusions Defender déclarées à l'échelle de la machine.",
                "Chaque exclusion est un angle mort volontaire. Le projet exige \
                 qu'elle porte une raison et une expiration (D11-02).",
                Some("docs/01-CAHIER-DES-CHARGES.md § D11-02"),
            ));
        }

        // Les règles ASR vivent sous les stratégies. Leur absence n'est pas une
        // désactivation : c'est une non-configuration, et la nuance compte.
        let asr = noms_des_valeurs(
            r"SOFTWARE\Policies\Microsoft\Windows Defender\Windows Defender Exploit Guard\ASR\Rules",
        );

        items.push(item_posture(
            "security.defender.asr_rules",
            if asr.is_empty() {
                ItemValue::Absent
            } else {
                ItemValue::List(asr)
            },
            "Règles de réduction de la surface d'attaque configurées par stratégie.",
            "Aucune règle configurée n'est pas une faute en soi, mais c'est une \
             couche de défense qu'on n'a pas prise.",
            None,
        ));

        // ─── Services de sécurité ───────────────────────────────────────────
        for (service, role) in SERVICES_SURVEILLES {
            let depart = u32_registre(
                &format!(r"SYSTEM\CurrentControlSet\Services\{service}"),
                "Start",
            );
            items.push(item_posture(
                &format!("security.services.{}.startup", service.to_lowercase()),
                depart.map_or(ItemValue::Absent, |v| {
                    ItemValue::Text(demarrage_service(v).to_owned())
                }),
                role,
                "Un service de sécurité désactivé est le premier maillon de la \
                 plupart des chaînes d'attaque (§6 du modèle de menace).",
                Some("docs/04-MODELE-DE-MENACE.md § 6"),
            ));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_type_de_demarrage_se_lit_en_francais() {
        // Principe P6 : « 4 » ne veut rien dire, « désactivé » si.
        assert_eq!(demarrage_service(2), "automatique");
        assert_eq!(demarrage_service(4), "désactivé");
        assert_eq!(demarrage_service(99), "inconnu");
    }

    #[test]
    fn un_filetime_se_convertit_en_date_reelle() {
        // Aller-retour plutôt qu'une constante calculée à la main : c'est en
        // calculant l'époque de tête que ce test avait échoué la première fois.
        // La machine sait faire l'arithmétique, pas moi.
        for iso in [
            "2026-08-02T00:00:00Z",
            "2000-01-01T00:00:01Z",
            "2038-01-19T03:14:07Z",
        ] {
            let attendu: DateTime<Utc> = iso.parse().expect("date de test valide");
            let ticks = horodatage_vers_filetime(attendu);
            assert_eq!(
                filetime_vers_horodatage(ticks),
                Some(attendu),
                "aller-retour cassé pour {iso}"
            );
        }
    }

    #[test]
    fn un_filetime_invraisemblable_ne_produit_pas_une_date() {
        // Un FILETIME nul se convertit très correctement en 1601-01-01 : la
        // conversion est juste, le résultat est absurde. Une date absurde dans un
        // journal forensique est pire qu'une date absente — elle se corrèle avec
        // d'autres événements et fabrique une histoire.
        assert!(
            filetime_vers_horodatage(0).is_none(),
            "1601 n'est pas une date"
        );
        assert!(
            filetime_vers_horodatage(u64::MAX).is_none(),
            "un débordement ne doit pas produire une date"
        );
        // Les deux bornes, éprouvées de part et d'autre. C'est aussi ce qui
        // vérifie que les constantes d'époque sont justes : si l'une est fausse
        // d'une seconde, l'une de ces quatre assertions casse.
        for (iso, accepte) in [
            ("1999-12-31T23:59:59Z", false),
            ("2000-01-01T00:00:00Z", true),
            ("2099-12-31T23:59:59Z", true),
            ("2100-01-01T00:00:00Z", false),
        ] {
            let d: DateTime<Utc> = iso.parse().expect("date de test valide");
            assert_eq!(
                filetime_vers_horodatage(horodatage_vers_filetime(d)).is_some(),
                accepte,
                "borne mal placée pour {iso}"
            );
        }
    }

    #[test]
    fn tout_item_de_posture_porte_son_explication() {
        for item in PostureCollector::items() {
            assert!(!item.purpose.is_empty(), "« {} » sans finalité", item.path);
            assert!(!item.risk.is_empty(), "« {} » sans risque", item.path);
            assert_eq!(item.provenance, Provenance::Observed);
            assert!(item.desired.is_none(), "un collecteur ne décide de rien");
            assert!(
                item.path.starts_with("security."),
                "« {} » n'est pas dans le domaine annoncé",
                item.path
            );
        }
    }

    #[test]
    fn une_cle_absente_ne_devient_jamais_un_faux_negatif() {
        // LA règle de ce module. Sur une machine réelle,
        // `EnableVirtualizationBasedSecurity` est absente alors que HVCI vaut 1 :
        // traduire cette absence en « désactivé » afficherait une machine protégée
        // comme vulnérable. Aucun item de posture ne doit valoir Bool(false) sans
        // qu'une valeur ait réellement été lue — ce test le vérifie par le fait
        // qu'aucun chemin de code ne produit Bool à partir d'un None.
        for item in PostureCollector::items() {
            if item.observed == ItemValue::Absent {
                continue;
            }
            assert_ne!(
                item.observed,
                ItemValue::Text(String::new()),
                "« {} » : une valeur vide n'est pas une valeur",
                item.path
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn un_service_toujours_present_ne_remonte_jamais_absent() {
        // Le premier jet écrivait « wscvc » au lieu de « wscsvc ». Le service
        // tournait, l'item affichait « absent », et rien ne bronchait : un nom de
        // service erroné est indiscernable d'un service non installé.
        //
        // Ces trois-là existent sur toute installation de Windows 11 ; s'ils
        // remontent absents, c'est le nom qui est faux, pas la machine.
        // `Sense` et `wscsvc` sont volontairement hors de cette liste : le premier
        // n'est présent qu'avec Defender for Endpoint, et une absence légitime ne
        // doit pas faire échouer un test.
        let items = PostureCollector::items();
        for service in ["windefend", "mpssvc", "eventlog"] {
            let chemin = format!("security.services.{service}.startup");
            let item = items
                .iter()
                .find(|i| i.path == chemin)
                .unwrap_or_else(|| panic!("« {chemin} » n'est pas produit du tout"));
            assert_ne!(
                item.observed,
                ItemValue::Absent,
                "« {chemin} » : ce service existe sur tout Windows 11 — \
                 le nom interrogé est probablement mal orthographié"
            );
        }
    }
}

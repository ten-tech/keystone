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
//! ## Deux sources, et la différence entre elles est l'information
//!
//! Le **registre** donne la configuration : Secure Boot, la stratégie VBS et
//! HVCI, la protection LSA, les versions et exclusions de Defender, le type de
//! démarrage des services, le pare-feu, le firmware, l'horloge.
//!
//! **WMI** donne l'exécution, et lui seul. Aucune valeur de registre n'atteste
//! que VBS tourne, que le noyau sécurisé a démarré, ou que la protection en
//! temps réel est active. Ces états vivent dans `Win32_DeviceGuard` et
//! `MSFT_MpComputerStatus` — voir [`crate::etat_effectif`] et l'ADR-0005.
//!
//! Les chemins le disent : `vbs_policy` contre `vbs_running`, `hvci_policy`
//! contre `hvci_running`. **Leur divergence est ce qui vaut d'être regardé** —
//! une configuration sans exécution décrit une machine qu'on croit protégée et
//! qui ne l'est pas.
//!
//! Ni l'une ni l'autre source n'exige un bloc `unsafe` : la bibliothèque WMI
//! expose une API sûre de bout en bout pour ce qu'on en fait. Ce qui l'exigerait
//! — l'usure SMART, par `DeviceIoControl` — n'est pas ici, et attend sa propre
//! décision.
//!
//! Le TPM et BitLocker par volume, eux, ne butent pas sur une API mais sur une
//! liste de contrôle d'accès : mesurés en accès refusé sans élévation, ils
//! appartiennent au broker, donc à la Phase 2.
//!
//! ## La règle qui gouverne tout ce fichier
//!
//! **Une clé absente n'est pas « désactivé ». Une clé refusée non plus.**
//!
//! Constaté sur une machine réelle : `EnableVirtualizationBasedSecurity` est
//! absente alors que HVCI vaut 1 — la protection tourne, elle n'est simplement pas
//! *imposée par stratégie*. Traduire cette absence en `false` afficherait « VBS
//! désactivé » sur une machine protégée, c'est-à-dire un faux négatif de sécurité :
//! précisément ce qu'un outil de posture n'a pas le droit de produire.
//!
//! Le pendant exact vaut pour le refus d'accès. Les exclusions de Defender vivent
//! sous une clé protégée par ACL, et la CLI ne s'exécute pas élevée (SEC-01) :
//! traduire ce refus en liste vide afficherait « 0 exclusion » sur une machine où
//! l'on n'a rien pu regarder. D'où [`Lecture`], qui porte **trois** états, et
//! [`ItemValue::Illisible`], qui n'est ni une valeur ni une absence.

use chrono::{DateTime, Utc};
use ks_core::{Item, ItemValue};

use crate::jetons::{
    DemarrageService, EtatProtectionLsa, EtatVbs, IntegriteCode, ProtectionVerrouillable,
    ServiceHyperviseur, TableDeCodes,
};

// `Domain`, `Nature` et `Provenance` ne servent qu'à fabriquer un item, ce que
// seule la branche Windows fait : hors Windows, ce collecteur ne produit rien
// plutôt que d'inventer des protections qui n'existent pas ailleurs.
#[cfg(windows)]
use ks_core::{Domain, Nature, Provenance};

/// Le jeton du type de démarrage d'un service Windows.
///
/// La valeur vient de `HKLM\SYSTEM\CurrentControlSet\Services\<nom>\Start`. On
/// ne publie ni le chiffre — « 4 » ne veut rien dire pour un humain — ni la
/// phrase française, qui serait comparée à chaque scan et qu'une relecture de
/// style ferait basculer en écart (ADR-0015). On publie le **jeton**, et le
/// libellé vit dans `ks_cli::lisible`.
#[must_use]
pub fn demarrage_service(valeur: u32) -> String {
    DemarrageService::depuis_code(valeur).jeton()
}

/// Ce qu'une tentative de lecture du registre peut donner. **Trois états.**
///
/// Le troisième est celui qui manquait, et son absence coûtait cher. Les
/// exclusions de Defender vivent sous une clé protégée par ACL : mesuré sur cette
/// machine, en session non élevée — c'est-à-dire dans les conditions où la CLI
/// s'exécute réellement (SEC-01) — son ouverture renvoie « accès refusé ». Le
/// premier jet avalait ce refus dans le même `Vec::new()` qu'une clé vide, et
/// publiait sereinement « 0 élément(s) » sur une machine où l'on n'avait rien pu
/// regarder.
///
/// C'est le défaut `wscvc` à l'identique, sur l'item que le §6 du modèle de menace
/// place au deuxième rang de valeur. Et il était invisible en intégration
/// continue, où le porteur d'exécution est administrateur : la lecture y réussit,
/// l'écart n'y est pas reproductible.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Lecture<T> {
    /// La valeur existe et a été lue.
    Trouvee(T),
    /// La clé ou la valeur n'existe pas sur cette machine.
    ///
    /// C'est le défaut, et c'est délibéré : une structure fabriquée sans lecture
    /// ne doit contenir que des absences, jamais des zéros.
    #[default]
    Absente,
    /// La lecture a été refusée. Ce n'est pas une absence : c'est un aveu.
    Refusee,
}

/// Ce qu'on dit à l'utilisateur quand une clé lui est refusée.
const REFUS: &str = "accès refusé sans élévation";

/// `HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)`, soit `0x8007_0005`.
///
/// C'est le seul code qu'on distingue. Tous les autres échecs — clé absente en
/// tête — se rangent en [`Lecture::Absente`], parce qu'ils décrivent
/// effectivement une machine où la chose n'existe pas.
#[cfg(windows)]
pub(crate) const ACCES_REFUSE: i32 = -2_147_024_891;

/// Range un résultat du registre dans l'un des trois états de [`Lecture`].
///
/// Remontée au niveau du module, et `pub(crate)`, parce que le collecteur de
/// virtualisation en a besoin mot pour mot : il avalait lui aussi un refus dans
/// un `Vec::new()`, et publiait « 0 distribution » là où il n'avait rien pu
/// lire. Une seconde copie de ces trois lignes aurait fait diverger les deux
/// classements au premier changement.
#[cfg(windows)]
pub(crate) fn classer<T>(resultat: windows_registry::Result<T>) -> Lecture<T> {
    match resultat {
        Ok(v) => Lecture::Trouvee(v),
        Err(e) if e.code().0 == ACCES_REFUSE => Lecture::Refusee,
        Err(_) => Lecture::Absente,
    }
}

/// Traduit un drapeau du registre.
///
/// L'absence ne devient **jamais** `false` : c'est la règle en tête de module, et
/// la seule chose à relire avant de toucher à cette fonction.
#[must_use]
pub fn drapeau(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Trouvee(v) => ItemValue::Bool(*v != 0),
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
    }
}

/// Le jeton d'une protection dont le verrou UEFI est optionnel — **composite**.
///
/// La table est celle de [`ProtectionVerrouillable`] : elle porte l'état *et* le
/// verrou dans une seule valeur. Seul Credential Guard s'en sert encore ; la
/// protection LSA, elle, se déplie en deux items (ADR-0015, point 4) et passe
/// par [`etat_protection_lsa`] et [`verrou_uefi_protection_lsa`].
///
/// Le code inconnu se **nomme**, comme dans [`demarrage_service`] : le faire
/// tomber dans « inactive » ferait passer un octet parasite pour un constat.
#[must_use]
pub fn protection_verrouillable(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
        Lecture::Trouvee(n) => ItemValue::Text(ProtectionVerrouillable::depuis_code(*n).jeton()),
    }
}

/// Le jeton de l'**état** de la protection LSA, son verrou mis à part.
///
/// Première moitié du dépliage exigé par le principe P6 : `activée, sans verrou
/// UEFI` portait deux faits dans une seule chaîne, donc on ne pouvait pas
/// déclarer « je veux la protection LSA active » sans se prononcer sur le
/// verrou. Les codes 1 et 2 donnent ici le même état.
#[must_use]
pub fn etat_protection_lsa(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
        Lecture::Trouvee(n) => ItemValue::Text(
            EtatProtectionLsa::depuis(ProtectionVerrouillable::depuis_code(*n)).jeton(),
        ),
    }
}

/// Le verrou UEFI de la protection LSA, seconde moitié du dépliage.
///
/// Un booléen, parce que la question est fermée : le verrou est posé, ou il ne
/// l'est pas. Un code hors table ne répond **ni oui ni non** — la valeur reste
/// alors un aveu, jamais un « pas de verrou » qui serait un faux positif sur la
/// contre-mesure qui garde les identifiants.
#[must_use]
pub fn verrou_uefi_protection_lsa(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
        Lecture::Trouvee(n) => ProtectionVerrouillable::depuis_code(*n)
            .verrou_uefi()
            .map_or_else(
                || ItemValue::illisible(&format!("code de protection non documenté ({n})")),
                ItemValue::Bool,
            ),
    }
}

/// Traduit un texte lu dans le registre.
#[must_use]
pub fn texte(lecture: Lecture<String>) -> ItemValue {
    match lecture {
        Lecture::Trouvee(s) => ItemValue::Text(s),
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
    }
}

/// Traduit une liste lue dans le registre.
///
/// Une liste vide reste une liste vide — c'est un constat légitime. Seul le refus
/// devient illisible.
#[must_use]
pub fn liste(lecture: Lecture<Vec<String>>) -> ItemValue {
    match lecture {
        Lecture::Trouvee(v) => ItemValue::List(v),
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
    }
}

/// Le jeton de `VirtualizationBasedSecurityStatus`, lu dans `Win32_DeviceGuard`.
///
/// La table est celle d'[`EtatVbs`], et **c'est le code 1 qui justifie toute
/// cette lecture** : une machine où VBS est configuré sans tourner — pilote
/// incompatible, refus côté hyperviseur — porte exactement la même
/// configuration au registre qu'une machine protégée. Le registre les confond ;
/// cette valeur les sépare.
#[must_use]
pub fn etat_vbs(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
        Lecture::Trouvee(n) => ItemValue::Text(EtatVbs::depuis_code(*n).jeton()),
    }
}

/// Codes de `SecurityServicesRunning` et `SecurityServicesConfigured`.
///
/// Les deux champs partagent la même table, documentée par Microsoft. On ne
/// nomme que ce dont on se sert : ajouter les autres sans les publier
/// encombrerait sans rien apporter.
pub mod service_vbs {
    /// Credential Guard.
    pub const CREDENTIAL_GUARD: u32 = 1;
    /// Intégrité mémoire, alias HVCI.
    pub const INTEGRITE_MEMOIRE: u32 = 2;
}

/// Un service protégé par l'hyperviseur figure-t-il dans la liste ?
///
/// Contrairement au registre, **un « à l'arrêt » est ici un constat**, pas une
/// supposition : la liste a été lue, et le code n'y est pas. C'est toute la
/// différence entre « je n'ai rien trouvé » et « j'ai regardé, ce n'est pas là ».
/// La valeur est **textuelle**, et pas un booléen, parce qu'un booléen s'affiche
/// « activé » — le vocabulaire d'un interrupteur. Or la question posée ici est
/// « est-ce que ça tourne ? », à laquelle « activé » répond de travers : c'est
/// précisément la confusion entre configuration et exécution que ce module
/// existe pour lever.
#[must_use]
pub fn service_vbs_present(lecture: &Lecture<Vec<u32>>, code: u32) -> ItemValue {
    match lecture {
        Lecture::Trouvee(codes) => {
            ItemValue::Text(ServiceHyperviseur::depuis_presence(codes.contains(&code)).jeton())
        }
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
    }
}

/// Le jeton de `CodeIntegrityPolicyEnforcementStatus`.
///
/// La table est celle d'[`IntegriteCode`] : **0** Off, **1** Audit, **2**
/// Enforced. Le mode audit journalise sans bloquer — même nuance que pour les
/// règles ASR, et même piège : le compter comme une protection serait un faux
/// positif.
#[must_use]
pub fn application_integrite_code(lecture: &Lecture<u32>) -> ItemValue {
    match lecture {
        Lecture::Absente => ItemValue::Absent,
        Lecture::Refusee => ItemValue::illisible(REFUS),
        Lecture::Trouvee(n) => ItemValue::Text(IntegriteCode::depuis_code(*n).jeton()),
    }
}

/// Normalise une date de firmware quand, et seulement quand, elle est certaine.
///
/// Les firmwares écrivent leur date en `M/J/AAAA`, sauf ceux qui écrivent
/// `J/M/AAAA`. Aucune spécification ne tranche, et la valeur relevée sur la
/// machine de référence — `10/21/2024` — n'est lisible que parce que 21 dépasse
/// le nombre de mois.
///
/// D'où la règle : on ne convertit que **l'ambiguïté levée**. `03/04/2024` reste
/// tel quel, parce que le 3 avril et le 4 mars sont également plausibles et qu'un
/// mois d'écart sur l'âge d'un firmware n'est pas une approximation, c'est une
/// invention. Le champ brut est publié dans tous les cas.
#[must_use]
pub fn date_firmware_certaine(brut: &str) -> Option<String> {
    let morceaux: Vec<&str> = brut.trim().split('/').collect();
    let [a, b, annee] = morceaux[..] else {
        return None;
    };
    let (a, b, annee) = (
        a.parse::<u32>().ok()?,
        b.parse::<u32>().ok()?,
        annee.parse::<u32>().ok()?,
    );
    if !(1970..=2200).contains(&annee) {
        return None;
    }
    // Un seul des deux nombres peut être un mois : l'ordre est alors déterminé.
    let (mois, jour) = match (a <= 12, b <= 12) {
        (true, false) => (a, b),
        (false, true) => (b, a),
        _ => return None,
    };
    if !(1..=31).contains(&jour) {
        return None;
    }
    Some(format!("{annee:04}-{mois:02}-{jour:02}"))
}

/// Une règle de pare-feu est-elle une autorisation entrante active ?
///
/// Le format de la valeur n'est contractuel nulle part : on lit les champs qu'on
/// reconnaît et on ignore le reste, plutôt que d'exiger une forme exacte qu'une
/// mise à jour de Windows briserait en silence.
///
/// C'est le seul sous-ensemble qui porte un signal : sur les 649 règles de la
/// machine de référence, l'écrasante majorité est sortante ou inactive. Publier
/// « 649 règles » ne dirait rien à personne (principe P6).
#[must_use]
pub fn est_autorisation_entrante_active(regle: &str) -> bool {
    let mut entrante = false;
    let mut autorise = false;
    let mut active = false;
    for champ in regle.split('|') {
        match champ.split_once('=') {
            Some(("Dir", "In")) => entrante = true,
            Some(("Action", "Allow")) => autorise = true,
            Some(("Active", v)) => active = v.eq_ignore_ascii_case("true"),
            _ => {}
        }
    }
    entrante && autorise && active
}

/// Traduit le mode d'une règle de réduction de surface d'attaque.
///
/// Les modes sont documentés : 0 éteinte, 1 bloque, 2 audit, 5 non configurée,
/// 6 avertit. La nuance décide de tout — une règle en **audit** ne bloque rien.
/// Compter « 12 règles configurées » sans les modes laisserait croire à douze
/// protections là où il peut n'y en avoir aucune.
#[must_use]
pub fn mode_asr(valeur: &str) -> &'static str {
    match valeur.trim() {
        "0" => "éteinte",
        "1" => "bloque",
        "2" => "audit",
        "5" => "non configurée",
        "6" => "avertit",
        _ => "mode inconnu",
    }
}

/// Les services dont l'arrêt est un signal, et la raison de les surveiller.
///
/// Liste volontairement courte : chacun est cité au §6 du modèle de menace ou
/// couvre une brique de posture. Un service désactivé ici n'est pas une curiosité,
/// c'est le premier maillon de la plupart des chaînes d'attaque.
#[cfg(windows)]
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

/// Fabrique un item de posture, avec sa vocation, sa finalité et son risque.
///
/// `nature` est un paramètre **obligatoire**, au même titre que `but` et
/// `risque` (principe P6, ADR-0009). Le préfixe `security.` ne la décide pas :
/// ce module produit les quatre natures, et `security.firmware.version` est un
/// constat à trois lignes d'un réglage.
#[cfg(windows)]
pub(crate) fn item_posture(
    chemin: &str,
    nature: Nature,
    valeur: ItemValue,
    but: &str,
    risque: &str,
    reference: Option<&str>,
) -> Item {
    Item {
        path: chemin.to_owned(),
        domain: Domain::Security,
        nature,
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
    use super::{
        application_integrite_code, classer, date_firmware_certaine, demarrage_service, drapeau,
        est_autorisation_entrante_active, etat_protection_lsa, etat_vbs, filetime_vers_horodatage,
        item_posture, liste, mode_asr, protection_verrouillable, service_vbs, service_vbs_present,
        texte, verrou_uefi_protection_lsa, Lecture, Nature, ACCES_REFUSE, SERVICES_SURVEILLES,
    };
    use ks_core::{Item, ItemValue};
    use windows_registry::{Type, LOCAL_MACHINE};

    /// Lit un entier. Le refus d'accès ne se confond pas avec l'absence.
    fn u32_registre(chemin: &str, nom: &str) -> Lecture<u32> {
        match LOCAL_MACHINE.open(chemin) {
            Ok(clef) => classer(clef.get_u32(nom)),
            Err(e) if e.code().0 == ACCES_REFUSE => Lecture::Refusee,
            Err(_) => Lecture::Absente,
        }
    }

    fn texte_registre(chemin: &str, nom: &str) -> Lecture<String> {
        match LOCAL_MACHINE.open(chemin) {
            Ok(clef) => classer(clef.get_string(nom)),
            Err(e) if e.code().0 == ACCES_REFUSE => Lecture::Refusee,
            Err(_) => Lecture::Absente,
        }
    }

    /// Les valeurs d'une clef — leur nom et leur contenu textuel.
    ///
    /// Les exclusions Defender et les règles ASR sont stockées ainsi : une valeur
    /// par élément, le nom portant l'information et, pour ASR, le contenu portant
    /// le mode. L'itérateur emprunte la clef, il faut donc la garder vivante
    /// jusqu'à la collecte — ce qu'une chaîne de `and_then` ne ferait pas.
    fn valeurs_de(chemin: &str) -> Lecture<Vec<(String, String)>> {
        let clef = match LOCAL_MACHINE.open(chemin) {
            Ok(c) => c,
            Err(e) if e.code().0 == ACCES_REFUSE => return Lecture::Refusee,
            Err(_) => return Lecture::Absente,
        };
        let Ok(valeurs) = clef.values() else {
            return Lecture::Absente;
        };
        // Le contenu peut être une chaîne ou un entier selon la clé ; on
        // n'interprète pas ici, on rend du texte et on laisse la traduction aux
        // fonctions pures, testables sans registre.
        Lecture::Trouvee(
            valeurs
                .map(|(nom, valeur)| {
                    // Les règles ASR sont documentées en `REG_SZ`, mais le registre
                    // accepte aussi bien un `REG_DWORD` : on couvre les deux plutôt
                    // que de parier sur une machine qu'on n'a pas sous la main.
                    let contenu = match valeur.ty() {
                        Type::U32 => u32::try_from(valeur).map(|n| n.to_string()),
                        _ => String::try_from(valeur),
                    }
                    .unwrap_or_default();
                    (nom, contenu)
                })
                .collect(),
        )
    }

    /// Les seuls noms des valeurs, quand le contenu ne porte rien.
    fn noms_des_valeurs(chemin: &str) -> Lecture<Vec<String>> {
        match valeurs_de(chemin) {
            Lecture::Trouvee(v) => Lecture::Trouvee(v.into_iter().map(|(n, _)| n).collect()),
            Lecture::Absente => Lecture::Absente,
            Lecture::Refusee => Lecture::Refusee,
        }
    }

    pub(super) fn items() -> Vec<Item> {
        // ─── Secure Boot ────────────────────────────────────────────────────
        let mut items = vec![item_posture(
            "security.platform.secure_boot",
            // **Objectif, et pas réglage** : on peut vouloir Secure Boot actif,
            // mais aucun verbe du broker n'écrit dans le firmware. Le classer
            // réglage promettrait une convergence qui n'arrivera jamais.
            Nature::Objectif,
            drapeau(&u32_registre(
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
        //
        // `Scenarios\…\Enabled` est la clé que Microsoft documente pour ACTIVER
        // HVCI, jamais pour en vérifier l'exécution : la page « Validate enabled
        // VBS and memory integrity features » ne cite aucune valeur de registre et
        // renvoie à `Win32_DeviceGuard`. Le chemin de l'item porte donc `_policy`,
        // comme VBS juste en dessous. Les deux sont au même niveau : deux
        // configurations. Une machine où HVCI est configuré mais où le noyau
        // sécurisé n'a pas démarré — pilote incompatible, refus côté hyperviseur —
        // afficherait « activé » sans être protégée.
        const DEVICE_GUARD: &str = r"SYSTEM\CurrentControlSet\Control\DeviceGuard";

        items.push(item_posture(
            "security.platform.hvci_policy",
            Nature::Reglage,
            drapeau(&u32_registre(
                &format!(r"{DEVICE_GUARD}\Scenarios\HypervisorEnforcedCodeIntegrity"),
                "Enabled",
            )),
            "Intégrité du code imposée par l'hyperviseur, telle que CONFIGURÉE. \
             L'état réellement en vigueur se lit ailleurs et n'est pas encore collecté.",
            "Sans elle, un pilote non signé ou détourné s'exécute en anneau 0.",
            None,
        ));

        // Le verrou UEFI d'HVCI, qui porte la même nuance que celui de LSA : une
        // configuration verrouillée survit à une réécriture du registre.
        items.push(item_posture(
            "security.platform.hvci_uefi_lock",
            // Réglage, malgré une asymétrie assumée : le verrou se pose, il ne
            // se retire pas. Sa convergence sera donc à sens unique, ce qui
            // relève du verbe, pas de la vocation de l'item (ADR-0009).
            Nature::Reglage,
            drapeau(&u32_registre(
                &format!(r"{DEVICE_GUARD}\Scenarios\HypervisorEnforcedCodeIntegrity"),
                "Locked",
            )),
            "Verrou UEFI de la configuration HVCI.",
            "Sans verrou, un attaquant administrateur désactive la protection par le \
             registre et un redémarrage.",
            None,
        ));

        items.push(item_posture(
            "security.platform.vbs_policy",
            // La STRATÉGIE s'écrit au registre — donc un réglage. Son exécution,
            // trente lignes plus bas, ne s'écrit nulle part : c'est un objectif.
            Nature::Reglage,
            drapeau(&u32_registre(
                DEVICE_GUARD,
                "EnableVirtualizationBasedSecurity",
            )),
            "Sécurité basée sur la virtualisation, telle qu'imposée par stratégie. \
             Absente ne veut pas dire inactive : aucune valeur de registre n'atteste \
             l'exécution.",
            "Une stratégie absente laisse l'état à la main de qui peut le changer.",
            None,
        ));

        // ─── Protection LSA et Credential Guard ─────────────────────────────
        //
        // Les deux se lisent dans la même clé `Lsa`, avec la même table de codes,
        // toutes deux documentées. `Scenarios\CredentialGuard\Enabled`, lu par le
        // premier jet, n'apparaît dans aucune procédure Microsoft : l'item était
        // juste par accident.
        const LSA: &str = r"SYSTEM\CurrentControlSet\Control\Lsa";

        // `RunAsPPL` porte DEUX faits — la protection est active, et son verrou
        // UEFI est posé ou non — que la valeur unique « activée, sans verrou
        // UEFI » mélangeait. Le principe P6 exige qu'un composite se déplie en
        // ses composantes exactes, et le dépliage rend enfin déclarable « je
        // veux la protection LSA active » sans obliger à se prononcer sur le
        // verrou (ADR-0015, point 4). Une seule lecture du registre alimente
        // les deux items.
        let run_as_ppl = u32_registre(LSA, "RunAsPPL");

        items.push(item_posture(
            "security.platform.lsa_protection",
            Nature::Reglage,
            etat_protection_lsa(&run_as_ppl),
            "La protection LSA empêche un processus non protégé de lire la mémoire du \
             service qui détient les secrets d'authentification.",
            "Sans elle, l'extraction d'identifiants ne demande qu'un outil courant.",
            None,
        ));

        items.push(item_posture(
            "security.platform.lsa_protection_uefi_lock",
            // **Objectif, et non réglage** — à l'inverse d'`hvci_uefi_lock`,
            // qui est un réglage. L'asymétrie est celle de l'ADR-0015 : le
            // verrou de la protection LSA n'a pas de valeur de registre
            // propre, il est un effet du code écrit dans `RunAsPPL`. Aucun
            // verbe ne l'écrit SEUL, donc on peut le vouloir sans savoir le
            // faire converger.
            Nature::Objectif,
            verrou_uefi_protection_lsa(&run_as_ppl),
            "Verrou UEFI de la protection LSA. Déplié de l'état lui-même : une valeur \
             qui porte deux faits ne se déclare pas.",
            "Sans verrou, la protection cède à une écriture du registre et un \
             redémarrage.",
            None,
        ));

        items.push(item_posture(
            "security.platform.credential_guard",
            Nature::Reglage,
            protection_verrouillable(&u32_registre(LSA, "LsaCfgFlags")),
            "Credential Guard isole les secrets d'authentification dans un espace que \
             le noyau ne peut pas lire. **Absent ne veut pas dire éteint** : depuis \
             Windows 11 22H2, il s'active par défaut sur les machines éligibles sans \
             qu'aucune valeur ne soit écrite.",
            "Sans lui, un attaquant SYSTEM récolte les empreintes et les tickets en \
             mémoire, et se déplace latéralement.",
            None,
        ));

        // ─── Defender ───────────────────────────────────────────────────────
        const DEFENDER: &str = r"SOFTWARE\Microsoft\Windows Defender";

        items.push(item_posture(
            "security.defender.engine_version",
            // Defender met à jour son moteur seul, plusieurs fois par mois. Une
            // version déclarée par égalité serait en écart au premier correctif.
            Nature::Mesure,
            texte(texte_registre(
                &format!(r"{DEFENDER}\Signature Updates"),
                "EngineVersion",
            )),
            "Version du moteur d'analyse de Defender.",
            "Un moteur ancien ne sait pas lire les signatures récentes.",
            None,
        ));

        items.push(item_posture(
            "security.defender.signature_version",
            Nature::Mesure,
            texte(texte_registre(
                &format!(r"{DEFENDER}\Signature Updates"),
                "AVSignatureVersion",
            )),
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
            // Elle change tous les jours — c'est même ce qu'on lui demande.
            // Ce qu'on voudra un jour contraindre est son ÂGE, ce qui suppose
            // un vocabulaire (« plus récent que sept jours ») qui n'existe pas.
            Nature::Mesure,
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
            // La clé est protégée par ACL : sans élévation, l'ouverture est
            // refusée. C'est le cas nominal du produit, pas un cas limite —
            // et « 0 élément » y serait un mensonge.
            items.push(item_posture(
                &format!("security.defender.exclusions.{categorie}"),
                // Réglage, bien que ces trois items soient les seuls illisibles
                // en permanence sur la machine de référence. **L'illisibilité
                // est une propriété de la lecture, pas de l'item** : une
                // exclusion se déclare, se veut, et s'écrira. Ce qu'on n'a pas
                // su lire aujourd'hui ne change pas ce à quoi l'item sert.
                Nature::Reglage,
                liste(noms_des_valeurs(&format!(
                    r"{DEFENDER}\Exclusions\{chemin}"
                ))),
                "Exclusions Defender déclarées à l'échelle de la machine. La clé exige \
                 des privilèges élevés : sans eux, l'item dit « illisible », jamais zéro.",
                "Chaque exclusion est un angle mort volontaire. Le projet exige \
                 qu'elle porte une raison et une expiration (D11-02).",
                Some("docs/01-CAHIER-DES-CHARGES.md § D11-02"),
            ));
        }

        // Les règles ASR vivent sur DEUX branches, et le premier jet n'en lisait
        // qu'une — la branche de stratégie, absente de cette machine, tandis que
        // la branche locale (celle qu'alimentent `Add-MpPreference` et Intune)
        // existait bel et bien. Une règle posée localement était donc invisible.
        //
        // Le mode se publie avec la règle : une règle en audit ne bloque rien, et
        // la compter comme une protection serait un faux positif.
        for (source, racine) in [
            ("policy", r"SOFTWARE\Policies\Microsoft\Windows Defender"),
            ("local", r"SOFTWARE\Microsoft\Windows Defender"),
        ] {
            let chemin = format!(r"{racine}\Windows Defender Exploit Guard\ASR\Rules");
            let regles = match valeurs_de(&chemin) {
                Lecture::Trouvee(v) => Lecture::Trouvee(
                    v.into_iter()
                        .map(|(guid, mode)| format!("{guid} = {}", mode_asr(&mode)))
                        .collect(),
                ),
                Lecture::Absente => Lecture::Absente,
                Lecture::Refusee => Lecture::Refusee,
            };

            // La branche `policy` vit sous `HKLM\SOFTWARE\Policies\…` : une
            // valeur qu'on y lit n'a pas d'autre auteur possible qu'une
            // autorité, et `politique::marquer` la porte jusqu'à la provenance
            // (ADR-0018). L'information était produite ici même — deux chemins,
            // deux origines — et jetée à la ligne suivante.
            //
            // La décision se lit dans l'expression qui construit l'item, à côté
            // du chemin qui la justifie : `marquer` ne marque que ce qui figure
            // dans `politique::CHEMINS` ET dont une valeur a été lue, donc la
            // branche `local` en ressort inchangée sans qu'aucune condition
            // s'écrive ici.
            items.push(crate::politique::marquer(item_posture(
                &format!("security.defender.asr_rules.{source}"),
                Nature::Reglage,
                liste(regles),
                "Règles de réduction de la surface d'attaque, avec leur mode. Une \
                 règle en audit journalise sans bloquer.",
                "Aucune règle configurée n'est pas une faute en soi, mais c'est une \
                 couche de défense qu'on n'a pas prise.",
                None,
            )));
        }

        // ─── État EFFECTIF, par WMI (ADR-0005) ──────────────────────────────
        //
        // Ce que les items `_policy` ci-dessus ne peuvent pas dire. Ils décrivent
        // ce qui est demandé ; ceux-ci, ce qui tourne. Leur divergence est
        // précisément l'information : une configuration sans exécution décrit une
        // machine qu'on croit protégée et qui ne l'est pas.
        let (plateforme, defender) = crate::etat_effectif::lire();

        items.push(item_posture(
            "security.platform.vbs_running",
            // Le cas qui justifie l'existence d'`Objectif`. On peut vouloir VBS
            // en cours d'exécution, et le suivre ; aucun verbe ne l'écrit —
            // l'exécution dépend du matériel, des pilotes et de l'hyperviseur.
            Nature::Objectif,
            etat_vbs(&plateforme.vbs),
            "Sécurité basée sur la virtualisation, telle qu'elle TOURNE. À lire \
             avec `vbs_policy` : « configurée, mais pas en cours d'exécution » est \
             un état réel, et le registre ne sait pas le distinguer de l'exécution.",
            "Une VBS configurée qui ne démarre pas — pilote incompatible, refus de \
             l'hyperviseur — laisse la machine sans aucune des protections qu'on \
             croit avoir activées.",
            Some("docs/adr/0005-lecture-detat-effectif.md"),
        ));

        items.push(item_posture(
            "security.platform.hvci_running",
            Nature::Objectif,
            service_vbs_present(&plateforme.services_actifs, service_vbs::INTEGRITE_MEMOIRE),
            "Intégrité mémoire réellement en cours d'exécution.",
            "Sans elle, un pilote non signé ou détourné s'exécute en anneau 0.",
            None,
        ));

        items.push(item_posture(
            "security.platform.credential_guard_running",
            Nature::Objectif,
            service_vbs_present(&plateforme.services_actifs, service_vbs::CREDENTIAL_GUARD),
            "Credential Guard réellement en cours d'exécution. Lève l'ambiguïté du \
             registre, où l'absence recouvrait « éteint » et « actif par défaut \
             depuis Windows 11 22H2 ».",
            "Sans lui, un attaquant SYSTEM récolte les empreintes et les tickets en \
             mémoire, et se déplace latéralement.",
            None,
        ));

        items.push(item_posture(
            "security.platform.code_integrity_enforcement",
            // L'état d'application constaté, pas la stratégie qui le demande :
            // il se lit par WMI et ne s'écrit par aucun verbe.
            Nature::Objectif,
            application_integrite_code(&plateforme.integrite_code),
            "Mode d'application de la stratégie d'intégrité du code.",
            "En mode audit, la stratégie journalise sans bloquer : ce n'est pas une \
             protection, c'est une observation.",
            None,
        ));

        items.push(item_posture(
            "security.platform.dma_protection_available",
            Nature::Objectif,
            crate::etat_effectif::propriete_disponible(
                &plateforme,
                crate::etat_effectif::propriete_materielle::PROTECTION_DMA,
            ),
            "Protection DMA **disponible sur ce matériel**. Ne dit pas qu'elle est \
             active : cette activation n'est lisible par aucune source accessible \
             sans élévation.",
            "Sans protection DMA, un périphérique branché à chaud lit la mémoire \
             physique, mots de passe et clés compris.",
            None,
        ));

        // Le chemin que `ks-core` donne depuis toujours comme exemple canonique
        // d'item, et que le collecteur ne produisait pas.
        for (suffixe, valeur, but, risque) in [
            (
                "realtime",
                &defender.temps_reel,
                "Protection en temps réel de Defender, telle qu'elle tourne.",
                "Sa désactivation est, au §6 du modèle de menace, le deuxième signal \
                 par rapport valeur / effort. Beaucoup d'intrusions commencent là.",
            ),
            (
                "tamper_protection",
                &defender.anti_alteration,
                "Protection contre l'altération des réglages de Defender.",
                "Sans elle, un attaquant administrateur éteint l'antivirus par une \
                 simple commande, sans laisser d'obstacle.",
            ),
            (
                "behavior_monitoring",
                &defender.surveillance_comportementale,
                "Surveillance comportementale de Defender.",
                "Sa désactivation retire la détection qui ne dépend pas des \
                 signatures, donc celle qui voit l'inconnu.",
            ),
        ] {
            items.push(item_posture(
                &format!("security.defender.{suffixe}"),
                // Ces trois-là se lisent par WMI comme les objectifs ci-dessus,
                // mais ce sont bien des réglages : `Set-MpPreference` les écrit,
                // donc un verbe le pourra. La source de lecture ne décide pas de
                // la vocation.
                Nature::Reglage,
                match valeur {
                    Lecture::Trouvee(v) => ItemValue::Bool(*v),
                    Lecture::Absente => ItemValue::Absent,
                    Lecture::Refusee => ItemValue::illisible("WMI n'a pas répondu"),
                },
                but,
                risque,
                Some("docs/04-MODELE-DE-MENACE.md § 6"),
            ));
        }

        // ─── Pare-feu ───────────────────────────────────────────────────────
        const PARE_FEU: &str =
            r"SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy";

        for (profil, clef) in [
            ("domain", "DomainProfile"),
            ("private", "StandardProfile"),
            ("public", "PublicProfile"),
        ] {
            items.push(item_posture(
                &format!("security.firewall.{profil}.enabled"),
                Nature::Reglage,
                drapeau(&u32_registre(
                    &format!(r"{PARE_FEU}\{clef}"),
                    "EnableFirewall",
                )),
                "Pare-feu actif pour ce profil de réseau. Le profil public est celui \
                 qui compte : c'est lui qui s'applique en déplacement.",
                "Un profil éteint expose les services en écoute à tout le réseau joint.",
                None,
            ));
        }

        // On ne publie pas le total des règles, qui ne dirait rien : seules les
        // autorisations entrantes actives ouvrent réellement la machine.
        let regles = valeurs_de(&format!(r"{PARE_FEU}\FirewallRules"));
        items.push(item_posture(
            "security.firewall.inbound_allow_rules",
            // Un décompte, pas une règle. Chaque installeur en ajoute ; le
            // déclarer par égalité produirait un écart à la première
            // installation. Ce qui se déclarera un jour, ce sont les règles
            // elles-mêmes, item par item.
            Nature::Mesure,
            match regles {
                Lecture::Trouvee(v) => ItemValue::Int(
                    i64::try_from(
                        v.iter()
                            .filter(|(_, regle)| est_autorisation_entrante_active(regle))
                            .count(),
                    )
                    .unwrap_or(-1),
                ),
                Lecture::Absente => ItemValue::Absent,
                Lecture::Refusee => ItemValue::illisible("accès refusé sans élévation"),
            },
            "Règles autorisant une connexion entrante, et réellement actives. Le \
             décompte total des règles n'est pas publié : il mélange l'entrant et le \
             sortant, l'actif et l'inactif, et ne se déplie donc en rien.",
            "Chaque autorisation entrante est une porte ouverte, souvent posée par un \
             installeur et jamais refermée.",
            None,
        ));

        // ─── Firmware et microcode ──────────────────────────────────────────
        const BIOS: &str = r"HARDWARE\DESCRIPTION\System\BIOS";

        for (suffixe, valeur, but) in [
            ("vendor", "BIOSVendor", "Fabricant du firmware."),
            ("version", "BIOSVersion", "Version du firmware installé."),
        ] {
            items.push(item_posture(
                &format!("security.firmware.{suffixe}"),
                // **Constat, sous un chemin `security.`** : c'est l'exemple qui
                // interdit de déduire la nature du préfixe. On subit la version
                // du firmware ; aucun état n'y est désirable, et rien ne
                // l'écrira jamais depuis le système.
                Nature::Constat,
                texte(texte_registre(BIOS, valeur)),
                but,
                "Un firmware ancien porte des vulnérabilités corrigées depuis, en \
                 dessous de tout ce que le système peut protéger.",
                None,
            ));
        }

        let date_brute = texte_registre(BIOS, "BIOSReleaseDate");
        items.push(item_posture(
            "security.firmware.release_date",
            Nature::Constat,
            match &date_brute {
                Lecture::Trouvee(brut) => {
                    ItemValue::Text(date_firmware_certaine(brut).unwrap_or_else(|| brut.clone()))
                }
                Lecture::Absente => ItemValue::Absent,
                Lecture::Refusee => ItemValue::illisible("accès refusé sans élévation"),
            },
            "Date de publication du firmware. Normalisée en AAAA-MM-JJ uniquement \
             quand l'ordre jour/mois est certain ; sinon publiée telle quelle, car un \
             mois d'écart sur l'âge d'un firmware serait une invention.",
            "C'est l'âge, pas le numéro de version, qui dit si le firmware suit.",
            None,
        ));

        // Aucune documentation Microsoft ne décrit cette valeur : on publie
        // l'hexadécimal brut plutôt qu'une interprétation inventée.
        items.push(item_posture(
            "security.firmware.microcode_revision",
            Nature::Constat,
            match LOCAL_MACHINE
                .open(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")
                .ok()
                .and_then(|k| k.get_value("Update Revision").ok())
            {
                Some(v) => ItemValue::Text(
                    v.as_ref()
                        .iter()
                        .map(|o| format!("{o:02x}"))
                        .collect::<String>(),
                ),
                None => ItemValue::Absent,
            },
            "Révision du microcode du processeur, en hexadécimal brut. Aucune source \
             officielle n'en documente le format : on ne l'interprète pas.",
            "Un microcode ancien laisse ouvertes des vulnérabilités matérielles que \
             seul le fabricant peut corriger.",
            None,
        ));

        // ─── Horloge : la CONFIGURATION, pas la dérive ──────────────────────
        //
        // D1-09 demande la cohérence de l'horloge, qui est une mesure : elle se
        // compare à une référence externe, elle ne se lit pas. Ces items disent
        // seulement à qui la machine fait confiance pour l'heure. Les présenter
        // comme une cohérence serait la garantie annoncée non tenue habituelle.
        const W32TIME: &str = r"SYSTEM\CurrentControlSet\Services\W32Time\Parameters";

        items.push(item_posture(
            "security.clock.ntp_server",
            Nature::Reglage,
            texte(texte_registre(W32TIME, "NtpServer")),
            "Source de temps configurée. Ce n'est PAS une mesure de dérive : la \
             cohérence de l'horloge (D1-09) se compare à une référence externe.",
            "Une source de temps détournée décale toute la valeur forensique du \
             journal, sans rien casser de visible.",
            Some("docs/01-CAHIER-DES-CHARGES.md § D1-09"),
        ));

        items.push(item_posture(
            "security.clock.timezone",
            Nature::Reglage,
            texte(texte_registre(
                r"SYSTEM\CurrentControlSet\Control\TimeZoneInformation",
                "TimeZoneKeyName",
            )),
            "Fuseau horaire du poste, nécessaire pour corréler des journaux \
             provenant de plusieurs machines.",
            "Un fuseau erroné décale les corrélations sans qu'aucune horloge soit \
             fausse.",
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
                Nature::Reglage,
                match depart {
                    Lecture::Trouvee(v) => ItemValue::Text(demarrage_service(v)),
                    Lecture::Absente => ItemValue::Absent,
                    Lecture::Refusee => ItemValue::illisible("accès refusé sans élévation"),
                },
                role,
                "Un service de sécurité mis à « désactivé » est le premier maillon de \
                 la plupart des chaînes d'attaque (§6). Attention à la portée : cet \
                 item lit le TYPE DE DÉMARRAGE, pas l'exécution. Un service en \
                 « automatique » mais arrêté à la main reste vert ici.",
                Some("docs/04-MODELE-DE-MENACE.md § 6"),
            ));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Importé ici plutôt que remonté au module : hors Windows, la fabrique
    // d'items n'existe pas, mais les tests portent quand même sur la forme des
    // items — et vérifient au passage qu'elle reste vide sur ces plateformes.
    use ks_core::Provenance;

    #[test]
    fn le_type_de_demarrage_sort_en_jeton_et_non_en_chiffre() {
        // Principe P6 : « 4 » ne veut rien dire. Mais la phrase française ne se
        // compare pas — elle vit dans `ks_cli::lisible` depuis l'ADR-0015, et
        // c'est le jeton qui part dans le relevé.
        assert_eq!(demarrage_service(2), "automatique");
        assert_eq!(demarrage_service(4), "desactive");
        assert_eq!(demarrage_service(99), "code-inconnu:99");
        assert!(
            ks_core::ItemValue::Text(demarrage_service(0)).est_constat(),
            "un jeton reste un constat"
        );
        for code in [0_u32, 1, 2, 3, 4, 99] {
            assert!(
                crate::jetons::est_bien_forme(&demarrage_service(code)),
                "le code {code} ne produit pas un jeton bien formé"
            );
        }
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
            // `Observed` partout, SAUF sous la ruche de politique, qui est une
            // origine et non une supposition (ADR-0018). L'encadrement complet
            // — quelles provenances sont interdites, et à quelles conditions
            // `Managed` est permis — vit dans `un_releve_nest_ni_lauteur_…`, à
            // l'échelle du crate ; ici on garde ce qui vaut pour CE collecteur.
            assert!(
                item.provenance == Provenance::Observed
                    || crate::politique::CHEMINS.contains(&item.path.as_str()),
                "« {} » : provenance {:?} hors ruche de politique",
                item.path,
                item.provenance
            );
            assert!(item.desired.is_none(), "un collecteur ne décide de rien");
            assert!(
                item.path.starts_with("security."),
                "« {} » n'est pas dans le domaine annoncé",
                item.path
            );
        }
    }

    /// La branche de stratégie passe-t-elle encore par la ruche de politique ?
    ///
    /// # Pourquoi un contrôle textuel, et pas une assertion sur les items
    ///
    /// Parce qu'une assertion sur les items **ne mordrait nulle part ici**. Sur
    /// la machine de référence, mesuré le 2026-08-17, la clé
    /// `HKLM\SOFTWARE\Policies\Microsoft\Windows Defender\…\ASR\Rules` n'existe
    /// pas : le relevé vaut `Absent`, donc `politique::marquer` est un
    /// non-opérant, donc retirer l'appel laisserait toute la suite verte. Il en
    /// va de même en intégration continue, et sur toute machine non gérée —
    /// c'est-à-dire partout où ce projet s'exécute aujourd'hui.
    ///
    /// La barrière est donc la même que celle qui tient la liste des
    /// collecteurs : textuelle, autonome, et éprouvée par falsification. Sa
    /// limite est assumée — elle vérifie qu'un appel est écrit, pas qu'il est
    /// juste. C'est la revue qui attrape le reste.
    ///
    /// # Le motif est assemblé à l'exécution, et ce n'est pas une coquetterie
    ///
    /// `include_str!` lit **ce fichier-ci**, tests compris. Un motif écrit tel
    /// quel dans l'assertion s'y trouverait lui-même, et le test passerait avec
    /// ou sans l'appel. Ce n'est pas une hypothèse : la première rédaction de
    /// ce test l'a fait, et la falsification — remplacer l'appel par
    /// `std::convert::identity` — l'a laissée verte.
    #[test]
    fn le_releve_de_la_branche_de_strategie_passe_par_la_ruche_de_politique() {
        const SOURCE: &str = include_str!("posture.rs");

        let appel = ["crate::politique", "::marquer(", "item_posture("].concat();
        assert_eq!(
            SOURCE.matches(appel.as_str()).count(),
            1,
            "l'item des règles ASR ne passe plus par la ruche de politique : \
             `Provenance::Managed` redevient inatteignable depuis un collecteur, \
             donc `DriftStatus::Conflict` inconstructible et le principe P10 sans \
             support (ADR-0018)"
        );
    }

    #[test]
    fn une_absence_ne_devient_jamais_un_faux_negatif() {
        // LA règle de ce module, éprouvée là où elle vit : dans les traducteurs.
        //
        // La version précédente de ce test parcourait les items produits en
        // affirmant en commentaire qu'elle vérifiait la règle. Elle ne vérifiait
        // rien : elle comparait à une chaîne vide, sautait les absences, et sur une
        // machine où tout est renseigné elle n'assertait pas une seule fois. Une
        // barrière qu'on n'a pas essayé de franchir ne prouve rien — et celle-ci
        // était déjà franchie en production sans broncher.
        assert_eq!(drapeau(&Lecture::Absente), ItemValue::Absent);
        assert_eq!(
            protection_verrouillable(&Lecture::Absente),
            ItemValue::Absent
        );
        assert_eq!(texte(Lecture::Absente), ItemValue::Absent);
        assert_eq!(liste(Lecture::Absente), ItemValue::Absent);

        // Et surtout : aucune absence ne produit un booléen, quel qu'il soit.
        assert_ne!(drapeau(&Lecture::Absente), ItemValue::Bool(false));
    }

    #[test]
    fn un_refus_de_lecture_ne_devient_jamais_un_constat() {
        // Le défaut trouvé sur cette machine : la clé des exclusions Defender est
        // protégée par ACL, la CLI n'est pas élevée (SEC-01), et « 0 élément(s) »
        // s'affichait sur une machine où rien n'avait pu être lu. Une liste vide
        // est un constat ; un refus est un aveu. Les confondre inverse le sens.
        assert_ne!(liste(Lecture::Refusee), ItemValue::List(Vec::new()));
        assert_ne!(liste(Lecture::Refusee), ItemValue::Absent);
        assert_ne!(drapeau(&Lecture::Refusee), ItemValue::Bool(false));
        assert_ne!(drapeau(&Lecture::Refusee), ItemValue::Absent);

        for valeur in [
            liste(Lecture::Refusee),
            drapeau(&Lecture::Refusee),
            texte(Lecture::Refusee),
            protection_verrouillable(&Lecture::Refusee),
        ] {
            assert!(
                !valeur.est_constat(),
                "un refus ne doit jamais alimenter un décompte"
            );
        }

        // Une liste réellement vide, elle, reste un constat.
        assert!(liste(Lecture::Trouvee(Vec::new())).est_constat());
    }

    #[test]
    fn le_verrou_uefi_nest_pas_annonce_a_lenvers() {
        // Microsoft documente : 1 AVEC variable UEFI, 2 SANS. Le premier jet
        // inversait les deux, et sur cette machine (RunAsPPL = 2) Keystone
        // annonçait une protection verrouillée qui cède en réalité à un `reg add`
        // suivi d'un redémarrage. Faux positif sur la garde des identifiants.
        let avec = protection_verrouillable(&Lecture::Trouvee(1));
        let sans = protection_verrouillable(&Lecture::Trouvee(2));

        assert_eq!(avec, ItemValue::Text("active-verrou-uefi".into()));
        assert_eq!(sans, ItemValue::Text("active-sans-verrou-uefi".into()));
        assert_eq!(
            protection_verrouillable(&Lecture::Trouvee(0)),
            ItemValue::Text("inactive".into())
        );

        // Un code inconnu se nomme, il ne tombe pas dans « inactive » : sinon un
        // octet parasite passerait pour un constat.
        let inconnu = protection_verrouillable(&Lecture::Trouvee(7));
        assert_ne!(inconnu, ItemValue::Text("inactive".into()));
        assert!(inconnu.to_string().contains('7'));
    }

    #[test]
    fn la_protection_lsa_se_deplie_en_un_etat_et_un_verrou() {
        // Le dépliage de l'ADR-0015. Deux items là où une chaîne portait deux
        // faits : on peut vouloir la protection active sans se prononcer sur le
        // verrou, ce que le principe P6 exige d'un indicateur composite.
        assert_eq!(
            etat_protection_lsa(&Lecture::Trouvee(1)),
            ItemValue::Text("active".into())
        );
        assert_eq!(
            etat_protection_lsa(&Lecture::Trouvee(2)),
            ItemValue::Text("active".into()),
            "verrouillée ou non, la protection est active — le verrou vit à côté"
        );
        assert_eq!(
            etat_protection_lsa(&Lecture::Trouvee(0)),
            ItemValue::Text("inactive".into())
        );

        assert_eq!(
            verrou_uefi_protection_lsa(&Lecture::Trouvee(1)),
            ItemValue::Bool(true)
        );
        assert_eq!(
            verrou_uefi_protection_lsa(&Lecture::Trouvee(2)),
            ItemValue::Bool(false)
        );

        // Un code hors table ne devient jamais « pas de verrou » : ce serait un
        // faux positif sur la contre-mesure qui garde les identifiants.
        let inconnu = verrou_uefi_protection_lsa(&Lecture::Trouvee(7));
        assert_ne!(inconnu, ItemValue::Bool(false));
        assert!(
            !inconnu.est_constat(),
            "un octet illisible n'est pas un constat"
        );

        // Et les deux moitiés se taisent ensemble quand la clé manque.
        for absente in [
            etat_protection_lsa(&Lecture::Absente),
            verrou_uefi_protection_lsa(&Lecture::Absente),
        ] {
            assert_eq!(absente, ItemValue::Absent);
        }
    }

    #[test]
    fn vbs_configure_mais_arrete_ne_passe_pas_pour_vbs_actif() {
        // La raison d'être de la lecture WMI. Le code 1 — « enabled but not
        // running » — décrit une machine dont la configuration est identique à
        // celle d'une machine protégée, et qui ne l'est pas. Le registre les
        // confond ; ces trois assertions exigent qu'on ne les confonde plus.
        let arrete = etat_vbs(&Lecture::Trouvee(1));
        let actif = etat_vbs(&Lecture::Trouvee(2));

        assert_ne!(arrete, actif);
        assert_eq!(arrete, ItemValue::Text("configure-non-demarre".into()));
        assert_eq!(actif, ItemValue::Text("en-execution".into()));
        assert_eq!(
            etat_vbs(&Lecture::Trouvee(0)),
            ItemValue::Text("eteint".into())
        );

        // Un code inconnu se nomme, comme partout ailleurs dans ce module.
        assert!(etat_vbs(&Lecture::Trouvee(9)).to_string().contains('9'));
        assert_eq!(etat_vbs(&Lecture::Absente), ItemValue::Absent);
        assert!(!etat_vbs(&Lecture::Refusee).est_constat());
    }

    #[test]
    fn un_service_absent_dune_liste_lue_est_un_constat() {
        // La différence décisive avec le registre. Ici la liste a été LUE : que
        // Credential Guard n'y figure pas est une information, pas une lacune.
        // C'est le seul endroit du module où un `false` est légitime.
        let mesure = Lecture::Trouvee(vec![service_vbs::INTEGRITE_MEMOIRE]);

        let actif = service_vbs_present(&mesure, service_vbs::INTEGRITE_MEMOIRE);
        let arrete = service_vbs_present(&mesure, service_vbs::CREDENTIAL_GUARD);

        assert_eq!(actif, ItemValue::Text("en-execution".into()));
        assert_eq!(
            arrete,
            ItemValue::Text("arrete".into()),
            "la liste est lue : son absence est un constat, pas une supposition"
        );

        // Jamais le vocabulaire de l'interrupteur : il redirait la confusion
        // entre configuration et exécution que ce module lève. Le jeton le dit
        // au relevé, le libellé le dira à l'écran.
        assert!(!actif.to_string().contains("activ"));

        // Et une liste qu'on n'a pas pu lire ne produit jamais « arrete ».
        for illisible in [
            service_vbs_present(&Lecture::Refusee, service_vbs::CREDENTIAL_GUARD),
            service_vbs_present(&Lecture::Absente, service_vbs::CREDENTIAL_GUARD),
        ] {
            assert_ne!(illisible, ItemValue::Text("arrete".into()));
        }
    }

    #[test]
    fn lintegrite_du_code_en_audit_ne_bloque_rien() {
        // Même piège que les règles ASR : le mode audit journalise sans bloquer.
        // Le compter comme une protection serait un faux positif.
        let audit = application_integrite_code(&Lecture::Trouvee(1));
        assert_eq!(audit, ItemValue::Text("audit".into()));
        assert_ne!(audit, application_integrite_code(&Lecture::Trouvee(2)));
        assert_eq!(
            application_integrite_code(&Lecture::Trouvee(2)),
            ItemValue::Text("imposee".into())
        );
        assert_eq!(
            application_integrite_code(&Lecture::Trouvee(0)),
            ItemValue::Text("eteinte".into())
        );
    }

    #[test]
    fn une_date_de_firmware_ambigue_nest_pas_devinee() {
        // Le cas de la machine de référence : 21 dépasse le nombre de mois, donc
        // l'ordre est déterminé.
        assert_eq!(
            date_firmware_certaine("10/21/2024").as_deref(),
            Some("2024-10-21")
        );
        // L'ordre inverse se lit aussi bien.
        assert_eq!(
            date_firmware_certaine("21/10/2024").as_deref(),
            Some("2024-10-21")
        );
        // Et voici tout l'intérêt : le 3 avril et le 4 mars sont également
        // plausibles. Un mois d'écart sur l'âge d'un firmware n'est pas une
        // approximation, c'est une invention — on renvoie donc None, et
        // l'appelant publie la chaîne brute.
        assert_eq!(date_firmware_certaine("03/04/2024"), None);
        assert_eq!(date_firmware_certaine("12/12/2024"), None);
        assert_eq!(date_firmware_certaine("pas une date"), None);
        assert_eq!(date_firmware_certaine("10/21/1802"), None, "hors époque");
        assert_eq!(
            date_firmware_certaine("13/32/2024"),
            None,
            "jour impossible"
        );
    }

    #[test]
    fn seule_une_autorisation_entrante_active_compte() {
        // 649 règles sur la machine de référence, dont la quasi-totalité est
        // sortante ou inactive. Publier le total ne dirait rien à personne.
        let ouvre = "v2.33|Action=Allow|Active=TRUE|Dir=In|Protocol=6|LPort=445|Name=X|";
        assert!(est_autorisation_entrante_active(ouvre));

        for fermee in [
            "v2.33|Action=Allow|Active=FALSE|Dir=In|LPort=445|",
            "v2.33|Action=Block|Active=TRUE|Dir=In|LPort=445|",
            "v2.33|Action=Allow|Active=TRUE|Dir=Out|RPort=443|",
            "v2.33|Action=Allow|Dir=In|",
            "",
        ] {
            assert!(
                !est_autorisation_entrante_active(fermee),
                "« {fermee} » n'ouvre rien"
            );
        }

        // Le format n'est contractuel nulle part : un champ inconnu se traverse
        // sans faire échouer la lecture, sinon une mise à jour de Windows
        // ferait tomber le décompte à zéro en silence.
        assert!(est_autorisation_entrante_active(
            "v9.99|ChampInedit=42|Action=Allow|Active=true|Dir=In|AutreNouveaute=x|"
        ));
    }

    #[test]
    fn une_regle_asr_en_audit_ne_passe_pas_pour_une_protection() {
        // Compter « 12 règles » sans les modes laisserait croire à douze
        // protections là où il peut n'y en avoir aucune : une règle en audit
        // journalise sans bloquer, une règle à 0 est éteinte.
        assert_eq!(mode_asr("1"), "bloque");
        assert_eq!(mode_asr("2"), "audit");
        assert_eq!(mode_asr("0"), "éteinte");
        assert_eq!(mode_asr("5"), "non configurée");
        assert_eq!(mode_asr("6"), "avertit");
        assert_eq!(mode_asr(" 1 "), "bloque", "le registre pad ses chaînes");
        assert_eq!(mode_asr(""), "mode inconnu");
        assert_eq!(mode_asr("42"), "mode inconnu");
    }

    #[cfg(windows)]
    #[test]
    fn la_posture_repartit_ses_items_en_quatre_natures() {
        // Ce collecteur pousse toujours les mêmes 39 chemins, quelle que soit
        // la machine : ce qui varie est la VALEUR, jamais la liste. Les quatre
        // décomptes ci-dessous sont donc reproductibles partout, y compris sur
        // un agent d'intégration continue où rien n'est configuré.
        //
        // C'est ce qui en fait une barrière et non un constat : reclasser un
        // item — ou en ajouter un sans décider de sa vocation — fait échouer
        // ce test avec le nom de la nature fautive.
        let items = PostureCollector::items();
        let (mut reglages, mut objectifs, mut mesures, mut constats) = (0, 0, 0, 0);
        for item in &items {
            // `match` exhaustif sans bras `_` : une variante de plus casse la
            // compilation ici même.
            match item.nature {
                Nature::Reglage => reglages += 1,
                Nature::Objectif => objectifs += 1,
                Nature::Mesure => mesures += 1,
                Nature::Constat => constats += 1,
            }
        }

        assert_eq!(
            (reglages, objectifs, mesures, constats),
            // Un objectif de plus qu'avant l'ADR-0015 : le verrou UEFI de la
            // protection LSA, déplié de l'état qui le portait dans sa chaîne.
            (24, 7, 4, 4),
            "répartition des natures de posture : {} items",
            items.len()
        );

        // Les arbitrages nommés par l'ADR-0009, un par un. Le premier est le
        // seul qui décide vraiment quelque chose : classer Secure Boot en
        // réglage promettrait une convergence qu'aucun verbe ne sait tenir,
        // faute de pouvoir écrire dans le firmware.
        let nature = |chemin: &str| {
            items
                .iter()
                .find(|i| i.path == chemin)
                .unwrap_or_else(|| panic!("« {chemin} » n'est pas produit"))
                .nature
        };

        assert_eq!(nature("security.platform.secure_boot"), Nature::Objectif);
        assert!(!nature("security.platform.secure_boot").est_convergeable());
        assert_eq!(nature("security.platform.vbs_running"), Nature::Objectif);
        assert_eq!(nature("security.platform.hvci_uefi_lock"), Nature::Reglage);
        // Le dépliage de l'ADR-0015, et son asymétrie assumée avec la ligne
        // ci-dessus : le verrou d'HVCI a sa propre valeur de registre, celui de
        // la protection LSA n'est qu'un effet du code écrit dans `RunAsPPL`.
        assert_eq!(nature("security.platform.lsa_protection"), Nature::Reglage);
        assert_eq!(
            nature("security.platform.lsa_protection_uefi_lock"),
            Nature::Objectif
        );
        assert!(!nature("security.platform.lsa_protection_uefi_lock").est_convergeable());
        assert_eq!(
            nature("security.services.windefend.startup"),
            Nature::Reglage
        );

        // Le préfixe ne décide de rien : un constat au milieu du domaine
        // `security.`, à côté d'un réglage et d'une mesure.
        assert_eq!(nature("security.firmware.version"), Nature::Constat);
        assert!(!nature("security.firmware.version").est_declarable());
        assert_eq!(
            nature("security.defender.signatures_applied_at"),
            Nature::Mesure
        );

        // Et les trois items illisibles en permanence gardent leur vocation :
        // la clé des exclusions est protégée par ACL, la CLI n'est pas élevée
        // (SEC-01), mais une exclusion se déclare et s'écrira quand même.
        for categorie in ["paths", "extensions", "processes"] {
            let chemin = format!("security.defender.exclusions.{categorie}");
            assert_eq!(
                nature(&chemin),
                Nature::Reglage,
                "« {chemin} » : l'illisibilité est une propriété de la lecture, \
                 pas de l'item"
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

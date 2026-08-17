//! Un **changement** — ce qui a bougé entre deux relevés, et qui l'a fait.
//!
//! ## Pourquoi un type distinct, alors qu'`Item` porte déjà une provenance
//!
//! La documentation d'[`Item::provenance`](crate::Item::provenance) avouait la
//! confusion : « d'où vient l'information — **ou, pour un changement, qui l'a
//! fait** ». Ce sont deux questions, et une seule réponse ne peut pas tenir les
//! deux. Une observation n'a pas d'auteur ; seul un changement en a un.
//!
//! L'ADR-0011 sépare donc les deux : `Item::provenance` répond à « d'où vient
//! cette lecture » — d'un collecteur ([`Provenance::Observed`]) ou d'une ruche
//! de politique ([`Provenance::Managed`], ADR-0018) — et [`Change::provenance`]
//! répond à « qui a écrit cette valeur ».
//!
//! ## Un intervalle, jamais un instant
//!
//! Un sondage périodique ne sait pas quand une valeur a changé. Il sait qu'elle
//! valait `A` au scan de mardi et `B` au scan de jeudi ; entre les deux, il
//! était aveugle. Prétendre à un instant précis fabriquerait, dans un journal
//! forensique, une chronologie fausse — la faute que
//! `filetime_vers_horodatage` refuse déjà pour les dates absurdes.
//!
//! C'est aussi ce qui rend l'attribution honnête : une source ne revendique un
//! changement que si son propre événement daté tombe **dans** cet intervalle.
//! Un intervalle large ne s'attribue à personne, et c'est le bon résultat.

use serde::{Deserialize, Serialize};

use crate::{ItemValue, Provenance, Timestamp};

/// Un changement constaté entre deux relevés, avec son intervalle et son auteur.
///
/// # Le piège des deux dates, et ce qui le désamorce
///
/// [`Change::after_scan_at`] accompagne [`Change::before`] : c'est la date du
/// dernier scan qui a vu l'**ancienne** valeur. Symétriquement,
/// [`Change::before_scan_at`] accompagne [`Change::after`]. Le croisement des
/// mots est déroutant, et il est délibéré : les deux champs nomment la position
/// du **changement** par rapport aux scans, pas celle des valeurs.
///
/// Deux choses empêchent de s'y tromper. La struct est `#[non_exhaustive]`,
/// donc inconstructible par littéral hors de ce crate ; et
/// [`Change::unattributed`] prend deux **couples** `(valeur, date)`, ce qui rend
/// l'appariement structurel plutôt que positionnel.
///
/// # Naissance sans auteur
///
/// Un changement neuf vaut [`Provenance::Unknown`], jamais
/// [`Provenance::Observed`] : `Observed` qualifie un relevé, et un changement
/// n'en est pas un. Un changement que personne ne revendique **est** le signal
/// de sécurité de D2-05, et il doit le rester tant qu'une source ne l'a pas
/// réclamé par liste blanche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub struct Change {
    /// Chemin canonique de l'item qui a changé.
    pub path: String,
    /// La valeur d'avant — celle que le scan de [`Change::after_scan_at`] a vue.
    pub before: ItemValue,
    /// La valeur d'après — celle que le scan de [`Change::before_scan_at`] a vue.
    pub after: ItemValue,
    /// Le changement est survenu **après** ce relevé…
    pub after_scan_at: Timestamp,
    /// …et **avant** celui-ci. Un sondage ne connaît qu'un intervalle.
    pub before_scan_at: Timestamp,
    /// Qui a écrit la nouvelle valeur, autant qu'on puisse l'établir.
    pub provenance: Provenance,
}

impl Change {
    /// Un changement constaté entre deux relevés, **sans auteur identifié**.
    ///
    /// Les deux paramètres sont des couples `(valeur, date du scan qui l'a
    /// vue)`. L'appariement est ainsi porté par la signature : on ne peut pas
    /// donner la valeur d'avant avec la date d'après, ce qu'une liste de cinq
    /// paramètres positionnels aurait rendu facile et silencieux.
    ///
    /// La provenance naît [`Provenance::Unknown`]. C'est la valeur juste, et
    /// c'est aussi la valeur sûre : une attribution qui échoue laisse le signal
    /// de sécurité en place au lieu de le dissoudre.
    #[must_use]
    pub fn unattributed(
        path: &str,
        before: (ItemValue, Timestamp),
        after: (ItemValue, Timestamp),
    ) -> Self {
        Self {
            path: path.to_owned(),
            before: before.0,
            after: after.0,
            after_scan_at: before.1,
            before_scan_at: after.1,
            provenance: Provenance::Unknown,
        }
    }

    /// L'intervalle du changement tient-il **entièrement** dans `[debut, fin)` ?
    ///
    /// C'est la seule question qu'une source datée a le droit de poser. Un
    /// chevauchement partiel ne prouve rien : si le changement a pu survenir
    /// avant le début de la fenêtre, la source ne peut pas le revendiquer.
    ///
    /// La borne haute est **exclue**, pour que deux fenêtres consécutives —
    /// deux journées, typiquement — ne se disputent pas un changement survenu
    /// exactement à minuit.
    #[must_use]
    pub fn fits_within(&self, debut: Timestamp, fin: Timestamp) -> bool {
        debut <= self.after_scan_at && self.before_scan_at < fin
    }

    /// Ce changement est-il un signal de sécurité ?
    ///
    /// Vrai tant que personne ne l'a revendiqué — voir
    /// [`Provenance::is_security_signal`].
    #[must_use]
    pub const fn is_security_signal(&self) -> bool {
        self.provenance.is_security_signal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn instant(jour: u32, heure: u32) -> Timestamp {
        Utc.with_ymd_and_hms(2026, 8, jour, heure, 0, 0)
            .single()
            .expect("date de test valide")
    }

    fn changement(chemin: &str, debut: Timestamp, fin: Timestamp) -> Change {
        Change::unattributed(
            chemin,
            (ItemValue::Text("26100".into()), debut),
            (ItemValue::Text("26200".into()), fin),
        )
    }

    #[test]
    fn un_changement_nait_sans_auteur_et_donc_en_signal() {
        // La valeur de naissance n'est pas un détail : un changement qui
        // naîtrait `Observed` serait un changement que personne ne regarde,
        // parce qu'`is_security_signal` répondrait faux dès la construction.
        let c = changement("inventory.os.kernel", instant(13, 8), instant(13, 20));
        assert_eq!(c.provenance, Provenance::Unknown);
        assert!(c.is_security_signal());
        assert_ne!(
            c.provenance,
            Provenance::Observed,
            "« Observed » qualifie un relevé, jamais un changement"
        );
    }

    #[test]
    fn les_valeurs_et_les_dates_restent_appariees() {
        // Le croisement des noms de champs est le piège de ce type. Le
        // constructeur prend des couples précisément pour qu'il ne puisse pas
        // se refermer ; ce test l'énonce, pour que personne ne « corrige » la
        // signature en cinq paramètres positionnels.
        let (avant, apres) = (instant(13, 8), instant(13, 20));
        let c = changement("inventory.os.kernel", avant, apres);
        assert_eq!(c.before, ItemValue::Text("26100".into()));
        assert_eq!(
            c.after_scan_at, avant,
            "la date d'avant suit la valeur d'avant"
        );
        assert_eq!(c.after, ItemValue::Text("26200".into()));
        assert_eq!(
            c.before_scan_at, apres,
            "la date d'après suit la valeur d'après"
        );
        assert!(
            c.after_scan_at < c.before_scan_at,
            "l'intervalle doit courir dans le sens du temps"
        );
    }

    #[test]
    fn un_changement_ne_tient_dans_une_fenetre_que_sil_y_est_entier() {
        let jour = instant(13, 0);
        let lendemain = jour + Duration::days(1);

        // Entièrement dans la journée : la fenêtre peut le revendiquer.
        assert!(changement("x", instant(13, 8), instant(13, 20)).fits_within(jour, lendemain));

        // À cheval sur la veille : le changement a PU survenir avant la
        // fenêtre, donc la fenêtre ne prouve rien.
        assert!(
            !changement("x", instant(12, 22), instant(13, 20)).fits_within(jour, lendemain),
            "un chevauchement partiel n'est pas une preuve"
        );

        // À cheval sur le lendemain, symétriquement.
        assert!(!changement("x", instant(13, 8), instant(14, 3)).fits_within(jour, lendemain));

        // Les bornes, nommément : la basse est incluse, la haute exclue, sans
        // quoi deux journées consécutives revendiqueraient le même changement.
        assert!(changement("x", jour, lendemain - Duration::nanoseconds(1))
            .fits_within(jour, lendemain));
        assert!(!changement("x", jour, lendemain).fits_within(jour, lendemain));
    }

    #[test]
    fn un_changement_survit_a_un_aller_retour() {
        // `Change` porte deux `ItemValue`, dont la sérialisation `untagged` a
        // déjà confondu deux sens une fois. On l'éprouve ici sur le type qui
        // les transporte, pas seulement sur eux.
        for valeur in [
            ItemValue::Absent,
            ItemValue::Bool(true),
            ItemValue::List(vec!["a".into()]),
            ItemValue::illisible("accès refusé sans élévation"),
        ] {
            let c = Change::unattributed(
                "security.firmware.microcode_revision",
                (valeur.clone(), instant(13, 8)),
                (ItemValue::Text("23410000".into()), instant(13, 20)),
            );
            let json = serde_json::to_string(&c).expect("sérialisation");
            let relu: Change = serde_json::from_str(&json).expect(&json);
            assert_eq!(relu, c, "aller-retour cassé : {json}");
            assert_eq!(
                relu.provenance,
                Provenance::Unknown,
                "un changement relu ne doit pas perdre son absence d'auteur"
            );
        }
    }
}

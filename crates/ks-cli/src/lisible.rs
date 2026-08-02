//! Rendre lisible ce que la machine a mesuré, sans toucher au relevé.
//!
//! Un item porte des **octets**, parce qu'un octet est ce que la machine a
//! mesuré et que la Phase 1 comparera des octets. Mais `56043241472` à l'écran
//! ne dit rien à personne, et le principe P6 refuse ce qu'on ne peut pas
//! comprendre. La conversion appartient donc à l'affichage, jamais au relevé.
//!
//! Ce module vit dans la bibliothèque, et non dans le binaire `ks`, pour la
//! même raison que [`crate::rapport`] : la coque `ks-ui` affiche les mêmes
//! valeurs que la CLI. Deux formateurs écrits séparément finissent par afficher
//! deux tailles différentes pour le même disque, et le jour où ça arrive, on ne
//! sait plus lequel croire.

use ks_core::{Item, ItemValue};

/// Rend une valeur d'item lisible par un humain, sans toucher au modèle.
///
/// Seuls les chemins qui se terminent par `_bytes` sont convertis : la
/// conversion se déclenche sur une convention de nommage explicite, pas sur une
/// heuristique de magnitude qui transformerait un jour un compteur en taille.
#[must_use]
pub fn valeur(item: &Item) -> String {
    if let (true, ItemValue::Int(n)) = (item.path.ends_with("_bytes"), &item.observed) {
        if let Ok(o) = u64::try_from(*n) {
            return octets(o);
        }
    }
    item.observed.to_string()
}

/// Formate une taille en unités binaires, avec la ponctuation française.
///
/// Une décimale suffit : la deuxième donnerait une précision que ni le calcul
/// ni le besoin ne justifient. L'espace avant l'unité est **insécable**, sans
/// quoi le terminal — ou la fenêtre — coupe « 52,2 » et « Gio » sur deux lignes.
#[must_use]
pub fn octets(octets: u64) -> String {
    const UNITES: [&str; 5] = ["o", "Kio", "Mio", "Gio", "Tio"];
    #[allow(clippy::cast_precision_loss)] // Une décimale affichée : la perte est sous l'arrondi.
    let mut valeur = octets as f64;
    let mut rang = 0;
    while valeur >= 1024.0 && rang < UNITES.len() - 1 {
        valeur /= 1024.0;
        rang += 1;
    }
    let arrondi = if rang == 0 {
        format!("{octets}")
    } else {
        format!("{valeur:.1}").replace('.', ",")
    };
    format!("{arrondi}\u{a0}{}", UNITES[rang])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_taille_saffiche_en_unites_lisibles() {
        // La valeur relevée sur la machine de référence. « 56043241472 » ne dit
        // rien à personne ; « 52,2 Gio » situe immédiatement le disque WSL comme
        // le premier poste d'occupation du poste.
        assert_eq!(octets(56_043_241_472), "52,2\u{a0}Gio");
        assert_eq!(octets(100_663_296), "96,0\u{a0}Mio");
        // En deçà du kibioctet, l'arrondi n'apporte rien : on garde l'entier.
        assert_eq!(octets(0), "0\u{a0}o");
        assert_eq!(octets(512), "512\u{a0}o");
        assert_eq!(octets(1024), "1,0\u{a0}Kio");
    }

    #[test]
    fn la_virgule_est_francaise_et_lespace_insecable() {
        let rendu = octets(1_610_612_736);
        assert!(rendu.contains(','), "séparateur décimal français");
        assert!(!rendu.contains('.'), "jamais le point décimal anglais");
        assert!(
            rendu.contains('\u{a0}'),
            "espace insécable : sinon le terminal coupe le nombre de son unité"
        );
        assert!(!rendu.contains(' '), "aucune espace ordinaire");
    }
}

//! # `ks-cli` en bibliothèque — ce que la CLI expose à ses clients
//!
//! La CLI reste la surface de référence du produit (principe P4) ; cette
//! bibliothèque n'ajoute aucune capacité, elle **partage** ce qui existe déjà.
//!
//! ## Pourquoi une bibliothèque plutôt qu'une copie
//!
//! Le générateur de rapport était un module privé du binaire `ks`. La coque
//! graphique `ks-ui` a besoin exactement du même HTML : non pas d'un HTML
//! ressemblant, mais du **même**, produit par le même code, à partir du même
//! inventaire. Deux copies d'un générateur de rapport divergent, et le jour où
//! elles divergent, l'une des deux ment sans que personne le sache.
//!
//! Le binaire `ks` consomme désormais cette bibliothèque comme n'importe quel
//! autre client : `use ks_cli::rapport;`. Il n'existe donc qu'un seul chemin de
//! code, et un seul jeu de tests.
//!
//! Le module [`lisible`] suit la même règle pour une raison plus discrète : la
//! CLI et la coque affichent les mêmes tailles de disque. Deux formateurs
//! écrits séparément finissent par en afficher deux, et le jour où ça arrive,
//! on ne sait plus lequel croire.
//!
//! [`confrontation`] existe pour la même raison, et c'est la plus visible :
//! sans elle, la coque charge les items sans leur désir, `Item::verdict()`
//! répond `NonContraint` partout, et sa vue Dérive affiche « sans objet » pour
//! toujours. Deux chargements de `workstation.yaml` écrits séparément
//! finiraient par ne plus refuser les mêmes fichiers, et l'écran et la CLI ne
//! diraient plus la même chose du même document.
//!
//! ## Ce que cette bibliothèque n'expose pas
//!
//! Le magasin du journal reste privé au binaire. Il ouvre une base SQLite en
//! écriture ; l'exposer inviterait un client non privilégié à écrire dans le
//! journal, ce que la Phase 0 n'autorise à personne (SEC-01).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod acceptation;
pub mod confrontation;
pub mod emetteur;
pub mod etat_desire;
pub mod lisible;
pub mod rapport;

// L'inventaire figé sur lequel s'éprouvent l'émetteur et la confrontation. Un
// test qui appellerait les collecteurs mesurerait la machine qui l'exécute, et
// ne prouverait donc rien sur la CI Linux — où aucun item n'est déclarable.
#[cfg(test)]
mod machine_de_reference;

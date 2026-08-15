//! Écrit `schema/workstation.schema.json` sur la sortie standard.
//!
//! ```text
//! cargo run --locked -q -p ks-cli --example generer-schema > schema/workstation.schema.json
//! ```
//!
//! ## Pourquoi un exemple, et pas une sous-commande de `ks`
//!
//! Le schéma s'adresse aux contributeurs du dépôt, jamais à l'utilisateur du
//! poste : personne n'a besoin de le régénérer sur une machine administrée. Une
//! sous-commande `ks schema` élargirait la surface du produit — donc ce qu'il
//! faut documenter, tester et maintenir — pour un besoin qui n'existe qu'ici.
//!
//! Un `[[bin]]` aurait le même défaut en pire : il entrerait dans le binaire
//! livré. Un exemple ne s'installe pas, et `cargo clippy --all-targets` le
//! couvre exactement comme le reste.
//!
//! ## Ce que ce fichier ne fait pas
//!
//! Il n'écrit rien sur le disque. Le fichier de destination est choisi par
//! l'appelant, par redirection — un générateur qui écrase un chemin en dur est
//! un générateur qu'on n'ose plus lancer pour voir.

fn main() {
    print!("{}", ks_cli::etat_desire::schema_json());
}

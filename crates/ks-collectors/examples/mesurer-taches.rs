//! Les tâches planifiées sont-elles lisibles sans élévation ? La mesure, rejouable.
//!
//! # Pourquoi ce fichier existe
//!
//! La feuille de route a porté pendant toute la Phase 0 une affirmation fausse :
//! que les tâches planifiées exigeaient une élévation, et qu'« aucun choix de
//! bibliothèque ne les rendra lisibles ». La mesure d'origine était juste — la
//! clé de registre `Schedule\TaskCache\Tree` renvoie bien « accès refusé » — mais
//! la conclusion généralisait le refus d'**un chemin d'accès** en une
//! impossibilité de plateforme.
//!
//! L'espace de noms WMI du planificateur répond sans élévation, et c'est ce que
//! ce fichier établit. Il est versionné plutôt que jeté pour deux raisons : la
//! feuille de route cite sa commande, donc la supprimer y laisserait une
//! référence morte ; et une mesure qu'on peut rejouer vaut mieux qu'une mesure
//! racontée, surtout quand elle contredit un document.
//!
//! ```text
//! cargo run -p ks-collectors --example mesurer-taches
//! ```
//!
//! Il ne collecte rien : quels items publier, de quelle nature, et lesquels ont
//! un état désirable, sont des questions de conception que cette mesure ouvre
//! sans les trancher.

#[cfg(windows)]
fn main() {
    use serde::Deserialize;
    use wmi::{COMLibrary, WMIConnection};

    /// Le strict nécessaire pour répondre à la question posée.
    ///
    /// Tout est `Option` sauf le nom : `MSFT_ScheduledTask` laisse des champs
    /// vides — 71 tâches sur 194 n'ont pas d'auteur sur la machine de mesure —
    /// et un champ obligatoire ferait échouer la désérialisation de la liste
    /// entière, donc conclurait « illisible » sur une lecture qui a réussi.
    #[derive(Deserialize, Debug)]
    #[serde(rename = "MSFT_ScheduledTask")]
    #[serde(rename_all = "PascalCase")]
    struct Tache {
        task_name: String,
        task_path: Option<String>,
        state: Option<u8>,
        author: Option<String>,
    }

    let Ok(com) = COMLibrary::new() else {
        println!("COM refusé");
        return;
    };
    match WMIConnection::with_namespace_path(r"root\Microsoft\Windows\TaskScheduler", com) {
        Err(e) => println!("connexion refusée : {e}"),
        Ok(connexion) => match connexion.query::<Tache>() {
            Err(e) => println!("requête refusée : {e}"),
            Ok(taches) => {
                println!("LU : {} tâches", taches.len());
                for tache in taches.iter().take(3) {
                    println!(
                        "  {}{}  état={:?} auteur={:?}",
                        tache.task_path.as_deref().unwrap_or(""),
                        tache.task_name,
                        tache.state,
                        tache.author.as_deref().unwrap_or("-")
                    );
                }
                println!(
                    "  champs absents : chemin={} auteur={}",
                    taches.iter().filter(|t| t.task_path.is_none()).count(),
                    taches.iter().filter(|t| t.author.is_none()).count()
                );
            }
        },
    }
}

#[cfg(not(windows))]
fn main() {
    // La branche non-Windows existe pour que la CI Linux compile ce fichier :
    // un exemple qui ne compile pas ailleurs cesse d'être vérifié par personne.
    println!("mesure spécifique à Windows");
}

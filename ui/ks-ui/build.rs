//! Génère le contexte Tauri : configuration validée, fichiers du frontend
//! embarqués, icône posée dans les ressources de l'exécutable Windows.

fn main() {
    tauri_build::build();
}

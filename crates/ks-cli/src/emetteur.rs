//! Écriture de `workstation.yaml` — ce que `ks import` dépose (ADR-0016, décision n° 3).
//!
//! ## Pourquoi un émetteur maison plutôt qu'un sérialiseur
//!
//! Le crate de lecture est configuré sans sa moitié écriture
//! (`default-features = false, features = ["deserialize"]`), et ce n'est pas une
//! économie de dépendance : c'est une mesure. Un sérialiseur serde produit
//!
//! ```yaml
//! security.services.windefend.startup: automatique
//! ```
//!
//! sans guillemets, donc un fichier qui se relit correctement **par chance** ; et
//! aucun sérialiseur serde n'émet de commentaire, alors que l'ADR-0010 promet des
//! clés triées et un commentaire de section par domaine comme contrepartie de la
//! forme plate. Les deux manques tombent ensemble ici.
//!
//! ## La règle d'écriture, en une phrase
//!
//! **Tout scalaire textuel est entouré de guillemets, sans exception ni
//! heuristique.** Pas de liste de valeurs dangereuses : une liste écrite à la
//! main ne détecte jamais ce qu'on a oublié d'y mettre, et YAML 1.1 en retype
//! plus qu'on ne croit — `no`, `n`, `off`, `Off`, `yes`, `y`, `on` deviennent des
//! booléens, `0x9` devient le nombre 9, `26200` un entier.
//!
//! Le critère qui compte n'est donc pas « cette valeur figure-t-elle dans une
//! liste », mais « la valeur se relit-elle telle qu'elle a été écrite ». Il se
//! vérifie par un aller-retour, jamais par lecture — voir
//! `un_texte_que_yaml_retyperait_se_relit_tel_quel`, qui **mesure** quelles
//! valeurs sont piégeuses au lieu de les recopier.
//!
//! Les booléens et les entiers, eux, s'écrivent nus : c'est leur forme
//! constatée, et les citer les ferait revenir en texte, donc refuser par le
//! contrôle de forme de [`ks_core::Desire::contraindre`].
//!
//! ## Ce que l'émetteur refuse d'écrire
//!
//! * ce qui n'a pas vocation à être déclaré — [`ks_core::Nature::est_declarable`]
//!   écarte les constats et les mesures, soit la majorité d'un scan ;
//! * ce qu'on n'a pas su lire — un [`ks_core::ItemValue::Illisible`] ne devient
//!   jamais un désir, ce qui écarte les trois exclusions Defender de la machine
//!   de référence, protégées par ACL. On ne déclare pas ce qu'on n'a pas
//!   regardé.
//!
//! ## Le fichier est déterministe
//!
//! Aucune horloge, aucun compteur, aucun ordre de collecte : le même état de
//! machine produit les mêmes octets. C'est ce qui rend un second `ks import`
//! lisible en diff — sans quoi chaque exécution afficherait un changement là où
//! rien n'a bougé, et l'utilisateur cesserait de regarder les diffs.

use std::fmt::Write as _;
use std::path::Path;

use ks_core::{Desire, Item};

use crate::rapport::{titre_domaine, ORDRE};

/// Ce qu'un scan a produit une fois traduit en fichier d'état désiré.
#[derive(Debug, Clone)]
pub struct Emission {
    /// Le document complet, prêt à être écrit sur le disque.
    pub yaml: String,
    /// Nombre d'items effectivement déclarés.
    pub declarations: usize,
    /// Les items déclarables que la lecture n'a pas rendus, et pourquoi.
    ///
    /// Ils ne figurent pas dans le fichier, et l'utilisateur doit l'apprendre de
    /// la commande : un item qui disparaît sans un mot ressemble à un item qui
    /// n'existe pas.
    pub illisibles: Vec<(String, String)>,
}

/// Traduit un scan en fichier d'état désiré.
///
/// `machine` est le nom du poste, tel que [`crate::rapport::nom_machine`] le lit
/// dans l'inventaire — jamais par un appel séparé, sous peine de titrer sur une
/// machine et de déclarer les items d'une autre.
#[must_use]
pub fn emettre(items: &[Item], machine: &str) -> Emission {
    emettre_en_conservant(items, machine, None)
}

/// Traduit un scan en fichier d'état désiré, **en conservant ce qui ne se relève pas**.
///
/// # Le défaut que cette fonction corrige
///
/// `ks import --force` réécrivait le document entier depuis le seul scan. Il
/// détruisait donc, sans un mot, tout ce qu'aucun collecteur ne sait produire :
/// les tolérances et leurs raisons, la surcouche de flotte, le propriétaire du
/// poste et sa description. Ce sont des **décisions humaines**, et l'exigence
/// D2-07 les désigne nommément comme ce qu'il ne faut pas perdre : « l'oubli du
/// pourquoi est la principale cause de pourrissement des configurations ».
///
/// Le défaut existait avant que les tolérances aient un effet, ce qui le rendait
/// discret ; le lot qui leur en a donné un l'a rendu grave. L'énoncer dans un
/// message d'avertissement aurait été un demi-remède : ce qui se perd sans qu'on
/// le veuille ne doit pas se perdre du tout.
///
/// Les items relevés, eux, sont **toujours** ceux du scan : c'est le sens même de
/// la commande, et conserver une ancienne déclaration ferait de l'import une
/// fusion, ce que personne n'a demandé.
#[must_use]
pub fn emettre_en_conservant(
    items: &[Item],
    machine: &str,
    conserve: Option<&crate::etat_desire::EtatDesire>,
) -> Emission {
    let mut yaml = String::new();
    yaml.push_str(EN_TETE);
    let _ = writeln!(
        yaml,
        "apiVersion: {}",
        crate::etat_desire::VERSION_DE_FORMAT
    );
    let _ = writeln!(yaml, "kind: {}", crate::etat_desire::GENRE_ATTENDU);
    yaml.push_str("\nmetadata:\n");
    let _ = writeln!(yaml, "  name: {}", citer(machine));
    if let Some(ancien) = conserve {
        for (cle, valeur) in [
            ("inherits", ancien.metadata.inherits.as_deref()),
            ("owner", ancien.metadata.owner.as_deref()),
            ("description", ancien.metadata.description.as_deref()),
        ] {
            if let Some(v) = valeur {
                let _ = writeln!(yaml, "  {cle}: {}", citer(v));
            }
        }
    }

    let mut declarations = 0_usize;
    let mut illisibles: Vec<(String, String)> = Vec::new();
    let mut corps = String::new();

    // Les domaines dans l'ordre d'affichage du produit, et les chemins triés à
    // l'intérieur de chacun : c'est le regroupement visuel que la forme plate
    // avait fait perdre, et que l'ADR-0010 promet de rendre par ce chemin-là.
    for domaine in ORDRE {
        let mut du_domaine: Vec<&Item> = items
            .iter()
            .filter(|i| i.domain == *domaine && i.nature.est_declarable())
            .collect();
        du_domaine.sort_by(|a, b| a.path.cmp(&b.path));

        let mut lignes = String::new();
        for item in du_domaine {
            match Desire::try_from(item.observed.clone()) {
                Ok(desire) => {
                    lignes.push_str(&declaration(&item.path, &desire));
                    declarations += 1;
                }
                // La valeur n'est pas un constat : c'est un aveu de lecture, et
                // un aveu ne se déclare pas. Il est nommé à l'écran, pas écrit
                // dans le fichier.
                Err(aveu) => illisibles.push((item.path.clone(), aveu.to_string())),
            }
        }
        if !lignes.is_empty() {
            let _ = writeln!(corps, "\n  # {}", titre_domaine(*domaine));
            corps.push_str(&lignes);
        }
    }

    // `{}` plutôt qu'une clé nue : `desired:` sans rien derrière vaut `null` en
    // YAML, et une table vide et une absence de table ne se relisent pas de la
    // même façon. Le cas se produit — un hôte Linux ne porte aucun item
    // déclarable — et un fichier qu'on ne sait pas relire est pire qu'un fichier
    // vide.
    yaml.push_str(if corps.is_empty() {
        "\ndesired: {}\n"
    } else {
        "\ndesired:"
    });
    yaml.push_str(&corps);

    // La liste des écarts tolérés existe dès l'import, vide : c'est là que
    // s'écrivent la raison et l'échéance qu'exige D2-06, et une clé absente se
    // cherche plus longtemps qu'une clé vide.
    //
    // Elle est **reconduite telle quelle** quand un document précédent en
    // portait : ce sont des décisions humaines, pas des relevés, et un scan n'a
    // rien à en dire. Leur pertinence, elle, est réévaluée à chaque `ks diff` —
    // une tolérance devenue sans objet s'y signale.
    let tolerances: Vec<&crate::etat_desire::EcartAccepte> = conserve
        .map(|a| a.accepted_drift.iter().collect())
        .unwrap_or_default();
    if tolerances.is_empty() {
        yaml.push_str("\nacceptedDrift: []\n");
    } else {
        yaml.push_str("\nacceptedDrift:\n");
        for t in tolerances {
            let _ = writeln!(yaml, "  - item: {}", citer(&t.item));
            let _ = writeln!(yaml, "    reason: {}", citer(t.reason.texte()));
            let _ = writeln!(yaml, "    expires: {}", t.expires);
            let _ = writeln!(yaml, "    decidedBy: {}", citer(&t.decided_by));
            let _ = writeln!(yaml, "    decidedAt: {}", citer(&t.decided_at.to_rfc3339()));
        }
    }

    illisibles.sort_by(|a, b| a.0.cmp(&b.0));
    Emission {
        yaml,
        declarations,
        illisibles,
    }
}

/// L'en-tête de commentaires du fichier.
///
/// Il dit ce que Keystone a fait — regardé, rien d'autre — et pourquoi les
/// textes portent des guillemets. Sans cette phrase, le premier réflexe d'un
/// éditeur humain est de les retirer, et le fichier redevient relisible par
/// chance.
///
/// Aucune date n'y figure, volontairement : le fichier doit être déterministe,
/// faute de quoi un second import afficherait un changement là où rien n'a
/// bougé. La date de l'adoption vit dans le message de commit proposé
/// (ADR-0017), qui est du texte affiché et non du contenu de fichier.
const EN_TETE: &str = "\
# État désiré de ce poste, adopté depuis ce que Keystone a lu de la machine.
# Rien n'a été modifié sur le poste : `ks import` regarde, puis dépose ce fichier.
#
# Les textes portent des guillemets pour se relire tels qu'ils ont été écrits :
# sans eux, YAML lit « off », « no » et « 0x9 » comme un booléen ou un nombre,
# et l'écart qui en naît est permanent.
#
# Les items dont la lecture a échoué ne figurent pas ici ; `ks import` les nomme
# à l'écran plutôt que de déclarer ce qu'il n'a pas regardé.
";

/// Une ligne de déclaration, avec son éventuel bloc de liste.
fn declaration(chemin: &str, desire: &Desire) -> String {
    match desire {
        // La forme objet décidée par l'ADR-0016, et non l'étiquette `!absent` de
        // l'ADR-0010 : mesuré, une étiquette YAML personnalisée n'est pas
        // atteignable par un enum `untagged`.
        Desire::Absent => format!("  {chemin}: {{ absent: true }}\n"),
        Desire::Bool(b) => format!("  {chemin}: {b}\n"),
        Desire::Int(n) => format!("  {chemin}: {n}\n"),
        Desire::Text(s) => format!("  {chemin}: {}\n", citer(s)),
        Desire::List(v) if v.is_empty() => format!("  {chemin}: []\n"),
        Desire::List(v) => {
            let mut bloc = format!("  {chemin}:\n");
            for element in v {
                let _ = writeln!(bloc, "    - {}", citer(element));
            }
            bloc
        }
    }
}

/// Entoure un texte de guillemets, et échappe ce qui casserait le document.
///
/// Le style à guillemets doubles est le seul de YAML qui accepte des
/// échappements : c'est ce qui permet d'écrire un chemin Windows sans que la
/// contre-oblique disparaisse, et une valeur contenant un guillemet sans
/// refermer la chaîne au milieu.
pub(crate) fn citer(texte: &str) -> String {
    let mut cite = String::with_capacity(texte.len() + 2);
    cite.push('"');
    for c in texte.chars() {
        match c {
            '\\' => cite.push_str("\\\\"),
            '"' => cite.push_str("\\\""),
            '\n' => cite.push_str("\\n"),
            '\r' => cite.push_str("\\r"),
            '\t' => cite.push_str("\\t"),
            // Les autres caractères de contrôle n'ont pas d'écriture littérale :
            // ils passent par leur code, faute de quoi le document porterait un
            // octet que rien ne relit.
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(cite, "\\x{:02x}", c as u32);
            }
            c => cite.push(c),
        }
    }
    cite.push('"');
    cite
}

/// La commande git à exécuter, **affichée et jamais lancée** (ADR-0017).
///
/// `git commit` déclenche les crochets du dépôt — `pre-commit`, `commit-msg`,
/// `post-commit` — qui sont des fichiers exécutables du disque que git lance
/// sans rien demander. Un commit déclenché par Keystone serait donc Keystone
/// exécutant un fichier arbitraire, c'est-à-dire le geste que la doctrine du
/// projet refuse sous le nom `RunScript { path }`.
///
/// Cette fonction rend du **texte**. C'est tout ce qu'elle peut faire, et c'est
/// le point : le service est rendu, la porte reste fermée.
///
/// `date` est passée en paramètre plutôt que lue ici — une fonction qui appelle
/// l'horloge ne se teste pas deux fois de la même façon.
#[must_use]
pub fn proposition_git(fichier: &Path, date: &str) -> String {
    let dossier = fichier
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| ".".to_owned(), |p| p.display().to_string());
    let nom = fichier.file_name().map_or_else(
        || fichier.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    format!(
        "  git -C {} add -- {}\n  git -C {} commit -m \"chore(config): adopte l'état lu du {date} (D2-02)\"",
        cite_si_espace(&dossier),
        cite_si_espace(&nom),
        cite_si_espace(&dossier),
    )
}

/// Entoure de guillemets un argument que l'interpréteur découperait.
///
/// « C:\\Mes documents » est un chemin banal sous Windows, et une commande
/// affichée qu'on ne peut pas coller telle quelle ne rend aucun service.
fn cite_si_espace(argument: &str) -> String {
    if argument.contains(char::is_whitespace) {
        format!("\"{argument}\"")
    } else {
        argument.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use ks_core::{ItemValue, Nature, ScalaireBrut};

    use super::*;
    use crate::etat_desire::EtatDesire;
    use crate::machine_de_reference;

    /// Relit un document émis, et rend sa table `desired` brute.
    fn relire(yaml: &str) -> std::collections::BTreeMap<String, ScalaireBrut> {
        EtatDesire::lire(yaml)
            .unwrap_or_else(|e| {
                panic!("le fichier écrit par Keystone ne se relit pas : {e}\n{yaml}")
            })
            .desired
    }

    /// Les valeurs candidates de l'aller-retour.
    ///
    /// Elles viennent des vingt et un scalaires mesurés par l'ADR-0016, des
    /// valeurs réellement relevées sur la machine de référence, et de quelques
    /// écritures qu'un registre Windows peut légitimement porter. **Rien ici
    /// n'est marqué « piégeux » à la main** : le test le mesure.
    const CANDIDATS: &[&str] = &[
        // Ce que YAML 1.1 lit comme un booléen.
        "no",
        "n",
        "off",
        "Off",
        "false",
        "False",
        "yes",
        "y",
        "on",
        "true",
        "True",
        // Ce qu'il lit comme un nombre.
        "23410000",
        "26200",
        "0x9",
        "0",
        "-1",
        "1.5",
        // Les deux écritures du vide, qui font échouer la lecture quand elles
        // ne sont pas citées.
        "~",
        "null",
        // Les jetons du vocabulaire fermé (ADR-0015) et les valeurs relevées.
        "automatique",
        "desactive",
        "en-execution",
        "imposee",
        "active",
        "disponible-sur-ce-materiel",
        "time.windows.com,0x9",
        "Romance Standard Time",
        // Ce qu'un chemin ou une version apporte de ponctuation.
        "25.3.0",
        "2024-10-21",
        "P0CN20WW",
        "C:\\Program Files\\Outil",
        "# pas un commentaire",
        "clé: valeur",
        "- pas une liste",
        "avec \"guillemets\"",
        "  bordé d'espaces  ",
        "",
        "@arobase",
        "*astérisque",
        "&esperluette",
    ];

    /// Un texte que YAML retyperait se relit tel qu'il a été écrit.
    ///
    /// **Le critère est mécanique.** Est piégeuse une valeur dont l'écriture
    /// nue ne revient pas en texte identique — que YAML en fasse un booléen, un
    /// nombre, ou qu'il refuse le document. Le test le **mesure** en écrivant
    /// la valeur sans guillemets, plutôt que de recopier la liste des huit
    /// conversions muettes de l'ADR-0016 : une liste écrite à la main ne
    /// détecte jamais ce qu'on a oublié d'y mettre.
    ///
    /// La barrière est alors éprouvée dans le seul sens qui compte : ce que
    /// l'émetteur écrit revient identique, y compris pour ces valeurs-là.
    #[test]
    fn un_texte_que_yaml_retyperait_se_relit_tel_quel() {
        let mut pieges = Vec::new();

        for candidat in CANDIDATS {
            // Ce que YAML fait de la valeur écrite NUE, mesuré.
            let nu = format!(
                "apiVersion: keystone/v1\nkind: Workstation\nmetadata:\n  name: \"x\"\ndesired:\n  un.chemin: {candidat}\n"
            );
            let brut = EtatDesire::lire(&nu)
                .ok()
                .and_then(|d| d.desired.get("un.chemin").cloned());
            if brut != Some(ScalaireBrut::Text((*candidat).to_owned())) {
                pieges.push(*candidat);
            }

            // Et ce que l'émetteur en fait : un aller-retour sans perte.
            let emission = emettre(&[item_texte("un.chemin", candidat)], "WKS-EXEMPLE-01");
            assert_eq!(
                relire(&emission.yaml).get("un.chemin"),
                Some(&ScalaireBrut::Text((*candidat).to_owned())),
                "« {candidat} » n'est pas revenu tel quel :\n{}",
                emission.yaml
            );
        }

        // Sans quoi le test tiendrait sur un corpus devenu inoffensif, et
        // passerait au vert le jour où le guillemetage disparaît.
        assert!(
            pieges.len() >= 8,
            "le corpus ne contient plus que {} valeur(s) que YAML retype — il ne prouve plus rien : {pieges:?}",
            pieges.len()
        );
    }

    /// Un item de texte fabriqué, pour éprouver une valeur précise.
    fn item_texte(chemin: &str, valeur: &str) -> Item {
        machine_de_reference::item(
            chemin,
            ks_core::Domain::Security,
            Nature::Reglage,
            ItemValue::Text(valeur.to_owned()),
        )
    }

    /// Ce que Keystone écrit, Keystone le relit — sur les items de la machine
    /// de référence, et sans perdre une seule intention.
    ///
    /// C'est l'aller-retour complet : émission, relecture par le lecteur du
    /// produit, puis typage de chaque scalaire par la **forme de la valeur
    /// constatée** — le contrôle de l'ADR-0016, celui-là même que l'utilisateur
    /// rencontrera. Un fichier qui se relirait par chance échouerait ici.
    #[test]
    fn le_fichier_ecrit_se_relit_item_par_item_sans_perte() {
        let items = machine_de_reference::items();
        let emission = emettre(&items, "WKS-EXEMPLE-01");
        let desired = relire(&emission.yaml);

        assert_eq!(
            desired.len(),
            emission.declarations,
            "le fichier ne porte pas le nombre de déclarations annoncé"
        );

        let mut vus = 0;
        for item in &items {
            let Some(brut) = desired.get(&item.path).cloned() else {
                continue;
            };
            let desire = Desire::contraindre(brut, &item.path, &item.observed)
                .unwrap_or_else(|e| panic!("Keystone refuse sa propre écriture : {e}"));
            assert_eq!(
                Ok(desire),
                Desire::try_from(item.observed.clone()),
                "« {} » : l'intention a changé en chemin",
                item.path
            );
            vus += 1;
        }
        assert_eq!(vus, emission.declarations, "des déclarations sans item");
    }

    /// On ne déclare pas ce qu'on n'a pas su lire.
    #[test]
    fn un_item_illisible_est_nomme_plutot_que_declare() {
        let items = machine_de_reference::items();
        let emission = emettre(&items, "WKS-EXEMPLE-01");

        let attendus: Vec<&str> = items
            .iter()
            .filter(|i| i.nature.est_declarable() && !i.observed.est_constat())
            .map(|i| i.path.as_str())
            .collect();
        assert!(
            !attendus.is_empty(),
            "la machine de référence a perdu ses items illisibles : le test ne prouve plus rien"
        );

        for chemin in &attendus {
            assert!(
                !emission.yaml.contains(chemin),
                "« {chemin} » a été déclaré alors que sa lecture a échoué"
            );
            assert!(
                emission.illisibles.iter().any(|(c, _)| c == chemin),
                "« {chemin} » a disparu sans un mot"
            );
        }
        assert_eq!(emission.illisibles.len(), attendus.len());
        for (_, raison) in &emission.illisibles {
            assert!(
                raison.contains("accès refusé"),
                "la raison ne remonte pas jusqu'à l'utilisateur : {raison}"
            );
        }
    }

    /// Seule la nature décide de ce qui entre dans le fichier.
    #[test]
    fn seules_les_natures_declarables_entrent_dans_le_fichier() {
        let items = machine_de_reference::items();
        let emission = emettre(&items, "WKS-EXEMPLE-01");
        let desired = relire(&emission.yaml);

        for item in &items {
            let declare = desired.contains_key(&item.path);
            match item.nature {
                Nature::Reglage | Nature::Objectif => assert_eq!(
                    declare,
                    item.observed.est_constat(),
                    "« {} » : un item déclarable et lisible doit être déclaré, et lui seul",
                    item.path
                ),
                Nature::Mesure | Nature::Constat => assert!(
                    !declare,
                    "« {} » : une {:?} n'a pas vocation à être déclarée",
                    item.path, item.nature
                ),
            }
        }
    }

    /// Le même état de machine produit les mêmes octets.
    ///
    /// Une date ou un ordre de collecte dans le fichier suffirait à faire
    /// afficher un changement là où rien n'a bougé — et un diff qui ment à
    /// chaque exécution cesse d'être lu.
    #[test]
    fn deux_imports_du_meme_etat_donnent_le_meme_fichier() {
        let items = machine_de_reference::items();
        let premier = emettre(&items, "WKS-EXEMPLE-01").yaml;

        let mut melanges = items.clone();
        melanges.reverse();
        let second = emettre(&melanges, "WKS-EXEMPLE-01").yaml;

        assert_eq!(
            premier, second,
            "l'ordre de collecte transparaît dans le fichier"
        );

        // Et les clés sont triées à l'intérieur de chaque section, ce que
        // l'ADR-0010 promet en contrepartie de la forme plate.
        let mut sections: Vec<Vec<&str>> = Vec::new();
        for ligne in premier.lines() {
            if ligne.starts_with("  # ") {
                sections.push(Vec::new());
            } else if let Some((chemin, _)) =
                ligne.strip_prefix("  ").and_then(|l| l.split_once(':'))
            {
                if let Some(section) = sections.last_mut() {
                    section.push(chemin);
                }
            }
        }
        assert!(!sections.is_empty(), "aucune section lue dans le fichier");
        for section in &sections {
            let mut trie = section.clone();
            trie.sort_unstable();
            assert_eq!(*section, trie, "une section n'est pas triée par chemin");
        }
    }

    /// Le regroupement par domaine, l'autre moitié de la contrepartie promise.
    #[test]
    fn chaque_domaine_declare_porte_son_commentaire_de_section() {
        let items = machine_de_reference::items();
        let emission = emettre(&items, "WKS-EXEMPLE-01");

        for domaine in ORDRE {
            let declare = items.iter().any(|i| {
                i.domain == *domaine && i.nature.est_declarable() && i.observed.est_constat()
            });
            let commentaire = format!("  # {}", titre_domaine(*domaine));
            assert_eq!(
                emission.yaml.contains(&commentaire),
                declare,
                "« {} » : commentaire de section et déclarations en désaccord",
                titre_domaine(*domaine)
            );
        }
    }

    /// Un hôte sans aucun item déclarable produit tout de même un fichier lisible.
    ///
    /// Le cas se produit : un poste Linux ne porte ni posture Windows ni WSL.
    /// `desired:` sans rien derrière vaut `null` en YAML, et le document
    /// entier deviendrait illisible.
    #[test]
    fn un_scan_sans_item_declarable_produit_un_fichier_relisible() {
        let sans_reglage: Vec<Item> = machine_de_reference::items()
            .into_iter()
            .filter(|i| !i.nature.est_declarable())
            .collect();
        let emission = emettre(&sans_reglage, "WKS-EXEMPLE-01");

        assert_eq!(emission.declarations, 0);
        assert!(relire(&emission.yaml).is_empty());
    }

    /// La commande git est du texte, et l'utilisateur peut la coller telle quelle.
    #[test]
    fn la_commande_git_est_affichable_et_collable() {
        let proposee = proposition_git(Path::new("D:\\poste\\workstation.yaml"), "2026-08-15");
        assert!(
            proposee.contains("git -C D:\\poste add -- workstation.yaml"),
            "{proposee}"
        );
        assert!(proposee.contains("chore(config)"), "{proposee}");
        assert!(
            proposee.contains("D2-02"),
            "la référence d'exigence manque : {proposee}"
        );

        // Un chemin sans dossier reste exécutable : `git -C .` vaut le
        // répertoire courant, là où un `-C` vide ferait échouer la commande.
        let ici = proposition_git(Path::new("w.yaml"), "2026-08-15");
        assert!(ici.contains("git -C . add -- w.yaml"), "{ici}");

        // Et un dossier à espaces se colle sans se couper en deux.
        let espace = proposition_git(
            Path::new("C:\\Mes documents\\workstation.yaml"),
            "2026-08-15",
        );
        assert!(espace.contains("-C \"C:\\Mes documents\""), "{espace}");
    }

    #[test]
    fn un_import_conserve_les_decisions_humaines() {
        // **Le défaut que ce test verrouille.** `ks import --force` réécrivait le
        // document depuis le seul scan, donc détruisait sans un mot les
        // tolérances, leurs raisons, la surcouche de flotte, le propriétaire et
        // la description — tout ce qu'aucun collecteur ne sait produire.
        //
        // Il était discret tant qu'une tolérance n'avait aucun effet ; le lot qui
        // lui en a donné un l'a rendu grave. Un aller-retour complet le prouve :
        // on émet, on garnit, on relit, on réémet, et tout doit être encore là.
        use crate::etat_desire::EtatDesire;

        let items = crate::machine_de_reference::items();
        let premier = emettre(&items, "WKS-EXEMPLE-01").yaml;

        let garni = premier
            .replace(
                "  name: \"WKS-EXEMPLE-01\"",
                "  name: \"WKS-EXEMPLE-01\"\n  \
                 inherits: \"./base.yaml\"\n  \
                 owner: \"tene\"\n  \
                 description: \"poste d'ingénierie\"",
            )
            .replace(
                "\nacceptedDrift: []",
                "\nacceptedDrift:\n  \
                 - item: security.services.fax.startup\n    \
                 reason: \"pilote du scanner : off\"\n    \
                 expires: 2026-10-15\n    \
                 decidedBy: \"tene\"\n    \
                 decidedAt: \"2026-07-18T09:12:00+02:00\"",
            );
        let ancien = EtatDesire::lire(&garni).expect("document garni valide");

        let reemis = emettre_en_conservant(&items, "WKS-EXEMPLE-01", Some(&ancien)).yaml;
        let relu = EtatDesire::lire(&reemis).expect("Keystone doit relire sa réémission");

        assert_eq!(relu.metadata.inherits.as_deref(), Some("./base.yaml"));
        assert_eq!(relu.metadata.owner.as_deref(), Some("tene"));
        assert_eq!(
            relu.metadata.description.as_deref(),
            Some("poste d'ingénierie")
        );
        assert_eq!(relu.accepted_drift.len(), 1);
        // La raison contient « off » et un deux-points : deux pièges de YAML que
        // le guillemetage systématique neutralise, y compris sur ce chemin-ci.
        assert_eq!(
            relu.accepted_drift[0].reason.texte(),
            "pilote du scanner : off"
        );
        assert_eq!(relu.accepted_drift[0].expires.to_string(), "2026-10-15");
        assert_eq!(relu.accepted_drift[0].decided_by, "tene");

        // Et le résultat reste déterministe : deux réémissions donnent les mêmes
        // octets, sans quoi chaque import afficherait un changement là où rien
        // n'a bougé.
        assert_eq!(
            reemis,
            emettre_en_conservant(&items, "WKS-EXEMPLE-01", Some(&ancien)).yaml
        );
    }

    #[test]
    fn un_import_sans_document_precedent_ecrit_une_liste_vide() {
        // La contre-épreuve : le cas courant, celui du premier import, ne doit
        // pas gagner de clé qu'il ne portait pas.
        let items = crate::machine_de_reference::items();
        let yaml = emettre_en_conservant(&items, "WKS-EXEMPLE-01", None).yaml;
        assert!(yaml.contains("\nacceptedDrift: []\n"), "{yaml}");
        assert!(!yaml.contains("inherits:"), "{yaml}");
    }
}

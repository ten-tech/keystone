//! Le côté **désiré** d'un item, et le contrôle qui neutralise le typage de YAML.
//!
//! ## Deux types, et pourquoi ce n'est pas un de trop
//!
//! [`ScalaireBrut`] est ce qu'un fichier a donné. [`Desire`] est ce que Keystone
//! a accepté. Entre les deux, [`Desire::contraindre`], et rien d'autre.
//!
//! Le lecteur de `workstation.yaml` (`ks_cli::etat_desire`) déclare sa table
//! `desired` en `BTreeMap<String, ScalaireBrut>` : un fichier ne peut donc pas
//! produire un [`Desire`] sans passer par le contrôle. C'est une **propriété du
//! seul point d'entrée**, tenue par une déclaration de type et un test, pas un
//! invariant que le compilateur prouve : `Desire::Text(…)` reste constructible
//! à la main, comme il doit l'être pour les tests et pour `ks import`. La nuance
//! est écrite ici parce que la confondre avec une garantie du compilateur est
//! exactement ce que `rules/rust.md` proscrit.
//!
//! ## Ce qu'un désir ne peut pas être
//!
//! Un constat peut être [`ItemValue::Illisible`] : « je n'ai pas su regarder »
//! est une mesure honnête. **Un désir ne le peut pas.** Vouloir qu'un item soit
//! illisible n'a aucun sens, et un type capable de l'exprimer serait un type mal
//! fait (ADR-0010, décision n° 3, dont l'ADR-0016 ne corrige que la syntaxe de
//! l'absence). L'énumération ci-dessous n'a donc pas cette variante, et le test
//! `un_desir_ne_peut_pas_avouer_un_echec_de_lecture` casse la **compilation** le
//! jour où quelqu'un l'ajoute.
//!
//! ## Le scalaire est typé par l'item, pas par YAML
//!
//! C'est la décision n° 5 de l'ADR-0016, et le cœur de ce module. YAML 1.1 lit
//! `off`, `no`, `n`, `on`, `yes`, `y` comme des booléens, `23410000` comme un
//! entier, `0x9` comme le nombre 9. Deux items de la machine de référence sont
//! exposés aujourd'hui : `inventory.os.kernel` vaut `Text("26200")` et
//! `security.firmware.microcode_revision` vaut `Text("23410000")`. Déclarés sans
//! guillemets, ils reviendraient en `Int` et produiraient un écart **permanent**
//! sur une machine qui n'a jamais changé.
//!
//! Guillemeter à l'écriture protège le fichier que Keystone écrit, pas celui que
//! l'humain édite. C'est l'humain qui écrira `off`. La contrainte se fait donc à
//! la **lecture**, contre la forme de la valeur constatée, et dans les deux sens :
//! un item constaté booléen déclaré avec un texte est refusé aussi.
//!
//! On refuse plutôt que de normaliser. Traduire `Bool(false)` en `Text("off")`
//! parce que l'item attend un texte reviendrait à deviner : `off`, `false`, `no`
//! et `desactive` deviendraient la même chose, alors que le collecteur émet un
//! jeton précis (ADR-0015). Refuser en expliquant coûte une phrase et n'invente
//! rien.

use serde::{Deserialize, Serialize};

use crate::ItemValue;

/// Ce qu'un fichier d'état désiré a donné pour une entrée, **avant contrôle**.
///
/// Volontairement le décalque de [`ItemValue`] moins `Illisible` : c'est le
/// vocabulaire qu'un désérialiseur `untagged` peut reconnaître dans un document
/// écrit à la main. Le type ne prétend rien de plus que « voilà ce que le
/// format a décidé » — et c'est précisément parce que le format décide mal
/// qu'il ne faut pas le confondre avec un [`Desire`].
///
/// # L'ordre des variantes ne décide de rien
///
/// En `untagged`, serde essaie les variantes dans l'ordre de déclaration. Ici
/// elles sont **mutuellement exclusives par leur contenu** — [`Self::Absent`]
/// exige un objet à clé `absent` et refuse toute autre clé —, exactement comme
/// pour [`ItemValue`]. Le test d'aller-retour l'exige variante par variante.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScalaireBrut {
    /// `{ absent: true }` — l'item ne doit pas exister.
    ///
    /// La syntaxe est une **forme objet**, et non l'étiquette `!absent` que
    /// l'ADR-0010 proposait : mesuré, une étiquette YAML personnalisée n'est pas
    /// atteignable par un enum `untagged`, elle donne « data did not match any
    /// variant ». La forme objet, elle, marche — et c'est déjà celle que porte
    /// [`ItemValue::Absent`] sur le fil (ADR-0016, décision n° 4).
    #[serde(with = "crate::item::absent_objet")]
    Absent,
    /// Ce que le format a lu comme un booléen. **Y compris `off`, `no` et `on`.**
    Bool(bool),
    /// Ce que le format a lu comme un entier. **Y compris `0x9`, qui vaut 9.**
    Int(i64),
    /// Ce que le format a lu comme un texte.
    Text(String),
    /// Ce que le format a lu comme une liste de textes.
    List(Vec<String>),
}

/// Ce qu'un fichier d'état désiré demande pour un item, **après contrôle**.
///
/// Cinq intentions, et pas de sixième : voir la note de module sur ce qu'un
/// désir ne peut pas être.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Desire {
    /// L'item ne doit pas exister. Sérialisé `{"absent": true}`, jamais `null`.
    ///
    /// La forme objet est reprise telle quelle de [`ItemValue::Absent`], par le
    /// même code : `null` se confondrait avec `Option::None`, c'est-à-dire avec
    /// « je ne contrains pas cet item » — deux intentions opposées.
    #[serde(with = "crate::item::absent_objet")]
    Absent,
    /// L'item doit valoir ce booléen.
    Bool(bool),
    /// L'item doit valoir cet entier.
    Int(i64),
    /// L'item doit valoir ce texte — un jeton d'ADR-0015, une version, un chemin.
    Text(String),
    /// L'item doit valoir cette liste.
    List(Vec<String>),
}

impl From<Desire> for ItemValue {
    /// Un désir est toujours une valeur d'item ; l'inverse est faux.
    ///
    /// C'est l'asymétrie que tout ce module existe pour tenir, et elle est ici
    /// visible dans les signatures : cette conversion-ci est **totale**, celle
    /// de [`ItemValue`] vers [`Desire`] renvoie un [`Result`].
    ///
    /// Le `match` est exhaustif sans bras `_` : ajouter une variante à
    /// [`Desire`] casse la compilation ici, donc oblige à décider ce qu'elle
    /// devient une fois comparée.
    fn from(desire: Desire) -> Self {
        match desire {
            Desire::Absent => Self::Absent,
            Desire::Bool(b) => Self::Bool(b),
            Desire::Int(n) => Self::Int(n),
            Desire::Text(s) => Self::Text(s),
            Desire::List(v) => Self::List(v),
        }
    }
}

impl TryFrom<ItemValue> for Desire {
    type Error = ItemValue;

    /// Une valeur constatée devient un désir, **sauf** si c'est un aveu d'échec.
    ///
    /// C'est ce dont `ks import` aura besoin : il ne déclare que ce qu'il a su
    /// lire, ce qui écarte notamment les trois exclusions Defender, illisibles
    /// en permanence sans élévation. L'erreur rend la valeur refusée plutôt
    /// qu'un message : l'appelant sait déjà de quel item il s'agit.
    fn try_from(valeur: ItemValue) -> Result<Self, Self::Error> {
        match valeur {
            ItemValue::Absent => Ok(Self::Absent),
            ItemValue::Bool(b) => Ok(Self::Bool(b)),
            ItemValue::Int(n) => Ok(Self::Int(n)),
            ItemValue::Text(s) => Ok(Self::Text(s)),
            ItemValue::List(v) => Ok(Self::List(v)),
            aveu @ ItemValue::Illisible { .. } => Err(aveu),
        }
    }
}

impl std::fmt::Display for Desire {
    /// Emprunte le formateur d'[`ItemValue`] plutôt que d'en écrire un second.
    ///
    /// Deux formateurs pour la même donnée finissent par en afficher deux, et
    /// le jour où ça arrive, on ne sait plus lequel croire.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ItemValue::from(self.clone()).fmt(f)
    }
}

/// La forme qu'un scalaire doit avoir, telle que la valeur constatée l'impose.
///
/// Quatre formes, et le `match` sur ce type reste exhaustif sans bras `_` chez
/// ses consommateurs : ajouter une forme oblige à décider du message qui
/// l'accompagne, plutôt que de tomber dans un fourre-tout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeAttendue {
    /// `true` ou `false`.
    Booleen,
    /// Un nombre entier.
    Entier,
    /// Un texte : jeton, version, chemin, numéro de série.
    Texte,
    /// Une liste de textes.
    Liste,
}

impl FormeAttendue {
    /// Le nom de la forme, avec son article, tel qu'il entre dans une phrase.
    #[must_use]
    pub const fn avec_article(self) -> &'static str {
        match self {
            Self::Booleen => "un booléen",
            Self::Entier => "un nombre entier",
            Self::Texte => "un texte",
            Self::Liste => "une liste",
        }
    }

    /// Ce que l'utilisateur peut faire pour obtenir cette forme-là.
    ///
    /// Le conseil dépend de la forme **attendue**, jamais de celle qui a été
    /// lue : « entourez de guillemets » n'a de sens que si l'item porte un
    /// texte, et le donner ailleurs enverrait l'utilisateur dans le mur.
    #[must_use]
    pub const fn conseil(self) -> &'static str {
        match self {
            Self::Texte => {
                "YAML lit off, no, n, on, yes, y et les suites de chiffres comme \
                 autre chose qu'un texte, avant que Keystone les voie. Entourez \
                 la valeur de guillemets."
            }
            Self::Booleen => "Déclarez true ou false, sans guillemets.",
            Self::Entier => "Déclarez le nombre sans guillemets.",
            Self::Liste => "Déclarez une liste, un élément par ligne, précédé d'un tiret.",
        }
    }
}

impl ScalaireBrut {
    /// La forme que le fichier a effectivement donnée.
    ///
    /// [`Self::Absent`] n'en a pas : « cet item ne doit pas exister » se déclare
    /// de la même manière quelle que soit la forme de la valeur constatée.
    #[must_use]
    pub const fn forme(&self) -> Option<FormeAttendue> {
        match self {
            Self::Absent => None,
            Self::Bool(_) => Some(FormeAttendue::Booleen),
            Self::Int(_) => Some(FormeAttendue::Entier),
            Self::Text(_) => Some(FormeAttendue::Texte),
            Self::List(_) => Some(FormeAttendue::Liste),
        }
    }
}

impl ItemValue {
    /// La forme qu'une valeur constatée impose au scalaire déclaré.
    ///
    /// `None` pour les deux cas où **il n'y a rien à quoi confronter** :
    ///
    /// * [`ItemValue::Illisible`] — la lecture a échoué, donc la forme réelle de
    ///   l'item est inconnue. Refuser la déclaration rendrait indéclarables les
    ///   trois exclusions Defender, illisibles en permanence sans élévation :
    ///   ce serait le défaut que l'ADR-0008 a corrigé, remis à l'envers.
    /// * [`ItemValue::Absent`] — l'item n'existe pas sur la machine, donc il
    ///   n'a pas de forme. Vouloir `true` pour une clé qui n'existe pas est un
    ///   désir parfaitement légitime : c'est même l'écart typique.
    ///
    /// Dans ces deux cas, [`Desire::contraindre`] accepte ce que le fichier a
    /// donné. C'est une **limite assumée** du contrôle, et elle est de la même
    /// famille que celle que l'ADR-0016 consigne pour les items non observés.
    #[must_use]
    pub const fn forme(&self) -> Option<FormeAttendue> {
        match self {
            Self::Absent | Self::Illisible { .. } => None,
            Self::Bool(_) => Some(FormeAttendue::Booleen),
            Self::Int(_) => Some(FormeAttendue::Entier),
            Self::Text(_) => Some(FormeAttendue::Texte),
            Self::List(_) => Some(FormeAttendue::Liste),
        }
    }
}

/// Un scalaire déclaré dont la forme ne correspond pas à celle de l'item.
///
/// Les deux formes sont conservées séparément du message : c'est ce qui permet
/// d'afficher la phrase et de journaliser le détail, plutôt qu'une seule chaîne
/// concaténée dont on ne peut plus rien extraire.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "« {chemin} » : la valeur déclarée a été lue comme {lu_avec_article}, alors que \
     cet item porte {attendu_avec_article}. {conseil}",
    lu_avec_article = .lu.avec_article(),
    attendu_avec_article = .attendu.avec_article(),
    conseil = .attendu.conseil()
)]
pub struct ErreurDeTypage {
    /// Le chemin de l'item concerné, tel qu'il est écrit dans le fichier.
    pub chemin: String,
    /// La forme que la valeur constatée impose.
    pub attendu: FormeAttendue,
    /// La forme que le fichier a donnée.
    pub lu: FormeAttendue,
}

impl Desire {
    /// Contraint un scalaire du fichier par la **forme de la valeur constatée**.
    ///
    /// C'est le seul chemin par lequel un fichier produit un [`Desire`].
    ///
    /// Trois cas, et ils sont dans la table de l'ADR-0016 :
    ///
    /// * le fichier dit `{ absent: true }` → accepté, quelle que soit la forme
    ///   constatée : un désir d'absence ne se type pas ;
    /// * la valeur constatée n'a pas de forme (illisible, ou absente) → accepté
    ///   tel quel, faute de quoi confronter — voir [`ItemValue::forme`] ;
    /// * sinon les deux formes doivent coïncider, **dans les deux sens**.
    ///
    /// # Ce que cette fonction ne voit pas
    ///
    /// Un chemin **déclaré mais non observé** n'arrive jamais ici : il n'a pas
    /// d'`observe` à lui opposer. Une faute de frappe dans un chemin et un item
    /// légitimement disparu échappent donc tous deux au typage, et se signalent
    /// « déclaré, non observé » en nommant les deux causes. C'est la dette prise
    /// par l'ADR-0010, et elle n'est pas remboursée ici.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`ErreurDeTypage`] quand la forme déclarée et la forme constatée
    /// diffèrent — le cas de `off` écrit sur un item qui porte un texte.
    pub fn contraindre(
        brut: ScalaireBrut,
        chemin: &str,
        observe: &ItemValue,
    ) -> Result<Self, ErreurDeTypage> {
        let (Some(lu), Some(attendu)) = (brut.forme(), observe.forme()) else {
            return Ok(Self::sans_controle(brut));
        };
        if lu == attendu {
            Ok(Self::sans_controle(brut))
        } else {
            Err(ErreurDeTypage {
                chemin: chemin.to_owned(),
                attendu,
                lu,
            })
        }
    }

    /// La traduction pure, une fois la décision d'accepter prise.
    ///
    /// Privée à dessein : c'est [`Desire::contraindre`] qui décide, et elle
    /// seule. Exposer ce chemin rendrait le contrôle contournable par mégarde.
    fn sans_controle(brut: ScalaireBrut) -> Self {
        match brut {
            ScalaireBrut::Absent => Self::Absent,
            ScalaireBrut::Bool(b) => Self::Bool(b),
            ScalaireBrut::Int(n) => Self::Int(n),
            ScalaireBrut::Text(s) => Self::Text(s),
            ScalaireBrut::List(v) => Self::List(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toutes les intentions du type, une par variante.
    ///
    /// Le `match` exhaustif sans bras `_` casse la **compilation** le jour où
    /// une variante s'ajoute sans rejoindre cette liste. Une liste
    /// d'échantillons ne détecte jamais ce qu'on a oublié d'y mettre.
    fn toutes_les_intentions() -> Vec<Desire> {
        let echantillons = vec![
            Desire::Absent,
            Desire::Bool(true),
            Desire::Bool(false),
            Desire::Int(0),
            Desire::Int(23_410_000),
            Desire::Text(String::new()),
            Desire::Text("automatique".into()),
            Desire::List(Vec::new()),
            Desire::List(vec!["a".into(), "b".into()]),
        ];
        for d in &echantillons {
            match d {
                Desire::Absent
                | Desire::Bool(_)
                | Desire::Int(_)
                | Desire::Text(_)
                | Desire::List(_) => {}
            }
        }
        echantillons
    }

    #[test]
    fn un_desir_ne_peut_pas_avouer_un_echec_de_lecture() {
        // La barrière est le `match` exhaustif de `toutes_les_intentions` et
        // celui de `From<Desire> for ItemValue` : ajouter `Illisible` à `Desire`
        // casse la compilation avant qu'un test s'exécute. Ce que ce test-ci
        // vérifie, c'est le sens de l'asymétrie — un aveu constaté ne devient
        // jamais un désir, alors que tout désir devient une valeur comparable.
        let aveu = ItemValue::illisible("accès refusé sans élévation");
        assert!(
            Desire::try_from(aveu.clone()).is_err(),
            "un aveu d'illisibilité est devenu un désir"
        );

        for desire in toutes_les_intentions() {
            let valeur = ItemValue::from(desire.clone());
            assert!(
                valeur.est_constat(),
                "{desire:?} : un désir a produit autre chose qu'un constat"
            );
            assert_eq!(
                Desire::try_from(valeur).ok(),
                Some(desire.clone()),
                "{desire:?} : aller-retour par ItemValue cassé"
            );
        }
    }

    #[test]
    fn aucune_intention_ne_se_confond_avec_une_autre_apres_un_aller_retour() {
        // Même famille de piège que pour `ItemValue` : une variante unité
        // sérialise en `null` en `untagged`, donc à l'identique d'`Option::None`.
        // « Cet item ne doit pas exister » redeviendrait « je ne contrains pas
        // cet item » — deux intentions opposées.
        let mut vues = Vec::new();
        for desire in toutes_les_intentions() {
            let json = serde_json::to_string(&desire).expect("sérialisation");
            let relu: Desire = serde_json::from_str(&json).expect(&json);
            assert_eq!(relu, desire, "aller-retour cassé : {json}");
            vues.push(json);
        }

        let avant = vues.len();
        vues.sort();
        vues.dedup();
        assert_eq!(
            avant,
            vues.len(),
            "deux intentions s'écrivent pareil : {vues:?}"
        );

        assert_eq!(
            serde_json::to_string(&Desire::Absent).expect("sérialisation"),
            r#"{"absent":true}"#,
            "un désir d'absence s'écrit en objet, jamais en null"
        );
        assert_ne!(
            serde_json::to_string(&Some(Desire::Absent)).expect("sérialisation"),
            serde_json::to_string(&Option::<Desire>::None).expect("sérialisation"),
            "« ne doit pas exister » et « non déclaré » se réécrivent pareil"
        );
    }

    #[test]
    fn un_desir_dabsence_est_accepte_quelle_que_soit_la_forme_constatee() {
        // Ligne « n'importe lequel | {absent: true} | accepté » de la table de
        // l'ADR-0016. Un désir d'absence ne se type pas : il dit justement que
        // la valeur n'a pas à exister.
        for observe in [
            ItemValue::Bool(true),
            ItemValue::Int(26_200),
            ItemValue::Text("automatique".into()),
            ItemValue::List(vec!["a".into()]),
            ItemValue::Absent,
            ItemValue::illisible("accès refusé sans élévation"),
        ] {
            let d = Desire::contraindre(ScalaireBrut::Absent, "un.chemin", &observe);
            assert_eq!(d, Ok(Desire::Absent), "constaté {observe:?}");
        }
    }

    #[test]
    fn la_contrainte_joue_dans_les_deux_sens() {
        // L'ADR-0016 l'écrit noir sur blanc : « un item constaté `Bool` déclaré
        // avec un texte est refusé aussi ». Un contrôle qui ne mordrait que
        // dans un sens laisserait passer la moitié des confusions.
        let quatre_formes = [
            (ItemValue::Bool(true), ScalaireBrut::Bool(false)),
            (ItemValue::Int(1), ScalaireBrut::Int(2)),
            (ItemValue::Text("a".into()), ScalaireBrut::Text("b".into())),
            (
                ItemValue::List(vec![]),
                ScalaireBrut::List(vec!["x".into()]),
            ),
        ];

        for (observe, brut) in &quatre_formes {
            // La diagonale : même forme des deux côtés, accepté.
            assert!(
                Desire::contraindre(brut.clone(), "un.chemin", observe).is_ok(),
                "constaté {observe:?}, déclaré {brut:?} : refusé à tort"
            );
            // Et tout le reste de la matrice : refusé, dans les deux sens.
            for (autre_observe, _) in &quatre_formes {
                if autre_observe.forme() == observe.forme() {
                    continue;
                }
                let e = Desire::contraindre(brut.clone(), "un.chemin", autre_observe)
                    .expect_err("formes différentes acceptées");
                assert_eq!(e.attendu, autre_observe.forme().expect("forme connue"));
                assert_eq!(e.lu, brut.forme().expect("forme connue"));
            }
        }
    }

    #[test]
    fn off_sur_un_item_texte_est_refuse_et_le_message_dit_quoi_faire() {
        // Le cas nommé par l'ADR-0016, et le seul dont le message est prescrit.
        // `off` est arrivé ici en `Bool(false)` : c'est YAML qui a décidé, pas
        // l'utilisateur, et c'est exactement ce que le message doit expliquer.
        let e = Desire::contraindre(
            ScalaireBrut::Bool(false),
            "security.services.windefend.startup",
            &ItemValue::Text("automatique".into()),
        )
        .expect_err("un booléen accepté sur un item texte");

        let phrase = e.to_string();
        for attendu in [
            "security.services.windefend.startup",
            "un booléen",
            "un texte",
            "Entourez",
            "guillemets",
            "off",
        ] {
            assert!(
                phrase.contains(attendu),
                "« {attendu} » absent de : {phrase}"
            );
        }

        // Voix du produit : jamais de superlatif d'urgence, jamais de reproche,
        // jamais de « ! ». Le message décrit ce qui s'est passé, ce que ça
        // implique, puis ce qu'on peut faire — dans cet ordre.
        assert!(!phrase.contains('!'), "ton d'urgence : {phrase}");
        for reproche in ["vous avez", "erreur de l'utilisateur", "oubli"] {
            assert!(!phrase.contains(reproche), "reproche : {phrase}");
        }

        // Et le détail technique reste séparé de la phrase : c'est ce qui
        // permet de journaliser l'un sans afficher l'autre.
        assert_eq!(e.attendu, FormeAttendue::Texte);
        assert_eq!(e.lu, FormeAttendue::Booleen);
    }

    #[test]
    fn le_conseil_depend_de_la_forme_attendue_pas_de_celle_qui_a_ete_lue() {
        // « Entourez de guillemets » n'a de sens que si l'item porte un texte.
        // Le donner ailleurs enverrait l'utilisateur dans le mur — et c'est
        // l'erreur qu'un message unique aurait produite.
        let vers_un_booleen = Desire::contraindre(
            ScalaireBrut::Text("off".into()),
            "security.defender.realtime",
            &ItemValue::Bool(true),
        )
        .expect_err("un texte accepté sur un item booléen");

        let phrase = vers_un_booleen.to_string();
        assert!(phrase.contains("true ou false"), "{phrase}");
        assert!(
            !phrase.contains("Entourez"),
            "conseil de guillemetage donné pour un item booléen : {phrase}"
        );

        // Les quatre formes ont chacune leur conseil, et aucun n'est vide.
        for forme in [
            FormeAttendue::Booleen,
            FormeAttendue::Entier,
            FormeAttendue::Texte,
            FormeAttendue::Liste,
        ] {
            assert!(!forme.conseil().is_empty(), "{forme:?} sans conseil");
            assert!(!forme.avec_article().is_empty(), "{forme:?} sans libellé");
        }
    }

    #[test]
    fn une_valeur_constatee_sans_forme_ne_contraint_rien() {
        // Les deux cas où il n'y a rien à quoi confronter, et où refuser serait
        // le défaut que l'ADR-0008 a corrigé, remis à l'envers : les trois
        // exclusions Defender sont illisibles en permanence sans élévation, et
        // les rendre indéclarables reviendrait à punir l'utilisateur d'un accès
        // qu'on n'a pas.
        for observe in [
            ItemValue::illisible("accès refusé sans élévation"),
            ItemValue::Absent,
        ] {
            assert_eq!(observe.forme(), None, "{observe:?} : forme inventée");
            let d = Desire::contraindre(ScalaireBrut::Bool(true), "un.chemin", &observe);
            assert_eq!(d, Ok(Desire::Bool(true)), "constaté {observe:?}");
        }

        // Et l'écart typique reste possible : vouloir `true` pour une clé qui
        // n'existe pas est un désir légitime, pas une faute de forme.
        let voulu = Desire::contraindre(ScalaireBrut::Bool(true), "un.chemin", &ItemValue::Absent)
            .expect("désir refusé sur un item absent");
        assert_ne!(ItemValue::from(voulu), ItemValue::Absent);
    }
}

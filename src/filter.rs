//! Filtres et construction de la requête de recherche GitHub.
//!
//! Seul endroit du programme qui connaît la syntaxe de recherche de GitHub.
//! Ajouter un filtre coûte trois modifications, toutes ici : une variante,
//! une ligne dans `fragment`, une ligne dans `parse`.
//!
//! Le module est pur : ni réseau, ni terminal, ni réglages.

/// Un filtre de recherche. Chaque variante sait produire son fragment de
/// chaîne ; c'est GitHub qui filtre, `owl` ne filtre jamais en mémoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    AuthoredByMe,
    ReviewRequestedFromMe,
    AssignedToMe,
    Open,
    Draft(bool),
    Org(String),
    Repo(String),
    Label(String),
    /// Soupape : n'importe quelle expression de recherche GitHub, transmise
    /// telle quelle, sans attendre qu'un filtre dédié existe.
    Raw(String),
}

/// Toujours en tête : `search` de type `ISSUE` ramène aussi des issues.
const PREFIXE: &str = "is:pr";

/// Toujours en queue : les pull requests récemment actives arrivent en premier.
const SUFFIXE: &str = "sort:updated-desc";

impl Filter {
    /// Fragment de requête de recherche GitHub.
    pub fn fragment(&self) -> String {
        match self {
            Filter::AuthoredByMe => "author:@me".to_string(),
            Filter::ReviewRequestedFromMe => "review-requested:@me".to_string(),
            Filter::AssignedToMe => "assignee:@me".to_string(),
            Filter::Open => "is:open".to_string(),
            Filter::Draft(true) => "draft:true".to_string(),
            Filter::Draft(false) => "draft:false".to_string(),
            Filter::Org(organisation) => format!("org:{organisation}"),
            Filter::Repo(depot) => format!("repo:{depot}"),
            // Les guillemets sont nécessaires : un libellé peut porter une
            // espace, qui séparerait sinon deux termes de recherche.
            Filter::Label(libelle) => format!("label:\"{libelle}\""),
            Filter::Raw(expression) => expression.clone(),
        }
    }

    /// Reconnaît une chaîne du fichier de réglages. Une chaîne non reconnue
    /// devient `Raw`, ce qui la transmet à GitHub telle quelle : mieux vaut
    /// une expression que `owl` ne comprend pas qu'un filtre perdu.
    pub fn parse(texte: &str) -> Filter {
        let texte = texte.trim();

        match texte {
            "author:@me" => return Filter::AuthoredByMe,
            "review-requested:@me" => return Filter::ReviewRequestedFromMe,
            "assignee:@me" => return Filter::AssignedToMe,
            "is:open" => return Filter::Open,
            "draft:true" => return Filter::Draft(true),
            "draft:false" => return Filter::Draft(false),
            _ => {}
        }

        // Filtres à valeur. Un préfixe sans valeur n'est pas un filtre : il
        // part en `Raw` plutôt que de fabriquer un `org:` vide.
        for (prefixe, construire) in [
            ("org:", Filter::Org as fn(String) -> Filter),
            ("repo:", Filter::Repo as fn(String) -> Filter),
            ("label:", Filter::Label as fn(String) -> Filter),
        ] {
            if let Some(valeur) = texte.strip_prefix(prefixe) {
                // Les guillemets sont la forme que produit `fragment` ; les
                // retirer ici est ce qui rend l'aller-retour fidèle.
                let valeur = valeur.trim_matches('"');
                if !valeur.is_empty() {
                    return construire(valeur.to_string());
                }
            }
        }

        Filter::Raw(texte.to_string())
    }
}

/// Assemble les fragments et garantit la présence de `is:pr` et du tri.
///
/// Les fragments vides sont écartés : une chaîne blanche dans les réglages
/// laisserait sinon une double espace au milieu de la requête.
pub fn build_query(filters: &[Filter]) -> String {
    let mut morceaux = Vec::with_capacity(filters.len() + 2);
    morceaux.push(PREFIXE.to_string());
    morceaux.extend(
        filters
            .iter()
            .map(Filter::fragment)
            .filter(|fragment| !fragment.is_empty()),
    );
    morceaux.push(SUFFIXE.to_string());
    morceaux.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaque_filtre_donne_le_fragment_de_la_spec() {
        let cas = [
            (Filter::AuthoredByMe, "author:@me"),
            (Filter::ReviewRequestedFromMe, "review-requested:@me"),
            (Filter::AssignedToMe, "assignee:@me"),
            (Filter::Open, "is:open"),
            (Filter::Draft(true), "draft:true"),
            (Filter::Draft(false), "draft:false"),
            (Filter::Org("acme".to_string()), "org:acme"),
            (Filter::Repo("acme/owl".to_string()), "repo:acme/owl"),
            (Filter::Label("bug".to_string()), "label:\"bug\""),
            (
                Filter::Raw("involves:@me -is:draft".to_string()),
                "involves:@me -is:draft",
            ),
        ];
        for (filtre, attendu) in cas {
            assert_eq!(filtre.fragment(), attendu, "filtre = {filtre:?}");
        }
    }

    #[test]
    fn un_libelle_a_espaces_reste_entre_guillemets() {
        assert_eq!(
            Filter::Label("needs review".to_string()).fragment(),
            "label:\"needs review\""
        );
    }

    #[test]
    fn les_filtres_par_defaut_donnent_la_chaine_de_la_spec() {
        assert_eq!(
            build_query(&[Filter::AuthoredByMe, Filter::Open]),
            "is:pr author:@me is:open sort:updated-desc"
        );
    }

    #[test]
    fn la_requete_porte_toujours_is_pr_en_tete_et_le_tri_en_queue() {
        let requete = build_query(&[Filter::Repo("acme/owl".to_string())]);
        assert_eq!(requete, "is:pr repo:acme/owl sort:updated-desc");

        // Y compris sans aucun filtre : la fonction reste totale. Le refus
        // d'une liste vide appartient à `config`, qui l'applique au démarrage.
        assert_eq!(build_query(&[]), "is:pr sort:updated-desc");
    }

    #[test]
    fn l_ordre_des_filtres_ne_change_pas_l_ensemble_ramene() {
        let un = build_query(&[Filter::AuthoredByMe, Filter::Open, Filter::Draft(false)]);
        let autre = build_query(&[Filter::Draft(false), Filter::Open, Filter::AuthoredByMe]);

        let cadre = |requete: &str| {
            let mots: Vec<String> = requete.split(' ').map(str::to_string).collect();
            (mots.first().cloned(), mots.last().cloned())
        };
        assert_eq!(cadre(&un), cadre(&autre), "is:pr en tête, tri en queue");

        let mut mots_un: Vec<&str> = un.split(' ').collect();
        let mut mots_autre: Vec<&str> = autre.split(' ').collect();
        mots_un.sort_unstable();
        mots_autre.sort_unstable();
        assert_eq!(
            mots_un, mots_autre,
            "les deux requêtes portent les mêmes termes, donc ramènent le même ensemble"
        );
    }

    #[test]
    fn un_fragment_vide_ne_laisse_pas_de_double_espace() {
        let requete = build_query(&[Filter::Raw(String::new()), Filter::Open]);
        assert_eq!(requete, "is:pr is:open sort:updated-desc");
        assert!(!requete.contains("  "), "requete = {requete}");
    }

    #[test]
    fn chaque_fragment_de_la_spec_se_relit_en_son_filtre() {
        let cas = [
            ("author:@me", Filter::AuthoredByMe),
            ("review-requested:@me", Filter::ReviewRequestedFromMe),
            ("assignee:@me", Filter::AssignedToMe),
            ("is:open", Filter::Open),
            ("draft:true", Filter::Draft(true)),
            ("draft:false", Filter::Draft(false)),
            ("org:acme", Filter::Org("acme".to_string())),
            ("repo:acme/owl", Filter::Repo("acme/owl".to_string())),
            ("label:\"bug\"", Filter::Label("bug".to_string())),
        ];
        for (texte, attendu) in cas {
            assert_eq!(Filter::parse(texte), attendu, "texte = {texte}");
        }
    }

    #[test]
    fn un_filtre_se_relit_depuis_son_propre_fragment() {
        let filtres = [
            Filter::AuthoredByMe,
            Filter::ReviewRequestedFromMe,
            Filter::AssignedToMe,
            Filter::Open,
            Filter::Draft(true),
            Filter::Draft(false),
            Filter::Org("acme".to_string()),
            Filter::Repo("acme/owl".to_string()),
            Filter::Label("needs review".to_string()),
        ];
        for filtre in filtres {
            assert_eq!(
                Filter::parse(&filtre.fragment()),
                filtre,
                "aller-retour cassé pour {filtre:?}"
            );
        }
    }

    #[test]
    fn un_libelle_sans_guillemets_est_accepte() {
        assert_eq!(Filter::parse("label:bug"), Filter::Label("bug".to_string()));
    }

    #[test]
    fn une_chaine_inconnue_devient_raw_et_reste_intacte() {
        let inconnue = "involves:@me -is:draft";
        assert_eq!(Filter::parse(inconnue), Filter::Raw(inconnue.to_string()));
        assert!(
            build_query(&[Filter::parse(inconnue)]).contains(inconnue),
            "l'expression doit traverser la requête sans retouche"
        );
    }

    #[test]
    fn un_prefixe_sans_valeur_reste_raw() {
        for texte in ["org:", "repo:", "label:", "draft:peut-etre", "is:closed"] {
            assert_eq!(
                Filter::parse(texte),
                Filter::Raw(texte.to_string()),
                "texte = {texte}"
            );
        }
    }

    #[test]
    fn les_espaces_autour_d_un_filtre_sont_ignorees() {
        assert_eq!(Filter::parse("  is:open  "), Filter::Open);
    }
}

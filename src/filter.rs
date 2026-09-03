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
const PREFIX: &str = "is:pr";

/// Toujours en queue : les pull requests récemment actives arrivent en premier.
const SUFFIX: &str = "sort:updated-desc";

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
            Filter::Org(org) => format!("org:{org}"),
            Filter::Repo(repo) => format!("repo:{repo}"),
            // Les guillemets sont nécessaires : un libellé peut porter une
            // espace, qui séparerait sinon deux termes de recherche.
            Filter::Label(label) => format!("label:\"{label}\""),
            Filter::Raw(expression) => expression.clone(),
        }
    }

    /// Reconnaît une chaîne du fichier de réglages. Une chaîne non reconnue
    /// devient `Raw`, ce qui la transmet à GitHub telle quelle : mieux vaut
    /// une expression que `owl` ne comprend pas qu'un filtre perdu.
    pub fn parse(text: &str) -> Filter {
        let text = text.trim();

        match text {
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
        for (prefix, build) in [
            ("org:", Filter::Org as fn(String) -> Filter),
            ("repo:", Filter::Repo as fn(String) -> Filter),
            ("label:", Filter::Label as fn(String) -> Filter),
        ] {
            if let Some(value) = text.strip_prefix(prefix) {
                // Les guillemets sont la forme que produit `fragment` ; les
                // retirer ici est ce qui rend l'aller-retour fidèle.
                let value = value.trim_matches('"');
                if !value.is_empty() {
                    return build(value.to_string());
                }
            }
        }

        Filter::Raw(text.to_string())
    }
}

/// Assemble les fragments et garantit la présence de `is:pr` et du tri.
///
/// Les fragments vides sont écartés : une chaîne blanche dans les réglages
/// laisserait sinon une double espace au milieu de la requête.
pub fn build_query(filters: &[Filter]) -> String {
    let mut parts = Vec::with_capacity(filters.len() + 2);
    parts.push(PREFIX.to_string());
    parts.extend(
        filters
            .iter()
            .map(Filter::fragment)
            .filter(|fragment| !fragment.is_empty()),
    );
    parts.push(SUFFIX.to_string());
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_filter_yields_the_fragment_from_the_spec() {
        let cases = [
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
        for (filter, expected) in cases {
            assert_eq!(filter.fragment(), expected, "filtre = {filter:?}");
        }
    }

    #[test]
    fn a_label_with_spaces_stays_quoted() {
        assert_eq!(
            Filter::Label("needs review".to_string()).fragment(),
            "label:\"needs review\""
        );
    }

    #[test]
    fn the_default_filters_give_the_string_from_the_spec() {
        assert_eq!(
            build_query(&[Filter::AuthoredByMe, Filter::Open]),
            "is:pr author:@me is:open sort:updated-desc"
        );
    }

    #[test]
    fn the_query_always_carries_is_pr_first_and_the_sort_last() {
        let query = build_query(&[Filter::Repo("acme/owl".to_string())]);
        assert_eq!(query, "is:pr repo:acme/owl sort:updated-desc");

        // Y compris sans aucun filtre : la fonction reste totale. Le refus
        // d'une liste vide appartient à `config`, qui l'applique au démarrage.
        assert_eq!(build_query(&[]), "is:pr sort:updated-desc");
    }

    #[test]
    fn the_order_of_the_filters_does_not_change_the_set_returned() {
        let first = build_query(&[Filter::AuthoredByMe, Filter::Open, Filter::Draft(false)]);
        let second = build_query(&[Filter::Draft(false), Filter::Open, Filter::AuthoredByMe]);

        let bounds = |query: &str| {
            let words: Vec<String> = query.split(' ').map(str::to_string).collect();
            (words.first().cloned(), words.last().cloned())
        };
        assert_eq!(
            bounds(&first),
            bounds(&second),
            "is:pr en tête, tri en queue"
        );

        let mut words_first: Vec<&str> = first.split(' ').collect();
        let mut words_second: Vec<&str> = second.split(' ').collect();
        words_first.sort_unstable();
        words_second.sort_unstable();
        assert_eq!(
            words_first, words_second,
            "les deux requêtes portent les mêmes termes, donc ramènent le même ensemble"
        );
    }

    #[test]
    fn an_empty_fragment_leaves_no_double_space() {
        let query = build_query(&[Filter::Raw(String::new()), Filter::Open]);
        assert_eq!(query, "is:pr is:open sort:updated-desc");
        assert!(!query.contains("  "), "requete = {query}");
    }

    #[test]
    fn each_fragment_from_the_spec_parses_back_to_its_filter() {
        let cases = [
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
        for (text, expected) in cases {
            assert_eq!(Filter::parse(text), expected, "texte = {text}");
        }
    }

    #[test]
    fn a_filter_parses_back_from_its_own_fragment() {
        let filters = [
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
        for filter in filters {
            assert_eq!(
                Filter::parse(&filter.fragment()),
                filter,
                "aller-retour cassé pour {filter:?}"
            );
        }
    }

    #[test]
    fn an_unquoted_label_is_accepted() {
        assert_eq!(Filter::parse("label:bug"), Filter::Label("bug".to_string()));
    }

    #[test]
    fn an_unknown_string_becomes_raw_and_stays_intact() {
        let unknown = "involves:@me -is:draft";
        assert_eq!(Filter::parse(unknown), Filter::Raw(unknown.to_string()));
        assert!(
            build_query(&[Filter::parse(unknown)]).contains(unknown),
            "l'expression doit traverser la requête sans retouche"
        );
    }

    #[test]
    fn a_prefix_without_a_value_stays_raw() {
        for text in ["org:", "repo:", "label:", "draft:peut-etre", "is:closed"] {
            assert_eq!(
                Filter::parse(text),
                Filter::Raw(text.to_string()),
                "texte = {text}"
            );
        }
    }

    #[test]
    fn spaces_around_a_filter_are_ignored() {
        assert_eq!(Filter::parse("  is:open  "), Filter::Open);
    }

    #[test]
    fn is_pr_written_in_the_settings_appears_twice_in_the_query() {
        assert_eq!(
            build_query(&[Filter::parse("is:pr")]),
            "is:pr is:pr sort:updated-desc"
        );
    }
}

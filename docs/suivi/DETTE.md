# Dette technique

Registre des points mis de côté pendant l'exécution des plans. Un point y entre
quand il est identifié puis délibérément différé — pas quand il est corrigé.

Ce fichier n'est pas un historique : une entrée résolue en sort.

## Format

Une entrée par point, la plus récente en haut.

```markdown
### <titre court>

- **Origine** : plan ou tâche d'où le point sort.
- **Ce qui est différé** : l'état courant, décrit tel quel.
- **Pourquoi** : la raison du report.
- **Ce qu'il faudrait faire** : la piste de correction, assez précise pour être reprise sans rouvrir l'analyse.
```

---

### Relecture des serveurs à chaque ouverture du panneau non garantie

- **Origine** : Plan `2026-10-15-serveurs-node`, relecture finale.
- **Ce qui est différé** : `PanelView` relit les serveurs dans `.onAppear` ; dans un `MenuBarExtra` en mode fenêtre, cet événement peut ne venir qu'à la première ouverture selon la version de macOS.
- **Pourquoi** : Non vérifiable sans piloter le panneau ; le bouton de rechargement et ⌘2 relisent de toute façon.
- **Ce qu'il faudrait faire** : Vérifier à la main en rouvrant le panneau sur Servers ; si rien n'est relu, appeler `state.panelOpened()` sur `NSWindow.didBecomeKeyNotification` de la fenêtre du panneau.

### Avertissements de lsof pris pour une panne

- **Origine** : Plan `2026-10-15-serveurs-node`, relecture finale.
- **Ce qui est différé** : `ports::system::outcome` traite une sortie vide accompagnée d'un message sur stderr et d'un code d'échec comme une erreur `system` ; un « WARNING: can't stat() » (volume réseau injoignable) en produit une.
- **Pourquoi** : Cas rare : des services système écoutent presque toujours.
- **Ce qu'il faudrait faire** : Ajouter `-w` aux deux appels de `lsof` dans `src/ports/system.rs`.

### Arrêt forcé possible sur un processus zombie

- **Origine** : Plan `2026-10-15-serveurs-node`, relecture finale.
- **Ce qui est différé** : Après 5 s, `stop` envoie `SIGKILL` à tout membre encore « vivant » ; un zombie pas encore ramassé par son parent compte comme vivant, d'où un `forced: true` trompeur.
- **Pourquoi** : Sans effet nuisible : le signal est sans effet sur un zombie.
- **Ce qu'il faudrait faire** : Avant l'arrêt forcé, relire `ps` et ne viser que les membres dont le parent et le temps écoulé sont cohérents avec la première lecture.

### Arrêt interrompu à mi-chaîne sur un refus

- **Origine** : Plan `2026-10-15-serveurs-node`, relecture finale.
- **Ce qui est différé** : Si le système refuse le signal à un membre du milieu de la chaîne, les membres du haut ont déjà reçu `SIGTERM` et la réponse est une erreur `permission`.
- **Pourquoi** : Toute la chaîne appartient au même utilisateur : le cas est presque impossible.
- **Ce qu'il faudrait faire** : Envoyer d'abord un signal `0` à toute la chaîne pour détecter un refus avant de couper quoi que ce soit.

### Petits défauts de l'onglet Servers

- **Origine** : Plan `2026-10-15-serveurs-node`, relecture finale.
- **Ce qui est différé** : « Not a port » garde les espaces de la saisie ; `submitSearch` ne vérifie pas que la fenêtre de confirmation est fermée ; `selectTab` efface le message bref des PR mais pas `servers.notice`.
- **Pourquoi** : Effets mineurs, sans perte de données ni arrêt non voulu.
- **Ce qu'il faudrait faire** : Dans `Servers.swift`, utiliser `trimmed` pour `.invalid` et ajouter `guard dialog == nil` en tête de `submitSearch` ; dans `BarState.selectTab`, effacer aussi le message de `servers`.

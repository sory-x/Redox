# Changements effectués pour SoryOS — `redox/`

Ce fichier documente toutes les modifications apportées au dossier `redox/`
pour l'adapter au projet SoryOS.

> **Règle :** chaque changement fait dans `redox/` doit être documenté ici,
> fichier par fichier, partie par partie, avec son **Avant/Après** et toute
> **erreur potentielle** liée au changement pour référence rapide.

---

## Historique des changements

### 2026-08-01 — Initialisation du dossier

- Création du dossier `docs-changes/` (ce fichier).
- Aucun changement de code effectué à ce stade.

### 2026-08-01 — Recette schedrs : bascule vers le fork SoryOS + config filesystem dédiée

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `recipes/tests/schedrs/recipe.toml` | `[source]` | `git = "https://gitlab.redox-os.org/akshitgaur2005/schedrs.git"` → `https://gitlab.com/sory-os/schedrs.git` | Recette identique au cookbook redox officiel (vérifié sur `redox-os/redox`). Fork créé via API GitLab (projet `sory-os/schedrs`, namespace id 138735240, `git push --mirror`). Le repo source reste accessible mais le fork garantit la stabilité/la souveraineté. |
| `config/soryos.toml` | — | **Nouveau** : config filesystem listant les **244** recettes non-`wip/` du manifeste SoryOS (`include = ["base.toml"]` + `[packages]`), générée depuis `sory-os-apt/redox-apps/manifest.json` | Utilisé par `repo cook --filesystem=config/soryos.toml --repo-binary` (équivalent de `make repo` avec `COOKBOOK_OPTS`). |

Rappel mécanique (sources) :

- `repo cook` télécharge les sources avant cuisson : `src/bin/repo/main.rs:381`.
- Clone git : `src/cook/fetch.rs:221` ; tarballs vérifiés blake3 : `src/cook/fetch.rs:111-125`.
- `cook --all` parcourt tout `recipes/` (y compris `wip/`) : `src/staged_pkg.rs:15-37`.
- `cook --filesystem=<config>` lit la liste des paquets depuis `conf.packages` : `src/bin/repo/main.rs:590-598`.
- Publication : `cook` spawn `repo_builder` (`main.rs:444-462`) qui assemble `repo/<target>/` (`src/bin/repo_builder.rs:46-77`).

### 2026-08-02 — Corrections de problèmes identifiés

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `.gitignore` | `/cookbook.toml` | Retiré `/cookbook.toml` du `.gitignore` | Plus besoin de `git add -f` pour suivre `cookbook.toml`. Le fichier `.gitignore` ne l'ignore plus. |
| `config/base.toml` | `[[files]] path = "/etc/pkg.d/50_redox"` | Ancienne migration runtime vers Pages | Historique : cette URL est conservée provisoirement tant que `sory-os/pkgutils` ne sait pas lire l’index Release signé. |
| `recipes/other/jeremy/recipe.toml` | — | **Supprimé** (repo privé `gitlab.redox-os.org/jackpot51/jeremy.git`, inaccessible en CI, non référencé par aucune config) | La recette n'était pas utilisée par `soryos.toml` ni par aucune config de build. Sa suppression n'affecte aucun build. |
| `config/soryos.toml` | Commentaire | `304 recettes` → `244 packages` | Le compteur dans le commentaire correspondait au nombre de recettes non-`wip/` du manifeste (244), pas 304. |

### 2026-08-02 — Fix : conflit d'override 30_console (orbital vs inputd)

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `config/desktop.toml` | `[[files]]` | Ajout override `30_console` avec `requires_weak 20_orbital` | `server.toml` inclut `minimal.toml` qui redéfinit `30_console` (97B, `inputd -A 2`) après `desktop-minimal.toml` (82B, `requires_weak 20_orbital`). La version finale écrasée n'avait PAS de dépendance sur Orbital, causant un race condition : `getty` démarrait avant qu'Orbital soit prêt, empêchant le panel/launcher de s'afficher correctement. |

---

## Répertoire des fichiers modifiés

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `recipes/tests/schedrs/recipe.toml` | `[source]` | URL git → fork `gitlab.com/sory-os/schedrs.git` | — |
| `config/soryos.toml` | — | Nouvelle config filesystem (244 recettes) | Doit rester synchronisé avec `sory-os-apt/redox-apps/manifest.json` |
| `.gitignore` | `/cookbook.toml` | Retiré de l'ignore — le fichier est maintenant tracké normalement | — |
| `config/base.toml` | `/etc/pkg.d/50_redox` | URL runtime Pages héritée | Ne pas l’utiliser pour le build ; le build strict passe par l’index GitHub Release. |
| `recipes/other/jeremy/recipe.toml` | — | **Supprimé** (repo privé, inutilisé) | — |
| `config/soryos.toml` | Commentaire | `304` → `244` | — |
| `config/desktop.toml` | `[[files]]` | Ajout override `30_console` avec `requires_weak 20_orbital` | Corrige le race condition getty/orbital |

---

## Répertoire des erreurs connues

> Section à remplir à chaque changement. Toute erreur observée en lien avec un
> changement doit être référencée ici.

| Erreur | Fichier | Ligne | Cause | Correctif |
|--------|---------|-------|-------|-----------|
| Panel/launcher Orbital ne s'affiche pas au boot (taskbar vide ou absente) | `config/desktop.toml` | — | `server.toml` inclut `minimal.toml` qui redéfinit `30_console` (97B, `inputd -A 2`) après `desktop-minimal.toml` (82B, `requires_weak 20_orbital`). La version finale écrasée n'avait PAS de dépendance sur Orbital, causant un race condition : `getty` démarrait avant qu'Orbital soit prêt. | Ajout override `30_console` dans `desktop.toml` avec `requires_weak 20_orbital` pour garantir qu'Orbital démarre avant `getty`. |

### 2026-08-03 — Diagnostic : argument d'affichage Orbital invalidé

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `config/desktop-minimal.toml` | `/usr/lib/init.d/20_orbital` | Test de `orbital display:3/activate orblogin launcher` dans QEMU | Échec : Orbital termine avec `could not open display, caused by: No such device (os error 19)`. Retour à `orbital orblogin launcher`, commande compatible avec le fork utilisé. |
| `config/desktop-contain.toml` | `/usr/lib/init.d/20_orbital` | Même vérification pour `contain_orblogin` | Retour à la commande sans argument `display:3/activate`. |

### 2026-08-03 — Bureau complet Redox intégré au profil SoryOS

| Fichier | Section | Changement | Résultat |
|---------|---------|------------|----------|
| `config/desktop.toml` | session graphique | Ajout de D-Bus, X11, Xfce, `orbital-x11`, `xfce4-x11-session`, environnement X11 et scripts du profil original `config/x11.toml` | Le profil `desktop` démarre maintenant une session complète avec panneau, fenêtres, menu et bureau Xfce. |
| `config/soryos-x11.toml` | nouveau profil | Profil séparé complet basé sur le profil X11 original, conservant les applications SoryOS | Permet de tester le bureau complet avec `CONFIG_NAME=soryos-x11`. |

Les URLs des recettes SoryOS forkées n'ont pas été remplacées par celles de Redox original.
Les composants originaux du bureau ont été ajoutés dans la configuration du fork.

### 2026-08-06 — Suppression du faux miroir Pages pour le build

| Fichier | Section | Changement | Résultat |
|---------|---------|------------|----------|
| `cookbook.toml` | `[mirrors]` | Suppression de `static.redox-os.org/pkg` → `sory-x.github.io/soryos-apt` | Le cookbook ne traite plus Pages comme dépôt binaire de secours. En mode Release strict, un paquet absent de l’index signé arrête le build. |

Le runtime `pkg` reste en transition : le fork `sory-os/pkgutils` lit encore le
format historique `repo.toml`. Il devra lire `index.json`, vérifier sa
signature Ed25519, puis résoudre les URLs des assets Release. Tant que ce
raccord n’est pas livré dans ce fork, `/etc/pkg.d/50_redox` est conservé pour
ne pas produire une image où `pkg` échoue immédiatement. Cela n’affecte pas la
récupération Release pendant la construction de l’ISO.

### 2026-08-06 — Backend Release natif de `pkgutils`

Le fork local `sory-os/pkgutils` contient maintenant un backend Release dans
`pkg-lib/src/repo_manager.rs`. Une entrée `/etc/pkg.d/*.json` terminée par
`/index.json` est reconnue comme dépôt strict. Le gestionnaire vérifie :

- le dépôt GitHub et le tag Release immuable ;
- la signature Ed25519 de `index.json` sans OpenSSL au runtime ;
- les URLs d’assets limitées à cette Release ;
- la taille et le BLAKE3 des métadonnées et archives ;
- la clé publique PKGAR publiée par l’index.

Les archives sont téléchargées dans un fichier temporaire et renommées après
vérification. La vérification BLAKE3 des `.pkgar` est faite par blocs de 1 MiB
pour éviter de charger une grosse archive en RAM. Le code local doit encore
être publié dans le dépôt GitLab `sory-os/pkgutils` pour que la recette Redox
qui pointe vers ce dépôt l’utilise dans la CI.

---

## Notes de référence

- **Point d'entrée du build** : `Makefile:8` → `all: $(BUILD)/harddrive.img`.
- **Installeur mode local** : `mk/config.mk:175` → `INSTALLER_OPTS=--cookbook=. --config-name=$(CONFIG_NAME)`.
- **Dépôt binaire local** : `repo/<arch>/` (`.pkgar` + `.toml` + `repo.toml`).
- **Source binaire de build SoryOS** : `src/cook/fetch_repo.rs` → index Release signé et assets GitHub Release.
- **Point de bascule miroir historique** : `src/config.rs` → `translate_mirror()` ; aucune entrée Pages de paquets ne doit être ajoutée dans `cookbook.toml`.
- **Clés de signature** : `build/id_ed25519.toml` + `build/id_ed25519.pub.toml` (générées par `src/cook/package.rs:42-51`).

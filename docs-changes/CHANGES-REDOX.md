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
| `config/soryos.toml` | — | **Nouveau** : config filesystem listant les **304** recettes non-`wip/` du manifeste SoryOS (`include = ["base.toml"]` + `[packages]`), générée depuis `sory-os-apt/redox-apps/manifest.json` | Utilisé par `repo cook --filesystem=config/soryos.toml --repo-binary` (équivalent de `make repo` avec `COOKBOOK_OPTS`). `jeremy` (dépôt privé) n'y figure pas. |
| `recipes/other/jeremy/recipe.toml` | — | Inchangé mais recette **non référencée** par la config filesystem | Dépôt privé (`gitlab.redox-os.org/jackpot51/jeremy.git`) → clone impossible en CI. Hérité tel quel du cookbook officiel. |

Rappel mécanique (sources) :

- `repo cook` télécharge les sources avant cuisson : `src/bin/repo/main.rs:381`.
- Clone git : `src/cook/fetch.rs:221` ; tarballs vérifiés blake3 : `src/cook/fetch.rs:111-125`.
- `cook --all` parcourt tout `recipes/` (y compris `wip/`) : `src/staged_pkg.rs:15-37`.
- `cook --filesystem=<config>` lit la liste des paquets depuis `conf.packages` : `src/bin/repo/main.rs:590-598`.
- Publication : `cook` spawn `repo_builder` (`main.rs:444-462`) qui assemble `repo/<target>/` (`src/bin/repo_builder.rs:46-77`).

---

## Répertoire des fichiers modifiés

| Fichier | Section | Changement | Erreurs potentielles |
|---------|---------|------------|----------------------|
| `recipes/tests/schedrs/recipe.toml` | `[source]` | URL git → fork `gitlab.com/sory-os/schedrs.git` | — |
| `config/soryos.toml` | — | Nouvelle config filesystem (304 recettes, ex. `jeremy`) | Doit rester synchronisé avec `sory-os-apt/redox-apps/manifest.json` |

---

## Répertoire des erreurs connues

> Section à remplir à chaque changement. Toute erreur observée en lien avec un
> changement doit être référencée ici.

| Erreur | Fichier | Ligne | Cause | Correctif |
|--------|---------|-------|-------|-----------|
| *(à remplir)* | | | | |

---

## Notes de référence

- **Point d'entrée du build** : `Makefile:8` → `all: $(BUILD)/harddrive.img`.
- **Installeur mode local** : `mk/config.mk:175` → `INSTALLER_OPTS=--cookbook=. --config-name=$(CONFIG_NAME)`.
- **Dépôt binaire local** : `repo/<arch>/` (`.pkgar` + `.toml` + `repo.toml`).
- **Source binaire distant** : `src/lib.rs:12` → `REMOTE_PKG_SOURCE = "https://static.redox-os.org/pkg"`.
- **Point de bascule miroir** : `src/config.rs:217` → `translate_mirror()` (via `cookbook.toml` `[mirrors]`).
- **Clés de signature** : `build/id_ed25519.toml` + `build/id_ed25519.pub.toml` (générées par `src/cook/package.rs:42-51`).

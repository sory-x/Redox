# Comparaison desktop SoryOS / Redox original

Analyse réalisée entre :

- le fork actif : `/home/sory/Bureau/sory-os/redox` ;
- la référence : `/home/sory/Bureau/sory-os/dossier-etude-pour-ameliorer-le-projet/redox`.

## Preuves recueillies

### 1. Le kernel et Orbital démarrent

Les logs QEMU montrent successivement :

- framebuffer `1280x800` ;
- `vesad` et `fbbootlogd` ;
- `init: switchroot to /usr /etc` ;
- `orbital`/`orblogin` jusqu'à l'ouverture de la session.

Il n'y a donc pas de panne initiale du kernel, du framebuffer ou du pilote
NVMe qui expliquerait à elle seule le fond d'écran sans bureau.

### 2. L'image testée n'est pas garantie à jour

Le fichier `build/x86_64/desktop/harddrive.img` est antérieur à la dernière
modification de `config/desktop.toml`. La commande `make ... qemu` démarre une
image existante ; elle ne garantit pas que la configuration courante a été
réinstallée dans cette image.

### 3. Le mode binaire externe masque les changements locaux

Avec `REPO_BINARY=1`, le cookbook sélectionne les `.pkgar` du dépôt distant
SoryOS. Les hash du dépôt distant et des artefacts locaux correspondent pour
Orbital, Orbutils, Cosmic Term, Cosmic Files, Cosmic Edit, Cosmic Reader,
Netsurf et `installer-gui`.

Le fork possède maintenant `REPO_BINARY_STRICT=1`. Dans ce mode, un paquet
absent du dépôt publié provoque une erreur immédiate ; il n'est plus remplacé
silencieusement par une compilation locale. Le workflow de construction de
l'ISO active ce mode.

L'artefact `installer-gui` actuellement utilisé contient encore :

```text
icon=/usr/share/icons/Pop/48x48/apps/system-os-installer.svg
```

alors que le manifest source corrigé demande une icône PNG. La correction
locale ne peut donc pas modifier une image construite avec l'ancien paquet
publié.

### 4. Les configurations desktop ne sont pas identiques

Par rapport à l'original, le fork actif ajoute `cosmic-reader`, conserve
`orbterm` au lieu de l'ignorer et agrandit le système de 650 MiB à 2048 MiB.
Ces différences doivent être testées séparément après validation du profil
minimal original.

Le fork conserve aussi la correction nécessaire de `30_console` :
`requires_weak 20_orbital`. Elle évite que `getty` démarre avant Orbital.

## Conclusion provisoire

La première rupture démontrée est un décalage entre les sources/configurations
locales et les paquets binaires réellement installés dans l'image. La seconde
est le démarrage d'une image QEMU obsolète. Le problème X11/D-Bus observé dans
certains logs appartient au profil X11 ou à une ancienne image ; il ne doit
pas être corrigé dans le profil Orbital natif.

La correction doit donc suivre cet ordre :

1. construire un profil desktop minimal identique à l'original ;
2. rafraîchir/republier les paquets SoryOS ;
3. reconstruire l'image complète ;
4. vérifier le launcher ;
5. réintroduire `cosmic-reader`, `orbterm` et les autres applications une par
   une.

## Audit complémentaire des scripts et de la CI

Les points suivants restent à corriger avant de considérer l'architecture
comme fiable :

1. `REPO_BINARY_STRICT` n'est pas transmis par `mk/podman.mk` au conteneur.
   Avec `PODMAN_BUILD=1`, le contrôle strict peut donc disparaître et le
   fallback vers les sources reste possible dans le conteneur.

2. `make image` supprime l'image mais pas `build/.../repo.tag`. Un dépôt déjà
   marqué comme construit peut donc être réutilisé sans relancer la
   vérification des paquets publiés. `make rebuild` est actuellement le seul
   chemin qui supprime explicitement ce marqueur.

3. `src/cook/fetch_repo.rs` synchronise la clé distante avant de lire le cache
   local et supprime le cache après huit heures. Un assemblage prétendument
   local peut donc encore demander le réseau, même quand le dépôt binaire est
   déjà présent.

4. Le test `curl -sI ... | head -1` du workflow ISO n'utilise pas `-f` et ne
   vérifie pas réellement le code HTTP. Une URL 404 peut être considérée comme
   valide avant le build.

5. Les modifications non commitées du fork ne sont pas utilisées par les
   workflows distants. Le dépôt `sory-os-apt` clone `sory-x/Redox` sur
   `main`; les corrections de l'icône, du contrôle strict et du desktop ne
   seront donc actives dans la CI qu'après commit et publication sur cette
   branche.

6. Le Release Cosmic est correctement préparé comme archive, mais le chemin
   standard `fetch_repo` ne sait pas lire directement une archive GitHub
   Release. Pour l'assemblage normal, les paquets Cosmic doivent donc aussi
   exister dans GitHub Pages, ou un extracteur explicite doit être ajouté.

7. La validation actuelle contrôle les manifests générés dans `recipes/*/target`,
   mais pas le contenu réel du dépôt publié ni la correspondance entre le
   commit du cookbook, les hashes `.pkgar` et l'image assemblée.

Les corrections appliquées depuis cet audit transmettent désormais les deux
variables de contrôle à Podman, suppriment `repo.tag` pendant `make image`,
permettent de forcer le rafraîchissement du cache avec
`REPO_BINARY_REFRESH=1` et rendent le contrôle HTTP du workflow bloquant.

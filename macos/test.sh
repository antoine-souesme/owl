#!/bin/sh
# Lance les tests de l'app. Sans Xcode, Swift Testing vit dans les outils en
# ligne de commande, hors des chemins que le compilateur explore seul ; et le
# système de compilation par défaut de Swift 6.4 échoue sur ce Mac, d'où le
# système natif.
set -eu
cd "$(dirname "$0")"
frameworks=/Library/Developer/CommandLineTools/Library/Developer/Frameworks
exec swift test --build-system native \
    -Xswiftc -F -Xswiftc "$frameworks" \
    -Xlinker -F -Xlinker "$frameworks" \
    "$@"

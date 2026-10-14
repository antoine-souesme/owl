#!/bin/sh
# Construit owl et l'app de la barre des menus, glisse owl dans Owl.app, signe
# le tout localement, remplace l'app installée dans ~/Applications et la
# relance. Rien n'est installé dans le terminal : l'app est autonome.
set -eu
cd "$(dirname "$0")"

cargo build --release --manifest-path ../Cargo.toml
swift build -c release --build-system native
binaries="$(swift build -c release --build-system native --show-bin-path)"

app="build/Owl.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binaries/OwlBar" "$app/Contents/MacOS/OwlBar"
cp ../target/release/owl "$app/Contents/MacOS/owl"
cp Resources/Info.plist "$app/Contents/Info.plist"
# La version de l'app est celle de owl, lue dans Cargo.toml.
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' ../Cargo.toml | head -1)"
plutil -replace CFBundleShortVersionString -string "$version" "$app/Contents/Info.plist"
# Le programme embarqué d'abord, l'app ensuite : la signature de l'app couvre
# ce qu'elle contient.
codesign --force --sign - "$app/Contents/MacOS/owl"
codesign --force --sign - "$app"

# L'ancienne version est arrêtée avant d'être remplacée, puis la nouvelle
# est relancée.
pkill -x OwlBar || true
mkdir -p "$HOME/Applications"
rm -rf "$HOME/Applications/Owl.app"
cp -R "$app" "$HOME/Applications/"
open "$HOME/Applications/Owl.app"
echo "Installed and launched: $HOME/Applications/Owl.app"

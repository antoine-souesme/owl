#!/bin/sh
# Compile l'app de la barre des menus, assemble Owl.app, la signe localement
# et l'installe dans ~/Applications.
set -eu
cd "$(dirname "$0")"

swift build -c release --build-system native
binaries="$(swift build -c release --build-system native --show-bin-path)"

app="build/Owl.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binaries/OwlBar" "$app/Contents/MacOS/OwlBar"
cp Resources/Info.plist "$app/Contents/Info.plist"
codesign --force --sign - "$app"

mkdir -p "$HOME/Applications"
rm -rf "$HOME/Applications/Owl.app"
cp -R "$app" "$HOME/Applications/"
echo "Installed: $HOME/Applications/Owl.app"

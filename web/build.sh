#!/bin/bash

# Copy styles and static html
mkdir -p ../webroot

cp -r ./styles/* ../webroot
cp ./join.html ../webroot/join.html
cp ./new.html ../webroot/new.html
cp ./present.html ../webroot/present.html

# Begin build elm source files
cd ./elm
# Build Presenter view
elm make src/Present.elm --output ../../webroot/present.js
# Build Join view
elm make src/Join.elm --output ../../webroot/join.js

cd ../../
# Read version from cargo toml and replace built html titles
if [ -f Cargo.toml ]; then
    version=$(grep -E '^version' Cargo.toml | awk -F '"' '{print $2}')
fi

ls webroot

# sed to edit the title tag for all webroot build .html files
if [ "$(uname)" = "Darwin" ]; then
    for file in webroot/*.html; do
        sed -i '' "s/<title>.*<\/title>/<title>Exhibit v$version<\/title>/" "$file"
    done
else
    for file in webroot/*.html; do
        sed -i "s/<title>.*<\/title>/<title>Exhibit v$version<\/title>/" "$file"
    done
fi
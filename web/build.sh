#!/bin/bash
set -x

# Copy styles and static html
mkdir -p ../webroot

cp ./styles/* ../webroot
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
    echo "Version: $version"
fi
# sed to edit the title tag for all webroot build .html files
for file in "webroot/*.html"; do
    # Use 
    sed -i '' "s/<title>.*<\/title>/<title>Exhibit v$version aaa<\/title>/" "$file"
done
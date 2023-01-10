#!/usr/bin/env sh

# Install the pscm standard library into ~/.pscm
# This script is intended to be run from the root of the pscm repository.

$PSCM_HOME = $HOME/.pscm-rs
$LIB_DIR = $PSCM_HOME/lib

mkdir -p $LIB_DIR
cp -r lib/* $LIB_DIR
cd $LIB_DIR
for file in *.p.scm; do
		echo $file
		mv -- "$file" "${file%%.p.scm}"
done

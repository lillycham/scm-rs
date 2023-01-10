#!/usr/bin/env fish
set PSCM_HOME $HOME/.pscm-rs
set LIB_DIR $PSCM_HOME/lib

mkdir -p $LIB_DIR
cp -r libscm/* $LIB_DIR
cd $LIB_DIR
for file in *.p.scm;
	echo $file
	mv -- $file (echo $file | sed 's/\.p\.scm//')
end
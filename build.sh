#!/usr/bin/env sh

if [ ! -d bin ]
then
	mkdir bin
fi

echo "Compiling shell..."
cd kennsh
cargo build --release
if [ $? -ne 0 ]
then
	exit $?
fi
cp target/release/kennsh ../bin/kennsh
cd ..

echo

echo "Compiling client..."
cd client
chmod +x build.sh
./build.sh
cp client ../bin/client
cd ..

echo "The compiled files are in the bin folder"


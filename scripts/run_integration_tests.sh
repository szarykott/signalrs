#!/bin/bash

script_location=$(readlink -f ${BASH_SOURCE[0]})
script_dirname=$(dirname ${script_location})
server_dir=$script_dirname/../dotnet/server-sample 
publish_dir=$script_dirname/../target/dotnet_server

# build .NET server
dotnet build -c Release $server_dir
if [ $? -ne 0 ]
then
    exit 1
fi

dotnet publish --no-build -c Release --output $publish_dir $server_dir
dotnet $publish_dir/server-sample.dll &
server_pid=$!

# let .NET server start
sleep 2

# run Rust integration tests
cargo test --all-features --manifest-path $script_dirname/../lib/client-integration-tests/Cargo.toml 
if [ $? -ne 0 ]
then
    kill $server_pid
    exit 1
fi

kill $server_pid
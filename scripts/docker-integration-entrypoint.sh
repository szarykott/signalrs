#!/bin/bash

/server/server-sample &
server_pid=$!
sleep 2

cargo test --all-features --manifest-path /lib/client-integration-tests/Cargo.toml 
if [ $? -ne 0 ]
then
    kill $server_pid
    exit 1
fi

kill $server_pid
exit 0
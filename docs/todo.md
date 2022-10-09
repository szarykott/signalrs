# signalrs

SignalR protocol implementation in Rust

# References

[HubProtcol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md)

[TransportProtocol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/TransportProtocols.md)

# Features

1. Client
   1. Send up to 13 args DONE
   1. Invoke up to 13 args
   1. Stream up to 13 args
   1. Client side streams
   1. Client builder
      1. for tests
      1. for web (implicit websocket)
   1. Client hub
   1. Proper cleanup (cooperative cancellation) 
2. Broadcast to other clients
3. Proper integration with tracing
4. Documentation
5. Macros crate for derive macro

# Examples

1. frontend - backend chat example
1. server to server (dotnet -> rust)

# Tests

1. Concurrency tests
1. Tests from MSFT repo

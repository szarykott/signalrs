# SignalRs

SignalR implementation in Rust.

**It is a work in progress**

I am working on it in my free time and I still discover areas I need to implement, which I missed previously. But still progressing.

I need to come up with a nice logo!

Oh, and I have no idea if this is blazingly fast (yet).

# Features

Plan is to support **in MVP**:
- both client and server
    - includes stuff necessary to have two-way communication (client invoking server methods and server invoking client methods)
- WebSocket text communication **only** (no binary, no long polling, no sse)
- integration with Axum **only**
- tight integration with Tokio (I desperately need tokio::spawn)

Obviously, if this create takes off, other stuff can be implemented.

# Progress

- Server is mostly fuctional protocol-wise in single client scenarios (no serious concurrent tests yet)
- Client is a work in progress
- Axum integration is mostly ready
- WebSocket text fully supported 

# Testing

- Unit tests within Rust code
- Cross-platform testing with:
    - .NET client implementation
    - JS client implementation

# References
Internals of the protocol were implemented based on following:

- Docs:
    - [HubProtcol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md)
    - [TransportProtocol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/TransportProtocols.md)
- Reverse engineering of .NET implementation

# SignalRs

SignalRs is Rust's implementation of SignalR protocol.

SignalR is an open-source protocol that simplifies adding real-time web functionality to apps.
Real-time web functionality enables server-side code to push content to clients instantly.

This library is an open source implementation of this protocol's client in Rust.
Originally developed at Microsoft in .NET ecosystem. Read more about it in [`offical documentation`](https://learn.microsoft.com/en-us/aspnet/core/signalr/introduction?view=aspnetcore-7.0).

In technical terms it is a RPC framework with bidirectional streaming capabilities.

**See docs.rs or examples folders of appropriate crates to see how to use it.**

 ## Why SignalR

 ### Ergonomics

 It allows bidirectional communication with ergonimic programming model.
 In cases where real-time communication is required it provides an easy to use framework, abstracting underlying transport layer.
 Getting WebSockets right is not an easy task.

 ### Integration with existing services

Since SignalR originated in .NET ecosystem, there are services that expose SignalR endpoints. This library allows easy integration with them.
This might be especially true for internal tooling at companies that do mostly C#. Truth to be told, it was a reason this library was created in the first place.


# Features 🚀

## Client

Client currently supports:
- call servers with value and stream arguments
- send a message and:
  - do not wait for a response
  - wait for a single response
  - wait for stream of responses
- have a client-side hub that supports
  - value arguments
- WebSockets
- JSON

Client does not support (yet):
- stream arguments to client-side hubs
- LongPolling
- ServerSentEvents
- MsgPack

To view source code of the client see `signalrs-client` crate in this repository.

## Server

Server is a work in progress. It is pretty much advanced and runs, but is not released yet.
To view its source code see `signalrs-next` crate in this repository.

# Testing

- Cross-platform testing with:
    - .NET client implementation

# References
Internals of the protocol were implemented based on following:

- Docs:
    - [HubProtcol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/HubProtocol.md)
    - [TransportProtocol](https://github.com/dotnet/aspnetcore/blob/main/src/SignalR/docs/specs/TransportProtocols.md)
- Talk [SignalR Deep Dive: Building Servers - David Fowler & Damian Edwards](https://youtu.be/iL9nLAjCPtM)

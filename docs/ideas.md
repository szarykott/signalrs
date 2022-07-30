## How to handle method response abstraction?

```
fn single_result(...) -> <what here> {
    42
}

fn stream_result(...) -> <what here> {
    <impl stream>
}
```

### Options

#### Enum as response

```rust
pub enum HubMethodResult<T> {
    Value(T),
    Stream(Box<dyn Stream<Item = T>>),
}
```

Pros:
- less abstract
- can implement From
- uniform handling for e.g Results between single resp and stream
- single handler impl for all

Cons:
- trait specialisation requires that there is some concrete stream implementation for hubs
- cannot have blanket implementation for streams
- builder cannot really distinguish between single and streaming result

#### Separate structs for single and stream

```rust
struct Single<T> {
    v: T
}

or even just 

T

and

struct Streaming {
    v: Box<dyn Stream<Item = T>>
}
```

Pros:
- easy to implement different behavior for single and stream
- could implement 


Cons:
- a lot of repeated code to implement proper logic for Result for instance
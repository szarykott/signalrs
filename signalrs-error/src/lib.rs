pub enum SignalRError {
    ProtocolViolation(String),
    Transport(String),
}

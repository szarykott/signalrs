#[derive(Debug)]
pub enum SignalRError {
    ProtocolViolation(String),
    Transport(String),
}

impl std::fmt::Display for SignalRError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::error::Error for SignalRError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl serde::de::Error for SignalRError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        todo!()
    }
}

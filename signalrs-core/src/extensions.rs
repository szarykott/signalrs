pub trait BoxExt {
    fn into_box(self) -> Box<Self>;
}

impl<T> BoxExt for T {
    fn into_box(self) -> Box<Self> {
        Box::new(self)
    }
}

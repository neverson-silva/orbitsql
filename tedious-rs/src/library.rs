pub const NAME: &str = "Tedious";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Library {
    pub name: &'static str,
}

pub const LIBRARY: Library = Library { name: NAME };

pub const fn library() -> Library {
    LIBRARY
}

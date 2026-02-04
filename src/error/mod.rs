use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("not a field declaration")]
    NotFieldDecl,
    #[error("not a struct or class")]
    NotStructOrClass,
    #[error("no name")]
    NoName,
    #[error("no offset")]
    NoOffset,
    #[error("no size")]
    NoSize,
    #[error("no alignment")]
    NoAlignment,
    #[error("no type")]
    NoType,
}

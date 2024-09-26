use thiserror::Error;

#[derive(Debug, Error)]
pub enum NumberFieldError {
    #[error("{0} field not present in message")]
    FieldNotPresent(String),
    #[error("{0} field was error value")]
    FieldError(String),
    // #[error(transparent)]
    // TryFromSliceError(std::array::TryFromSliceError)
}

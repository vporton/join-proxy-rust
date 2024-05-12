use actix_web::{http::{header::ContentType, StatusCode}, HttpResponse, ResponseError};
use thiserror::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Error)]
pub enum MyError {
    Anyhow(Box<anyhow::Error>),
    Io(std::io::Error),
}

impl Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anyhow(e) => Debug::fmt(&*e, f),
            Self::Io(e) => Debug::fmt(&*e, f),
        }
    }
}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> Self {
        Self::Anyhow(Box::new(err))
    }
}

impl From<std::io::Error> for MyError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl ResponseError for MyError {
    fn status_code(&self) -> StatusCode {
        // if self.err.downcast_ref::<AuthenticationFailedError>().is_some() {
        //     StatusCode::UNAUTHORIZED
        // } else if self.err.downcast_ref::<KYCError>().is_some() {
        //     StatusCode::UNAUTHORIZED
        // } else {
        StatusCode::INTERNAL_SERVER_ERROR // TODO
        // }
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(ContentType::plaintext())
            .body(format!("{}", self.to_string()))
    }
}

pub type MyResult<T> = Result<T, MyError>;
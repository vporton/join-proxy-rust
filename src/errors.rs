use actix_web::{http::{header::ContentType, StatusCode}, HttpResponse, ResponseError};
use thiserror::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Error)]
pub struct MyError {
    err: Box<anyhow::Error>,
}

impl Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&*self.err, f)
    }
}

impl From<anyhow::Error> for MyError {
    fn from(err: anyhow::Error) -> MyError {
        MyError { err: Box::new(err) }
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
            .body(format!("{}\n{}", self.to_string(), self.err.backtrace()))
    }
}

pub type MyResult<T> = Result<T, MyError>;
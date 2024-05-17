use actix_web::{http::{header::ContentType, StatusCode}, HttpResponse, ResponseError};
use http_for_actix::status::InvalidStatusCode;
use k256::ecdsa;
use thiserror::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Error)]
pub enum MyError {
    Anyhow(Box<anyhow::Error>),
    Io(std::io::Error),
    InvalidMethod(http::method::InvalidMethod),
    HttpResponse(reqwest::Response),
    ReqwestError(reqwest::Error),
    InvalidStatus(InvalidStatusCode),
    HeaderToStrForActix(http_for_actix::header::ToStrError),
    HeaderToStr(http::header::ToStrError),
    // InvalidHeaderName(Box<http_for_actix::header::IntoHeaderName>),
    MyCorruptedDB(MyCorruptedDBError),
    InvalidHeaderName(InvalidHeaderNameError),
    InvalidHeaderValue(InvalidHeaderValueError),
    Base64Decode(base64::DecodeError),
    Ecdsa(ecdsa::Error),
}

impl Display for MyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anyhow(e) => Debug::fmt(&*e, f),
            Self::Io(e) => Debug::fmt(&*e, f),
            Self::InvalidMethod(e) => Debug::fmt(&*e, f),
            Self::HttpResponse(e) => Debug::fmt(&*e, f),
            Self::ReqwestError(e) => Debug::fmt(&*e, f),
            Self::InvalidStatus(e) => Debug::fmt(&*e, f),
            Self::HeaderToStrForActix(e) => Debug::fmt(&*e, f),
            Self::HeaderToStr(e) => Debug::fmt(&*e, f),
            // Self::InvalidHeaderName(e) => Debug::fmt(&*e, f),
            Self::MyCorruptedDB(e) => Debug::fmt(&*e, f),
            Self::InvalidHeaderName(e) => Debug::fmt(&*e, f),
            Self::InvalidHeaderValue(e) => Debug::fmt(&*e, f),
            Self::Base64Decode(e) => Debug::fmt(&*e, f),
            Self::Ecdsa(e) => Debug::fmt(&*e, f),
        }
    }
}

#[derive(Debug, Default, Error)]
pub struct MyCorruptedDBError {}

impl Display for MyCorruptedDBError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Wrong data in DB.")
    }
}

#[derive(Debug, Default, Error)]
pub struct InvalidHeaderNameError {}

impl Display for InvalidHeaderNameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid header name.")
    }
}

#[derive(Debug, Default, Error)]
pub struct InvalidHeaderValueError {}

impl Display for InvalidHeaderValueError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid header value.")
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

impl From<http::method::InvalidMethod> for MyError {
    fn from(err: http::method::InvalidMethod) -> Self {
        Self::InvalidMethod(err)
    }
}

impl From<reqwest::Response> for MyError {
    fn from(err: reqwest::Response) -> Self {
        Self::HttpResponse(err)
    }
}

impl From<reqwest::Error> for MyError {
    fn from(err: reqwest::Error) -> Self {
        Self::ReqwestError(err)
    }
}

impl From<InvalidStatusCode> for MyError {
    fn from(err: InvalidStatusCode) -> Self {
        Self::InvalidStatus(err)
    }
}

impl From<http_for_actix::header::ToStrError> for MyError {
    fn from(err: http_for_actix::header::ToStrError) -> Self {
        Self::HeaderToStrForActix(err)
    }
}

impl From<http::header::ToStrError> for MyError {
    fn from(err: http::header::ToStrError) -> Self {
        Self::HeaderToStr(err)
    }
}

// impl From<Box<http_for_actix::header::IntoHeaderName>> for MyError {
//     fn from(err: Box<http_for_actix::header::IntoHeaderName>) -> Self {
//         Self::InvalidHeaderName(err)
//     }
// }

impl From<MyCorruptedDBError> for MyError {
    fn from(err: MyCorruptedDBError) -> Self {
        Self::MyCorruptedDB(err)
    }
}

impl From<InvalidHeaderNameError> for MyError {
    fn from(err: InvalidHeaderNameError) -> Self {
        Self::InvalidHeaderName(err)
    }
}

impl From<InvalidHeaderValueError> for MyError {
    fn from(err: InvalidHeaderValueError) -> Self {
        Self::InvalidHeaderValue(err)
    }
}

impl From<base64::DecodeError> for MyError {
    fn from(err: base64::DecodeError) -> Self {
        Self::Base64Decode(err)
    }
}

impl From<ecdsa::Error> for MyError {
    fn from(err: ecdsa::Error) -> Self {
        Self::Ecdsa(err)
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
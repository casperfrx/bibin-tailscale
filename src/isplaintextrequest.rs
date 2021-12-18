use std::ops::Deref;

use rocket::request::{FromRequest, Outcome};
use rocket::Request;

use async_trait::async_trait;

/// Holds a value that determines whether or not this request wanted a plaintext response.
///
/// We assume anything with the text/plain Accept or Content-Type headers want plaintext,
/// and also anything calling us from the console or that we can't identify.
pub struct IsPlaintextRequest(pub bool);

impl Deref for IsPlaintextRequest {
    type Target = bool;

    fn deref(&self) -> &bool {
        &self.0
    }
}

#[async_trait]
impl<'a> FromRequest<'a> for IsPlaintextRequest {
    type Error = ();

    async fn from_request(request: &'a Request<'_>) -> Outcome<IsPlaintextRequest, ()> {
        if let Some(format) = request.format() {
            if format.is_plain() {
                return Outcome::Success(IsPlaintextRequest(true));
            }
        }

        match request
            .headers()
            .get_one("User-Agent")
            .and_then(|u| u.split_once('/')).map(|u| u.0)
        {
            None | Some("Wget") | Some("curl") | Some("HTTPie") => {
                Outcome::Success(IsPlaintextRequest(true))
            }
            _ => Outcome::Success(IsPlaintextRequest(false)),
        }
    }
}

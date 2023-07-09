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
            .and_then(|u| u.split_once('/'))
            .map(|u| u.0)
        {
            None | Some("Wget") | Some("curl") | Some("HTTPie") => {
                Outcome::Success(IsPlaintextRequest(true))
            }
            _ => Outcome::Success(IsPlaintextRequest(false)),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::rocket;
    use rocket::http::Header;
    use rocket::http::Status;
    use rocket::local::blocking::Client;

    use super::IsPlaintextRequest;

    #[get("/tests/plaintext")]
    fn tests_plaintext(is_plain: IsPlaintextRequest) -> &'static str {
        if is_plain.0 {
            "plain_text"
        } else {
            "html"
        }
    }

    #[test]
    fn test_no_header() {
        let client = Client::debug_with(routes![tests_plaintext]).unwrap();
        let response = client.get(uri!(tests_plaintext)).dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "plain_text");
    }

    #[test]
    fn test_api_headers() {
        let client = Client::debug_with(routes![tests_plaintext]).unwrap();

        for user_agent in ["curl/8.1.2", "HTTPie/3.2.1", "Wget/1.21.4"] {
            let response = client
                .get(uri!(tests_plaintext))
                .header(Header::new("User-Agent", user_agent))
                .dispatch();
            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "plain_text");
        }
    }

    #[test]
    fn test_browser_headers() {
        let client = Client::debug_with(routes![tests_plaintext]).unwrap();

        for user_agent in [
            // Vivaldi 6.1.3035.100 stable
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
            // Mozilla Firefox 114.0.2
            "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/114.0",
        ] {
            let response = client
                .get(uri!(tests_plaintext))
                .header(Header::new(
                    "User-Agent",
                    user_agent,
                ))
                .dispatch();
            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "html");
        }
    }
}

use crate::auth::AuthKey;
use crate::config::BibinConfig;
use crate::highlight::Highlighter;
use crate::io::{get_all_paste, get_paste, ReadPool};
use crate::RedirectOrContent;
use crate::{isplaintextrequest::IsPlaintextRequest, HtmlOrPlain};
use base64::{engine::general_purpose, Engine as _};
use qrcode_generator::QrCodeEcc;
use rocket::http::{RawStr, Status};
use rocket::response::content::RawJson;
use rocket::response::Redirect;
use rocket::State;
use std::borrow::Cow;
use std::collections::HashMap;

use askama::{Html as AskamaHtml, MarkupDisplay, Template};

///
/// Show paste page
///

#[derive(Responder)]
#[response(content_type = "image/png")]
pub struct PngResponder(Vec<u8>);

#[derive(Template)]
#[template(path = "paste.html")]
struct ShowPaste<'a> {
    content: MarkupDisplay<AskamaHtml, Cow<'a, String>>,
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index;

#[derive(Template)]
#[template(path = "curl_help.txt")]
pub struct CurlIndex {
    root_url: String,
}

#[get("/")]
pub fn index(
    config: &State<BibinConfig>,
    plaintext: IsPlaintextRequest,
) -> Result<HtmlOrPlain, Status> {
    if plaintext.0 {
        CurlIndex {
            root_url: config.prefix.clone(),
        }
        .render()
        .map(HtmlOrPlain::Plain)
        .map_err(|_| Status::InternalServerError)
    } else {
        Index
            .render()
            .map(HtmlOrPlain::Html)
            .map_err(|_| Status::InternalServerError)
    }
}

#[get("/all_entries")]
pub async fn all_entries(
    pool: &State<ReadPool>,
    password: AuthKey,
    config: &State<BibinConfig>,
) -> Result<RawJson<String>, Status> {
    if !password.is_valid(&config.password) {
        return Err(Status::Unauthorized);
    }

    let entries = match get_all_paste(pool).await {
        Ok(entries) => entries,
        Err(e) => {
            warn!("[ALL_ENTRIES] Error in get_all_paste: {}", e);
            return Err(Status::InternalServerError);
        }
    };
    // Convert entries into a hashmap
    let result = entries
        .iter()
        .map(|(k, v)| (k, v))
        .collect::<HashMap<&String, &String>>();
    let json = serde_json::to_string(&result).unwrap();
    Ok(RawJson(json))
}

#[get("/<name>/qr")]
pub async fn get_qr(
    name: String,
    config: &State<BibinConfig>,
    pool: &State<ReadPool>,
) -> Result<PngResponder, Status> {
    let mut splitter = name.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    match get_paste(pool, key).await {
        // TODO: not found or Internal error
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[GET_QR] Error in get_paste: {}", e);
            return Err(Status::InternalServerError);
        }
        Ok(Some(_)) => (),
    };

    let result = qrcode_generator::to_png_to_vec(
        format!("{}/{}", config.prefix, &name),
        QrCodeEcc::Medium,
        1024,
    )
    .unwrap();
    Ok(PngResponder(result))
}

#[get("/<key>/raw")]
pub async fn get_item_raw(key: &str, pool: &State<ReadPool>) -> Result<HtmlOrPlain, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let content = match get_paste(pool, key).await {
        // TODO: not found or Internal error
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[GET_RAW] Error in get_paste: {}", e);
            return Err(Status::InternalServerError);
        }
        Ok(Some(content)) => content,
    };

    Ok(HtmlOrPlain::Plain(content))
}

#[get("/<key>")]
pub async fn get_item(
    key: &str,
    plaintext: IsPlaintextRequest,
    pool: &State<ReadPool>,
    highlighter: &State<Highlighter>,
) -> Result<RedirectOrContent, Status> {
    let mut splitter = key.splitn(2, '.');
    let key = splitter.next().ok_or(Status::NotFound)?;
    let ext = splitter.next();

    let entry = match get_paste(pool, key).await {
        Ok(Some(data)) => data,
        Ok(None) => return Err(Status::NotFound),
        Err(e) => {
            warn!("[SHOW_PASTE] get_paste error: {}", e);
            return Err(Status::InternalServerError);
        }
    };

    if let Some(extension) = ext {
        match extension {
            "url" => return Ok(RedirectOrContent::Redirect(Redirect::to(entry))),
            "qr" => match qrcode_generator::to_png_to_vec(entry, QrCodeEcc::Medium, 1024) {
                Ok(code) => return Ok(RedirectOrContent::Png(code)),
                Err(e) => {
                    warn!("[SHOW_PASTE] qrcode_generator: {}", e);
                    return Err(Status::InternalServerError);
                }
            },
            "b64" => {
                return Ok(RedirectOrContent::Plain(
                    general_purpose::STANDARD.encode(entry),
                ))
            }
            _ => (),
        }
    }

    if *plaintext {
        Ok(RedirectOrContent::Plain(entry))
    } else {
        let code_highlighted = match ext {
            Some(extension) => match highlighter.highlight(&entry, extension) {
                Ok(html) => html,
                Err(error) => {
                    error!("Error highlighting from extension {} {}", extension, error);
                    return Err(Status::InternalServerError);
                }
            },
            None => String::from(RawStr::new(&entry).html_escape()),
        };

        // Add <code> tags to enable line numbering with CSS
        let html = format!(
            "<code>{}</code>",
            code_highlighted.replace('\n', "</code><code>")
        );

        let content = MarkupDisplay::new_safe(Cow::Borrowed(&html), AskamaHtml);

        let template = ShowPaste { content };
        match template.render() {
            Ok(html) => Ok(RedirectOrContent::Html(html)),
            Err(_) => Err(Status::InternalServerError),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::iter::FromIterator;

    use crate::config::BibinConfig;
    use crate::highlight::Highlighter;
    use crate::io;
    use crate::io::{ReadPool, WritePool};
    use crate::rocket;
    use rocket::http::ext::IntoCollection;
    use rocket::http::{Header, Status};
    use rocket::local::asynchronous::Client;
    use rocket::tokio;
    use tempfile::NamedTempFile;

    use super::get_qr;
    use super::index;
    use super::{all_entries, rocket_uri_macro_all_entries};
    use super::{get_item, rocket_uri_macro_get_item};
    use super::{get_item_raw, rocket_uri_macro_get_item_raw};

    const ENTRY_CONTENT: &str = "This is a test";
    const PASSWORD: &str = "password123";

    async fn create_test_client() -> (NamedTempFile, Client) {
        let temp = NamedTempFile::new().unwrap();
        let file_name = temp.path().to_str().unwrap();
        let write_pool = WritePool::new(file_name)
            .await
            .expect("Error when creating the writing pool");

        write_pool
            .init()
            .await
            .expect("Error during initialization");

        let read_pool = ReadPool::new(file_name, 10)
            .await
            .expect("Error when creating the reading pool");

        let rocket = rocket::Rocket::build()
            .manage(read_pool)
            .manage(write_pool)
            .manage(Highlighter::new())
            .manage(
                serde_json::from_str::<BibinConfig>(
                    r#"{ "password": "password123", "prefix": "/" }"#,
                )
                .unwrap(),
            )
            .mount(
                "/",
                routes![index, all_entries, get_qr, get_item, get_item_raw],
            );
        // the NamedTempFile will be deleted when `temp` goes out of scope. We need
        // to hand it over to the tests so that it stays on the fs until the end of the test
        (temp, Client::untracked(rocket).await.unwrap())
    }

    #[rocket::async_test]
    async fn test_simple_case() {
        let (_temp, client) = create_test_client().await;
        let write_pool = client.rocket().state::<WritePool>().unwrap();

        let response = client.get(uri!(get_item_raw("bob"))).dispatch().await;
        assert_eq!(response.status(), Status::NotFound);

        let response = client.get(uri!(get_item("bob"))).dispatch().await;
        assert_eq!(response.status(), Status::NotFound);

        const ENTRY_CONTENT: &str = "This is a test";
        let key = io::store_paste(&write_pool, 5, 1000, ENTRY_CONTENT.to_string())
            .await
            .unwrap();
        assert_ne!(key, "");

        let response = client.get(uri!(get_item_raw(&key))).dispatch().await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.into_string().await.unwrap(),
            ENTRY_CONTENT.to_string()
        );

        let response = client.get(uri!(get_item(&key))).dispatch().await;
        assert_eq!(response.status(), Status::Ok);
    }

    #[tokio::test]
    async fn test_all_entries() {
        let (_temp, client) = create_test_client().await;
        let write_pool = client.rocket().state::<WritePool>().unwrap();

        let response = client.get(uri!(all_entries)).dispatch().await;
        assert_eq!(response.status(), Status::Unauthorized);

        let response = client
            .get(uri!(all_entries))
            .header(Header::new("X-API-Key", PASSWORD))
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().await.unwrap(), "{}");

        let key = io::store_paste(&write_pool, 5, 1000, ENTRY_CONTENT.to_string())
            .await
            .unwrap();
        assert_ne!(key, "");

        let response = client
            .get(uri!(all_entries))
            .header(Header::new("X-API-Key", PASSWORD))
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.headers().get("Content-Type").count(), 1);
        for t in response.headers().get("Content-Type") {
            assert!(t == "application/json");
        }
        let response = response.into_string().await.unwrap();
        let response_data: HashMap<String, String> = serde_json::de::from_str(&response).unwrap();
        assert_eq!(
            response_data,
            HashMap::from_iter([(key, ENTRY_CONTENT.to_string())])
        );
    }
}

use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    body::StreamBody,
    extract::{Path, State},
    http::{
        header::{self, ToStrError, ACCEPT_ENCODING},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router, Server,
};
use serde::de::{self, Deserialize};
use tokio_util::io::ReaderStream;

#[derive(Debug)]
pub struct AppState {
    root: PathBuf,
    json: bool,
    brotli: bool,
    gzip: bool,
}

pub fn var_or_else(env: &str, default: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| default.into())
}

#[allow(clippy::unused_async)]
pub async fn index() -> Html<&'static str> {
    Html(include_str!("../index.html"))
}

pub async fn hash5(
    Path(hash5): Path<Hash5>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let accepted = match get_accepted_encodings(&headers).map_err(|_| StatusCode::BAD_REQUEST) {
        Ok(accepted) => accepted,
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("invalid Accept-Encoding header: {err}"),
            ))
        }
    };

    let mut path = state.root.join(hash5.inner);
    let mut headers = HeaderMap::new();

    if state.brotli && accepted.brotli {
        path.set_extension("json.br");
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("br"));
    } else if state.gzip && accepted.gzip {
        path.set_extension("json.gz");
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    } else if state.json {
        path.set_extension("json");
    }

    let body = match tokio::fs::File::open(path)
        .await
        .map(ReaderStream::new)
        .map(StreamBody::new)
    {
        Ok(body) => body,
        Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {err}"))),
    };

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    Ok((headers, body))
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    ToStrError(#[from] ToStrError),

    #[error("Accept-Encoding item had an invalid format")]
    InvalidFormat,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AcceptedEncodings {
    brotli: bool,
    gzip: bool,
}

pub fn get_accepted_encodings(headers: &HeaderMap) -> Result<AcceptedEncodings, Error> {
    headers
        .get_all(ACCEPT_ENCODING)
        .into_iter()
        .map(|s| s.to_str().map_err(Error::from))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flat_map(|s| s.split(',').map(str::trim))
        .try_fold(AcceptedEncodings::default(), |mut acc, item| {
            match item.split(";q=").next().ok_or(Error::InvalidFormat)? {
                "br" => acc.brotli = true,
                "gzip" => acc.gzip = true,
                _ => (),
            }

            Ok(acc)
        })
}

pub async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let root = PathBuf::from(var_or_else("ROOT", ""));
    let json = root.join("0/0/0/0/0.json").exists();
    let brotli = root.join("0/0/0/0/0.json.br").exists();
    let gzip = root.join("0/0/0/0/0.json.gz").exists();

    let state = AppState {
        root,
        json,
        brotli,
        gzip,
    };

    let host: IpAddr = var_or_else("HOST", "127.0.0.1").parse()?;
    let port: u16 = var_or_else("PORT", "8080").parse()?;
    let address = SocketAddr::new(host, port);

    println!(
        "brotli: {} | gzip: {} | json: {}",
        state.brotli, state.gzip, state.json
    );

    if state.root.as_os_str().is_empty() {
        println!("using current working directory as root");
    } else {
        println!("root: {}", state.root.display());
    }

    println!("starting server at http://{address}/");

    let app = Router::new()
        .route("/", get(index))
        .route("/:hash5", get(hash5))
        .with_state(Arc::new(state));

    Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

pub struct Hash5 {
    inner: PathBuf,
}

pub fn hex(byte: u8) -> &'static str {
    match byte {
        48 => "0",
        49 => "1",
        50 => "2",
        51 => "3",
        52 => "4",
        53 => "5",
        54 => "6",
        55 => "7",
        56 => "8",
        57 => "9",
        65 | 97 => "A",
        66 | 98 => "B",
        67 | 99 => "C",
        68 | 100 => "D",
        69 | 101 => "E",
        70 | 102 => "F",
        _ => unreachable!(),
    }
}

impl<'de> Deserialize<'de> for Hash5 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <&str>::deserialize(deserializer)?;
        let raw = raw.as_bytes();

        if raw.len() != 5 {
            return Err(de::Error::invalid_length(raw.len(), &"5"));
        }

        for byte in raw {
            if !matches!(byte, 48..=57 | 65..=70 | 97..=102) {
                return Err(de::Error::invalid_value(
                    de::Unexpected::Char(*byte as char),
                    &"ascii hex character value",
                ));
            }
        }

        let path = std::path::Path::new(hex(raw[0]));
        let path = path.join(hex(raw[1]));
        let path = path.join(hex(raw[2]));
        let path = path.join(hex(raw[3]));
        let path = path.join(hex(raw[4]));

        Ok(Hash5 { inner: path })
    }
}

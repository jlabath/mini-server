use chrono::{DateTime, Utc};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::{convert::Infallible, env, io, net::SocketAddr};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt; // for read_to_end()

fn not_found() -> Response<Body> {
    Response::builder()
        .status(404)
        .body("not found\r\n".into())
        .unwrap()
}

fn forbidden() -> Response<Body> {
    Response::builder()
        .status(403)
        .body("forbidden\r\n".into())
        .unwrap()
}

fn trouble() -> Response<Body> {
    Response::builder()
        .status(500)
        .body("sad bear is sad\r\n".into())
        .unwrap()
}

async fn files(path: &str) -> io::Result<Vec<String>> {
    let mut file_names = vec![];
    let mut entries = fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        if let Ok(metadata) = entry.metadata().await {
            if metadata.is_file() {
                if let Ok(name) = entry.file_name().into_string() {
                    file_names.push(name);
                }
            }
        } else {
            println!("Couldn't get file type for {:?}", entry.path());
        }
    }

    Ok(file_names)
}

async fn index_view(_req: &Request<Body>) -> Response<Body> {
    let mut contents = String::from(
        "
<!DOCTYPE html>
<html lang=\"en\">
  <head>
    <meta charset=\"UTF-8\">
    <title>index</title>
  </head>
  <body>
    <h3>Welcome</h3>
<ul>
",
    );
    if let Ok(fnames) = files(".").await {
        for fname in fnames.iter() {
            let chunk = format!("<li><a href=\"{}\">{}</a></li>", fname, fname);
            contents.push_str(&chunk);
        }
    }
    contents.push_str("</ul></body></html>");
    Response::builder()
        .status(200)
        .header("Content-type", "text/html")
        .body(contents.into())
        .unwrap()
}

async fn file_view(req: &Request<Body>) -> Response<Body> {
    let mut chars = req.uri().path().chars();
    chars.next(); //drop / which is first character in path
    let path = chars.as_str();
    //first check for dots
    //this may be unnecessary - hyper seems to always flatten to /
    if path.contains("..") {
        forbidden()
    } else {
        match File::open(path).await {
            Ok(mut file) => {
                let mut contents = vec![];
                match file.read_to_end(&mut contents).await {
                    Ok(_) => file_response(path, contents).await,
                    Err(_) => trouble(),
                }
            }
            Err(_) => not_found(),
        }
        //file goes out of scope and gets closed automagically
    }
}

async fn file_response(path: &str, contents: Vec<u8>) -> Response<Body> {
    Response::builder()
        .status(200)
        .header("Content-type", mime_type(path))
        .body(contents.into())
        .unwrap()
}

fn mime_type(path: &str) -> &str {
    let path = String::from(path).to_lowercase(); //we can shadow orig variable if we want to
    if path.ends_with("html") {
        "text/html"
    } else if path.ends_with("htm") {
        "text/html"
    } else if path.ends_with("txt") {
        "text/plain"
    } else if path.ends_with("wasm") {
        "application/wasm"
    } else if path.ends_with("js") {
        "text/javascript"
    } else {
        "application/octet-stream"
    }
}

async fn handle(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let response = match req.uri().path() {
        "/" => index_view(&req).await,
        _ => file_view(&req).await,
    };
    //logging
    let now: DateTime<Utc> = Utc::now();
    let ua_agent = match req.headers().get("user-agent") {
        Some(agent) => agent.to_str().unwrap(),
        None => "-",
    };
    println!(
        "{} {} {} {} {:?} {}",
        now.to_rfc3339(),
        req.uri(),
        response.status(),
        req.method(),
        req.version(),
        ua_agent,
    );

    //return response
    Ok(response)
}

#[tokio::main]
async fn main() {
    //port via PORT variable
    let checked_port: Result<u16, std::num::ParseIntError> = match env::var("PORT") {
        Ok(val) => val.parse(),
        Err(_) => Ok(3000),
    };
    let port = match checked_port {
        Ok(n) => n,
        _ => 3000,
    };
    println!(
        "starting server on 127.0.0.1:{}\nYou can use PORT environment variable to change this.",
        port
    );
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

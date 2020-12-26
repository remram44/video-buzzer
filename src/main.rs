use std::net::Ipv4Addr;
use warp::Filter;
use warp::path;
use warp::reply::with::header;

const PORT: u16 = 8000;

#[tokio::main]
async fn main() {
    let routes =
        // Index, redirect to a video player
        path::end()
            .map(|| b"./" as &[u8]) // TODO
        // Buzzer join URL
        .or(
            warp::path!(u32)
                .map(|_| b"<>" as &[u8]) // TODO
        )
        // Video player
        .or(
            warp::path!("video" / u32)
                .map(|_| include_bytes!("../player.html") as &[u8])
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // Buzzer view
        .or(
            warp::path!("buzz" / u32 / u32)
                .map(|_, _| b"<>" as &[u8]) // TODO
        )
        // API
        .or(
            warp::path!("api" / "buzz" / u32 / String)
                .map(|_, _| b"{}" as &[u8]) // TODO
        );

    eprintln!("Starting server on port {}", PORT);
    warp::serve(routes).run((Ipv4Addr::UNSPECIFIED, PORT)).await;
}

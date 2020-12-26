use futures::{FutureExt, SinkExt, StreamExt};
use log::{info, warn};
use rand::Rng;
use std::collections::HashMap;
use std::env;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use warp::{Filter, Rejection, Reply};
use warp::http::Uri;
use warp::path;
use warp::reply::with::header;

const PORT: u16 = 8000;

struct Player {
    connected_channels: u32,
}

impl Default for Player {
    fn default() -> Player {
        Player{
            connected_channels: 0,
        }
    }
}

#[derive(Default)]
struct VideoRoom {
    channels: Vec<futures::channel::mpsc::UnboundedSender<String>>,
    players: HashMap<String, Player>,
}

type Rooms = Arc<Mutex<HashMap<u32, VideoRoom>>>;

fn with_rooms(rooms: Rooms)
-> impl Filter<Extract = (Rooms,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rooms.clone())
}

fn redirect_to_random_video() -> impl Reply {
    let video_id: u32 = rand::thread_rng().gen_range(1..1000000);
    let uri = Uri::builder()
        .path_and_query(format!("/video/{}", video_id))
        .build()
        .expect("Building random video URL failed");
    warp::redirect::temporary(uri)
}

fn host_websocket(
    video_id: u32,
    ws: warp::ws::Ws,
    rooms: Rooms,
) -> impl Reply {
    ws.on_upgrade(move |ws| {
        info!("Video {}: host connected", video_id);

        // Create a channel to communicate with buzzers
        let (chan_tx, chan_rx) = futures::channel::mpsc::unbounded();

        // Update room
        {
            let mut rooms = rooms.lock().unwrap();
            let room = rooms.entry(video_id).or_default();
            room.channels.push(chan_tx);
        }

        // Forward from internal channel to WebSocket
        let (ws_tx, _ws_rx) = ws.split();
        chan_rx
            .map(|msg| Ok(warp::ws::Message::text(msg)))
            .forward(ws_tx)
            .map(move |result| {
                if let Err(e) = result {
                    warn!("websocket error: {:?}", e);
                }
                info!("Video {}: host disconnected", video_id);
            })
    })
}

async fn buzzer_websocket(
    video_id: u32,
    player_name: String,
    ws: warp::ws::Ws,
    rooms: Rooms,
) -> Result<impl Reply, Rejection> {
    let player_name = percent_encoding::percent_decode(player_name.as_bytes())
        .decode_utf8()
        .map_err(|_| warp::reject::not_found())?
        .into_owned();
    Ok(ws.on_upgrade(move |ws| {
        info!("Video {}: player {:?} connected", video_id, player_name);

        // Update player
        {
            let mut rooms = rooms.lock().unwrap();
            let room = rooms.entry(video_id).or_default();

            let player = room.players.entry(player_name.clone()).or_default();
            player.connected_channels += 1;
        }

        async move {
            let (_tx, mut rx) = ws.split();
            while let Some(msg) = rx.next().await {
                info!("Video {}: player {:?}: {:?}", video_id, player_name, msg);

                let channels = {
                    let mut rooms = rooms.lock().unwrap();
                    let room = rooms.entry(video_id).or_default();
                    room.channels.clone()
                };

                for mut chan in channels {
                    let _ = chan.send(player_name.clone()).await;
                }
            }

            info!("Video {}: player {:?} disconnected", video_id, player_name);

            // Update player
            {
                let mut rooms = rooms.lock().unwrap();
                let room = rooms.entry(video_id).or_default();

                let player = room.players.entry(player_name.clone()).or_default();
                player.connected_channels -= 1;
            }
        }
    }))
}

#[tokio::main]
async fn main() {
    // Logging
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "video_buzzer=info");
    }
    pretty_env_logger::init();

    let rooms: Rooms = Arc::new(Mutex::new(HashMap::new()));

    let routes =
        // Index, redirect to a video player
        path::end()
            .map(redirect_to_random_video)
        // Video player
        .or(
            warp::path!("video" / u32)
                .map(|_| include_bytes!("../video.html") as &[u8])
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // Buzzer join URL
        .or(
            warp::path!(u32)
                .map(|_| include_bytes!("../join.html") as &[u8])
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // Buzzer view
        .or(
            warp::path!("buzz" / u32 / String)
                .map(|_, _| include_bytes!("../buzzer.html") as &[u8])
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // API
        .or(
            warp::path("api").and(
                // Video player WebSocket
                warp::path!("host" / u32)
                    .and(warp::ws())
                    .and(with_rooms(rooms.clone()))
                    .map(host_websocket)
                .or(
                    warp::path!("buzzer" / u32 / String)
                        .and(warp::ws())
                        .and(with_rooms(rooms))
                        .and_then(buzzer_websocket)
                )
            )
        );

    let routes = routes.with(warp::log("video_buzzer"));

    eprintln!("Starting server on port {}", PORT);
    warp::serve(routes).run((Ipv4Addr::UNSPECIFIED, PORT)).await;
}

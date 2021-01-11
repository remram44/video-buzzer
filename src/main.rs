use futures::select;
use futures::{SinkExt, StreamExt};
use log::{info, warn};
use rand::Rng;
use std::collections::hash_map::{Entry, HashMap};
use std::env;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use warp::http::Uri;
use warp::path;
use warp::reply::with::header;
use warp::{Filter, Rejection, Reply};

mod files;

const PORT: u16 = 8000;

struct TempSet<T> {
    next_id: u32,
    set: HashMap<u32, T>,
}

impl<T> TempSet<T> {
    fn add(&mut self, value: T) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.set.insert(id, value);
        id
    }

    fn remove(&mut self, id: u32) -> bool {
        self.set.remove(&id).is_some()
    }

    fn iter(&self) -> std::collections::hash_map::Values<u32, T> {
        self.set.values()
    }

    fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}

impl<T> Default for TempSet<T> {
    fn default() -> TempSet<T> {
        TempSet {
            next_id: 0,
            set: HashMap::new(),
        }
    }
}

/// Events sent to the video host
enum Event {
    PlayerJoined(String),
    PlayerBuzzed(String),
}

/// Information about a player
struct Player {
    connected_channels: u32,
}

impl Default for Player {
    fn default() -> Player {
        Player {
            connected_channels: 0,
        }
    }
}

#[derive(Default)]
struct VideoRoom {
    /// Channels to video hosts in this room (usually just one)
    channels: TempSet<futures::channel::mpsc::UnboundedSender<Event>>,
    /// List of players currently in this room
    players: HashMap<String, Player>,
}

impl VideoRoom {
    fn is_empty(&self) -> bool {
        // The room is empty if it has no host and no player
        self.channels.is_empty()
            && self.players.values().all(|p| p.connected_channels == 0)
    }
}

type Rooms = Arc<Mutex<HashMap<u32, VideoRoom>>>;

fn with_rooms(
    rooms: Rooms,
) -> impl Filter<Extract = (Rooms,), Error = std::convert::Infallible>
       + Clone {
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

        async move {
            let (mut ws_tx, ws_rx) = ws.split();

            // Create a channel to communicate with buzzers
            let (chan_tx, mut chan_rx) = futures::channel::mpsc::unbounded();

            // Update room
            let (chan_id, players): (_, Vec<String>) = {
                let mut rooms = rooms.lock().unwrap();
                let room = rooms.entry(video_id).or_default();
                (
                    room.channels.add(chan_tx),
                    room.players.keys().cloned().collect(),
                )
            };

            // Send list of players
            for player_name in players {
                let _ = ws_tx
                    .send(warp::ws::Message::text(
                        format!("join {}", player_name),
                    ))
                    .await;
            }

            // Forward from channel to WebSocket, until WebSocket closes
            let mut ws_rx = ws_rx.fuse();
            loop {
                select! {
                    msg = chan_rx.next() => match msg {
                        // Message from channel, send it on WebSocket
                        Some(msg) => {
                            let text = match msg {
                                Event::PlayerJoined(player) => {
                                    format!("join {}", player)
                                }
                                Event::PlayerBuzzed(player) => {
                                    format!("buzz {}", player)
                                }
                            };
                            match ws_tx
                                .send(warp::ws::Message::text(text))
                                .await
                            {
                                Err(e) => {
                                    warn!("websocket error: {:?}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                        // Channel is closed
                        None => panic!("Internal channel was closed"),
                    },
                    msg = ws_rx.next() => match msg {
                        // Message from WebSocket, ignore
                        Some(_) => {}
                        // WebSocket is closed
                        None => {
                            info!("Video {}: host disconnected", video_id);
                            break;
                        }
                    },
                }
            }

            // Remove channel now that connection is closed
            {
                let mut rooms = rooms.lock().unwrap();
                let room = rooms.get_mut(&video_id).unwrap();
                room.channels.remove(chan_id);

                // No one left, free memory
                if room.is_empty() {
                    info!("Video {}: empty, removing", video_id);
                    rooms.remove(&video_id);
                }
            }
        }
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

        async move {
            // Update player
            let notify_channels = {
                let mut rooms = rooms.lock().unwrap();
                let room = rooms.entry(video_id).or_default();

                match room.players.entry(player_name.clone()) {
                    Entry::Occupied(mut player) => {
                        player.get_mut().connected_channels += 1;
                        Vec::new()
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Player {
                            connected_channels: 1,
                        });

                        info!("(first connection, notify)");
                        room.channels.iter().cloned().collect::<Vec<_>>()
                    }
                }
            };

            for mut chan in notify_channels {
                let _ =
                    chan.send(Event::PlayerJoined(player_name.clone())).await;
            }

            // Receive messages on WebSocket, forward to room
            let (_tx, mut rx) = ws.split();
            while let Some(msg) = rx.next().await {
                let msg = match msg {
                    Ok(ref m) => match m.to_str() {
                        Ok(s) => s,
                        Err(_) => continue,
                    },
                    Err(e) => {
                        warn!("websocket error: {:?}", e);
                        break;
                    }
                };
                info!(
                    "Video {}: player {:?}: {:?}",
                    video_id, player_name, msg
                );

                let channels = {
                    let mut rooms = rooms.lock().unwrap();
                    let room = rooms.entry(video_id).or_default();
                    room.channels.iter().cloned().collect::<Vec<_>>()
                };

                for mut chan in channels {
                    let _ = chan
                        .send(Event::PlayerBuzzed(player_name.clone()))
                        .await;
                }
            }

            info!("Video {}: player {:?} disconnected", video_id, player_name);

            // Update player
            {
                let mut rooms = rooms.lock().unwrap();
                let room = rooms.entry(video_id).or_default();

                let player =
                    room.players.entry(player_name.clone()).or_default();
                player.connected_channels -= 1;

                // No one left, free memory
                if room.is_empty() {
                    info!("Video {}: empty, removing", video_id);
                    rooms.remove(&video_id);
                }
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
                .map(|_| ()).untuple_one()
                .and(files::video())
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // Buzzer join URL
        .or(
            warp::path!(u32)
                .map(|_| ()).untuple_one()
                .and(files::join())
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // Buzzer view
        .or(
            warp::path!("buzz" / u32 / String)
                .map(|_, _| ()).untuple_one()
                .and(files::buzzer())
                .with(header("Content-Type", "text/html; charset=utf-8"))
        )
        // CSS files
        .or(
            warp::path("css").and(
                warp::path("bootstrap.min.css")
                    .and(files::css_bootstrap())
                .or(
                    warp::path("custom.css")
                        .and(files::css_custom())
                )
            )
            .with(header("Content-Type", "text/css"))
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

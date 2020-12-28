Video Buzzer
============

This is a couch gaming system for television shows. It is a video player (that you should display on your TV) that allows players to buzz from their smartphone, which pauses the video.

![screenshot](screenshot.jpg)

The workflow is as follow:

* Navigate to the application on your smart TV (or a computer hooked to the TV)
* Load a local video file
* Each player enters the displayed link into their smartphone, then their name when prompted. They now see a buzzer button
* Play the video file. During the show, players can buzz, which displays their name on the TV and pauses the video so they can give their answer
* The video file resumes and the player finds out if they were right

Note that:

* If the answer given is incorrect, there is no possibility for an other player to give another (because the recorded show will state the correct answer)
* This requires players to be in the same room, the video is not streamed to each device, just played on the "host" device (TV)
* This works with any recorded game show, so long as it is not too interactive (because the recording won't respond to your actions)

Status
------

* [x] Can play a video file
* [x] Can join a game and buzz
* [x] Buzzing stops the video, displays name of player
* [x] Allow full-screening video, make buzzer big
* [x] Make interface pretty and user-friendly

How to run
----------

You can use the app by going to https://buzzer.remram.fr/.

If you want to deploy it for yourself, build it using `cargo build --release` (you will need the [Rust compiler](https://www.rust-lang.org/tools/install)). This will make an executable `target/release/video-buzzer` which you can run on your server.

To run the app in development mode, you can use `cargo run` (you will need the [Rust compiler](https://www.rust-lang.org/tools/install)). In this mode, the HTML/CSS files are served from disk, allowing you to test changes without rebuilding. If you have [cargo-watch](https://crates.io/crates/cargo-watch) installed, you can use `cargo watch -w src -x run` to have the server rebuild and restart when you change the Rust sources.

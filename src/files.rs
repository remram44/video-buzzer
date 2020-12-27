#[cfg(not(debug_assertions))]
use std::convert::Infallible;
use warp::Filter;
#[cfg(debug_assertions)]
use warp::Rejection;
#[cfg(not(debug_assertions))]
use warp::filters::any::any;
#[cfg(debug_assertions)]
use warp::fs::{File, file};

macro_rules! use_file {
    ($file:ident) => {
        #[cfg(not(debug_assertions))]
        pub fn $file() -> impl Filter<Extract = (&'static [u8],), Error = Infallible> + Clone {
            any().map(|| include_bytes!(concat!("../", stringify!($file), ".html")) as &[u8])
        }

        #[cfg(debug_assertions)]
        pub fn $file() -> impl Filter<Extract = (File,), Error = Rejection> + Clone {
            file(concat!(stringify!($file), ".html"))
        }
    };
}

use_file!(video);
use_file!(join);
use_file!(buzzer);
